//! `ja1front --setup` (port option, not in the original): the play settings of config.rs on the office switch box.
//! Each option is a switch row: the handle place shows the value (far left = the first, far right = the last), the
//! lamp is lit when the handle is not at far left, and the name holds the value for options with more than 2 values
//! (a click gives the next value, a right click the one before). The big lever starts the game; Esc quits. Every
//! change goes to ja1.cfg at once (the window does the file).

use std::sync::Arc;

use crate::config::OPTS;
use crate::data::Common;
use crate::office::{OfficeData, Pic};
use crate::panel::{self, Painter, PanelData};
use crate::switchbox::{self, Step};

const PW: usize = 320;
const N: usize = OPTS.len();
/// The switch slot of each option (switchbox::place): mouse and window left, sound and game right. Insert #13556 item
/// 3: Subtitles (option 8) in the left column (slot 6), A.I.M. grid (option 12) in the last right row (slot 16). Slot
/// 14 (the old Go switch, 2026-11-25: test saves out of the release) stays empty: films above, game rules below.
const SLOT: [usize; N] = [0, 1, 2, 3, 4, 5, 10, 11, 6, 12, 13, 15, 16];
/// The row width that a click hits (lever + name).
const ROW_W: i32 = 104;
/// The widest row label that fits its field (text at slot x + 30, the field ends 75 pixels on, 2 pixels room).
pub const LABEL_MAX: i32 = 73;
/// The big lever: its place and its 9 frames (22 x 130 at PANBRUSH (200 + 22 f, 0)), as switchbox.rs.
const LEVER: (i32, i32) = (244, 31);
/// The help panel over the empty switch rows 7..9 and 17..19 (x, y, w, h; row 16 = A.I.M. grid), its fill and frame
/// colours (room palette: a dark blue grey and the slot colour). It ends where CLOSE starts.
const HELP: (i32, i32, i32, i32) = (28, 131, 211, 41);
const HELP_FILL: u8 = 6;
const HELP_EDGE: u8 = 27;
/// The text colours: a lit row (warm, as the copper of the levers), on, off.
const LIT: u8 = 186;
const ON: u8 = 0x28;
/// Insert #13556 item 1: off = 0x7A (room palette (170, 170, 202)), readable and less bright than on.
const OFF: u8 = 0x7A;
/// The engraved CLOSE under the lever area: covered and replaced by the lever's meaning.
const CLOSE: (i32, i32, i32, i32) = (171, 172, 62, 11);

/// What a press or a key asks the window to do.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Act {
    None,
    /// The values changed: write the file.
    Changed,
    Play,
    Quit,
}

pub struct Setup {
    pd: Arc<PanelData>,
    c: Arc<Common>,
    data: OfficeData,
    /// The value index of each option.
    pub v: [usize; N],
    /// The option under the mouse or chosen by the keys.
    pub hover: Option<usize>,
    /// A line that replaces the help (a file error).
    pub note: Option<String>,
    pub buf: Vec<u8>,
    pub pal: [u8; 768],
    /// A slide frame: option i draws its handle at this x (switch_pic) instead of its value's place.
    slide: Option<(usize, i32)>,
}

impl Setup {
    /// The box with the values `v`.
    pub fn new(iso: &str, exe: &str, v: [usize; N]) -> Result<Setup, i32> {
        crate::sound::screen_main(iso, exe);
        let data = OfficeData::load(exe)?;
        let (c, pd) = (crate::common(iso)?, panel::panel_data(iso, exe)?);
        let pal = data.room_palette();
        let mut s = Setup { pd, c, data, v, hover: None, note: None, buf: vec![0; PW * 200], pal, slide: None };
        s.draw();
        Ok(s)
    }

    /// The row label: the name of a lever, else "name: value".
    pub fn label(&self, i: usize) -> String {
        let o = &OPTS[i];
        if o.lever() { o.label.to_string() } else { format!("{}: {}", o.label, o.values[self.v[i]].2) }
    }

    /// "width label" for every row label with every value (a test checks them against the field).
    pub fn label_widths(&mut self) -> Vec<String> {
        let mut out = vec![];
        for i in 0..N {
            let keep = self.v[i];
            for j in 0..OPTS[i].values.len() {
                self.v[i] = j;
                let l = self.label(i);
                out.push(format!("{} {}", panel::text_width(&self.pd.fonts, 1, l.as_bytes()), l));
            }
            self.v[i] = keep;
        }
        for l in std::env::var("JA1_SETUP_MEASURE").unwrap_or_default().split(',').filter(|l| !l.is_empty()) {
            out.push(format!("{} {} (extra)", panel::text_width(&self.pd.fonts, 1, l.as_bytes()), l));
        }
        out
    }

    /// The option row at (x, y).
    pub fn row_at(x: i32, y: i32) -> Option<usize> {
        (0..N).find(|&i| {
            let (sx, sy) = switchbox::place(SLOT[i]);
            sx <= x && x < sx + ROW_W && sy - 3 <= y && y < sy + 11
        })
    }

    fn on_lever(x: i32, y: i32) -> bool {
        let (lx, ly) = LEVER;
        lx <= x && x <= lx + 22 && ly <= y && y <= ly + 65
    }

    /// The picture: the room, the box, the rows, the help line.
    pub fn draw(&mut self) {
        let (room, _) = self.data.room();
        crate::office::put(&mut self.buf, room, 0, 0);
        let sw = self.data.swbox.clone();
        part(&mut self.buf, &sw, 0, 0, 282, 200, 0, 0);
        let mut table0 = self.c.remap[0];
        let labels: Vec<String> = (0..N).map(|i| self.label(i)).collect();
        let mut p = Painter { d: &self.pd, buf: &mut self.buf, table0: &mut table0, table29: panel::table29(&self.c) };
        for i in 0..N {
            let (x, y) = switchbox::place(SLOT[i]);
            let o = &OPTS[i];
            // Insert #13556 item 2: Speed (option 1) works only with Grab mouse (option 0) on, so it is dim while that is
            // off (a click or a key still changes it).
            let on = if o.lever() { self.v[i] == 1 } else if i == 1 { self.v[0] == 1 } else { true };
            let hx = match self.slide {
                Some((j, h)) if j == i => h,
                _ => handle_x(self.v[i], o.values.len()),
            };
            switch_pic(p.buf, &self.data.panbrush, hx, x, y);
            let lit = self.hover == Some(i);
            let fg = if lit { LIT } else if on { ON } else { OFF };
            p.text(labels[i].as_bytes(), x + 30, y + 2, false, fg, 3, 1);
        }
        // An empty switch slot (slot 14, the old Go switch) shows the plate beside it, not a black hole.
        for slot in (0..7).chain(10..17).filter(|s| !SLOT.contains(s)) {
            blank_slot(p.buf, slot);
        }
        let help: String = match (&self.note, self.hover) {
            (Some(n), _) => n.clone(),
            (None, Some(i)) => OPTS[i].help.to_string(),
            (None, None) => "Pull the lever to play. Esc quits.".to_string(),
        };
        let (hx, hy, hw, hh) = HELP;
        fill(p.buf, hx, hy, hw, hh, HELP_EDGE);
        fill(p.buf, hx + 1, hy + 1, hw - 2, hh - 2, HELP_FILL);
        let lines: Vec<String> = help.split('\n').flat_map(|l| wrap(l, 34)).collect();
        // The row label heads the help when the help has 3 lines or fewer (4 lines fill the panel: Size).
        let head = self.hover.is_some() && lines.len() <= 3;
        if let (true, Some(i)) = (head, self.hover) {
            p.text(labels[i].as_bytes(), hx + 6, hy + 4, false, LIT, 3, 1);
        }
        for (k, line) in lines.iter().enumerate().take(4) {
            // A help with one line per value (Size) lights the line of the chosen value.
            let chosen = self.note.is_none() && self.hover.is_some_and(|i| help.contains('\n') && self.v[i] == k);
            p.text(line.as_bytes(), hx + 6, hy + 4 + 9 * (head as i32 + k as i32), false,
                   if chosen { LIT } else { ON }, 3, 1);
        }
        let (cx, cy, cw, ch) = CLOSE;
        fill(p.buf, cx, cy, cw, ch, 18);
        p.text(b"PULL = PLAY", cx + cw / 2, cy + 2, true, OFF, 3, 1);
    }

    /// The next (`step` = 1) or previous (-1) value of option i.
    fn cycle(&mut self, i: usize, step: i32) {
        let n = OPTS[i].values.len() as i32;
        self.v[i] = (self.v[i] as i32 + step).rem_euclid(n) as usize;
    }

    /// A mouse move to (x, y): true when the picture changed.
    pub fn hover_at(&mut self, x: i32, y: i32) -> bool {
        let h = Self::row_at(x, y);
        if h == self.hover {
            return false;
        }
        self.hover = h;
        self.draw();
        true
    }

    /// A press at (x, y), `right` = the right button; the steps to show and play.
    pub fn press(&mut self, x: i32, y: i32, right: bool, steps: &mut Vec<Step>) -> Act {
        if let Some(i) = Self::row_at(x, y) {
            self.hover = Some(i);
            return self.change(i, if right { -1 } else { 1 }, steps);
        }
        if !right && Self::on_lever(x, y) {
            return self.pull(steps);
        }
        Act::None
    }

    /// Insert #13504: after the switch sound the handle slides from the old to the new value's place in 3 frames
    /// (a wrap slides the full width); the last frame is the picture at rest.
    fn change(&mut self, i: usize, step: i32, steps: &mut Vec<Step>) -> Act {
        steps.push(Step::Sfx(switchbox::FX_SWITCH, 0x2B11, 20, true));
        let n = OPTS[i].values.len();
        let from = handle_x(self.v[i], n);
        self.cycle(i, step);
        let to = handle_x(self.v[i], n);
        self.note = None;
        for f in 1..=SLIDE_FRAMES {
            self.slide = if f < SLIDE_FRAMES { Some((i, from + (to - from) * f / SLIDE_FRAMES)) } else { None };
            self.draw();
            steps.push(Step::Show(self.buf.clone(), SLIDE_MS));
        }
        Act::Changed
    }

    /// The big lever goes down (9 frames), then the game starts.
    fn pull(&mut self, steps: &mut Vec<Step>) -> Act {
        steps.push(Step::Sfx(switchbox::FX_LEVER, 0x2B11, 20, true));
        let (lx, ly) = LEVER;
        for f in 0..9 {
            let pb = &self.data.panbrush;
            part(&mut self.buf, pb, 200 + 22 * f, 0, 22, 130, lx, ly);
            steps.push(Step::Show(self.buf.clone(), 60));
        }
        Act::Play
    }

    /// A key (SDL key name): Up / Down choose a row, Space / Right / Left change it, Enter or P plays, Esc quits.
    pub fn key(&mut self, name: &str, steps: &mut Vec<Step>) -> Act {
        let order = screen_order();
        let pos = self.hover.and_then(|h| order.iter().position(|&i| i == h));
        match name {
            "Up" | "Down" => {
                let n = order.len();
                let k = match (pos, name) {
                    (None, "Up") => n - 1,
                    (None, _) => 0,
                    (Some(k), "Up") => (k + n - 1) % n,
                    (Some(k), _) => (k + 1) % n,
                };
                self.hover = Some(order[k]);
                self.draw();
                steps.push(Step::Show(self.buf.clone(), 0));
                Act::None
            }
            "Space" | "Right" | "Left" => match self.hover {
                Some(i) => self.change(i, if name == "Left" { -1 } else { 1 }, steps),
                None => Act::None,
            },
            "Return" | "Keypad Enter" | "P" => self.pull(steps),
            "Escape" => Act::Quit,
            _ => Act::None,
        }
    }

    /// The command line words after ISO EXE: the lever starts the main menu (saves/ holds the games).
    pub fn start_args(&self) -> Vec<String> {
        vec!["--menu".into(), "saves".into()]
    }
}

/// Cover the switch cell of `slot` (22 x 13 from 1 pixel left of and above its place, frame included) with the plate
/// right of it (the empty label field of the same row).
fn blank_slot(buf: &mut [u8], slot: usize) {
    let (x, y) = switchbox::place(slot);
    for r in y - 1..y + 12 {
        let o = r as usize * PW;
        buf.copy_within(o + (x + 23) as usize..o + (x + 45) as usize, o + (x + 1) as usize);
    }
}

/// The slide of a switch handle: frames and the time each frame shows.
const SLIDE_FRAMES: i32 = 3;
const SLIDE_MS: u32 = 30;
/// The handle x in the switch (0 = far left, HANDLE_MAX = far right) of value v of n, at equal steps.
const HANDLE_MAX: i32 = 17;

pub fn handle_x(v: usize, n: usize) -> i32 {
    if n < 2 {
        return HANDLE_MAX;
    }
    let (v, d) = (v as i32, n as i32 - 1);
    (2 * HANDLE_MAX * v + d) / (2 * d)
}

/// A switch (26 x 7, PANBRUSH row 74) with its handle at `hx` and its lamp (lit = handle not at far left). At the
/// two ends: the OFF (26, 74) and ON (0, 74) pictures. Between: the empty slot (ON cols 0-16 = frame + track, OFF
/// cols 17-25 = track + frame) and the ON handle (cols 17-25) at hx.
fn switch_pic(buf: &mut [u8], pb: &Pic, hx: i32, x: i32, y: i32) {
    if hx <= 0 {
        part(buf, pb, 26, 74, 26, 7, x, y);
    } else {
        part(buf, pb, 0, 74, 17, 7, x, y);
        part(buf, pb, 26 + 17, 74, 9, 7, x + 17, y);
        part(buf, pb, 17, 74, 9, 7, x + hx, y);
    }
    part(buf, pb, if hx > 0 { 58 } else { 52 }, 74, 6, 4, x + 10, y + 7);
}

/// Copy the w x h area at (sx, sy) of `pic` to (x, y) of the 320 x 200 picture `buf`.
fn part(buf: &mut [u8], pic: &Pic, sx: i32, sy: i32, w: i32, h: i32, x: i32, y: i32) {
    let (pw, ph, px) = pic;
    for r in 0..h {
        for c in 0..w {
            let (u, v, dx, dy) = ((sx + c) as usize, (sy + r) as usize, x + c, y + r);
            if u < *pw && v < *ph && (0..PW as i32).contains(&dx) && (0..200).contains(&dy) {
                buf[dy as usize * PW + dx as usize] = px[v * pw + u];
            }
        }
    }
}

fn fill(buf: &mut [u8], x: i32, y: i32, w: i32, h: i32, c: u8) {
    for r in y.max(0)..(y + h).min(200) {
        for q in x.max(0)..(x + w).min(PW as i32) {
            buf[r as usize * PW + q as usize] = c;
        }
    }
}

/// Lines of at most `n` characters, broken at spaces.
fn wrap(s: &str, n: usize) -> Vec<String> {
    let mut out: Vec<String> = vec![String::new()];
    for w in s.split(' ') {
        let last = out.last_mut().unwrap();
        if !last.is_empty() && last.len() + 1 + w.len() > n {
            out.push(w.to_string());
        } else {
            if !last.is_empty() {
                last.push(' ');
            }
            last.push_str(w);
        }
    }
    out
}

/// Insert #13556 item 4: the options in screen order (Up / Down keys): the left column top to bottom, then the right
/// one (slot order).
pub fn screen_order() -> Vec<usize> {
    let mut o: Vec<usize> = (0..N).collect();
    o.sort_by_key(|&i| SLOT[i]);
    o
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rows_do_not_overlap_and_each_row_hits_its_option() {
        for i in 0..N {
            let (x, y) = switchbox::place(SLOT[i]);
            assert_eq!(Setup::row_at(x + 1, y + 2), Some(i));
            assert_eq!(Setup::row_at(x + ROW_W - 1, y + 2), Some(i));
        }
        assert_eq!(Setup::row_at(LEVER.0 + 5, LEVER.1 + 5), None);
    }

    /// the maintainer 2026-11-14: "Pixels: Smooth" and "Capture mouse" ran past the field. Every label with every value fits.
    #[test]
    fn every_label_fits_its_field() {
        let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
        let (iso, exe) = (root.join("rom/JA1.gog"), root.join("build/game113b/JA.EXE"));
        if !iso.exists() || !exe.exists() {
            return;
        }
        let _g = crate::sound::SCREEN_TEST_LOCK.lock().unwrap_or_else(|e| e.into_inner());
        let v = OPTS.each_ref().map(|o| o.default);
        let mut s = Setup::new(iso.to_str().unwrap(), exe.to_str().unwrap(), v).unwrap();
        for line in s.label_widths() {
            let w: i32 = line.split(' ').next().unwrap().parse().unwrap();
            assert!(w <= LABEL_MAX, "label too wide for its field: {}", line);
        }
    }

    /// Insert #13504: the handle of every value of every option is inside the slot, rises with the value, and sits at
    /// the OFF / ON picture places at the two ends.
    #[test]
    fn handle_place_rises_with_the_value() {
        for o in OPTS.iter() {
            let n = o.values.len();
            let xs: Vec<i32> = (0..n).map(|v| handle_x(v, n)).collect();
            assert_eq!((xs[0], xs[n - 1]), (0, HANDLE_MAX), "{}", o.label);
            assert!(xs.windows(2).all(|w| w[0] < w[1]), "{}: {:?}", o.label, xs);
            assert!(xs.iter().all(|&x| (0..=HANDLE_MAX).contains(&x) && x + 9 <= 26), "{}: {:?}", o.label, xs);
        }
    }

    /// A switch at rest gives the old pictures at the two ends (the OFF / ON art and the lamp); a middle place keeps
    /// the frame and differs from both.
    #[test]
    fn switch_ends_are_the_old_pictures() {
        let (w, h) = (64usize, 81usize);
        let px: Vec<u8> = (0..w * h).map(|k| (k * 7 % 251) as u8).collect();
        let pb: Pic = (w, h, px);
        let old = |on: bool| {
            let mut b = vec![0u8; PW * 200];
            part(&mut b, &pb, if on { 0 } else { 26 }, 74, 26, 7, 28, 26);
            part(&mut b, &pb, if on { 58 } else { 52 }, 74, 6, 4, 38, 33);
            b
        };
        let new = |hx: i32| {
            let mut b = vec![0u8; PW * 200];
            switch_pic(&mut b, &pb, hx, 28, 26);
            b
        };
        assert!(new(0) == old(false), "far left is not the OFF picture");
        assert!(new(HANDLE_MAX) == old(true), "far right is not the ON picture");
        let mid = new(handle_x(1, 3));
        assert!(mid != old(true) && mid != old(false));
        assert_eq!(mid[26 * PW + 28], pb.2[74 * w], "the left frame of the slot");
    }

    /// Insert #13556 items 3 + 4: Subtitles sits under Scanlines in the left column, A.I.M. grid last on the right,
    /// and Up / Down walk the rows as the screen shows them.
    #[test]
    fn keys_follow_the_screen_order() {
        assert_eq!(screen_order(), [0, 1, 2, 3, 4, 5, 8, 6, 7, 9, 10, 11, 12]);
        let o = screen_order();
        for w in o.windows(2) {
            let (a, b) = (switchbox::place(SLOT[w[0]]), switchbox::place(SLOT[w[1]]));
            assert!(a.0 < b.0 || (a.0 == b.0 && a.1 < b.1), "{:?} before {:?}", a, b);
        }
        assert_eq!((OPTS[8].key, SLOT[8]), ("subtitles", 6));
        assert_eq!((OPTS[12].key, SLOT[12]), ("aim_browser", 16));
    }

    #[test]
    fn help_lines_wrap_at_spaces() {
        let w = wrap("On: the window holds the mouse, as DOS. Ctrl+F10 frees it.", 34);
        assert_eq!(w, vec!["On: the window holds the mouse, as", "DOS. Ctrl+F10 frees it."]);
    }
}
