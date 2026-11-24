//! The salary box (docs/spec/strategic.md "Salary raise", host op 297 raise_offer): "Raise salary?" with the merc's
//! face, a dollar field and the buttons NO, OK, down and up. The player steps the offer up or down from the merc's
//! salary; OK is shaded until the offer is above the start.
use crate::invdraw::InvData;
use crate::panel::{archive_member, pcx, Painter, PW};
use crate::question::BOX;

/// The text of the box (0x99E47).
pub const TEXT: &[u8] = b"Raise salary?";

/// Buttons 5 (NO), 6 (OK), 0x11 (down) and 0x12 (up): screen x, y (button table 0x9345A).
pub const BUTTONS: [(i32, i32); 4] = [(142, 92), (208, 92), (152, 69), (219, 69)];

/// The highest offer the up button still raises (25000).
pub const MAX: i16 = 25000;

/// 0x67CD8 (DL 1) for button k: the mouse is inside the button's place + the pressed element's w, h (both edges in).
pub fn button_hit(k: usize, x: i32, y: i32) -> bool {
    let (bx, by) = BUTTONS[k];
    let (w, h) = if k < 2 { (37, 15) } else { (19, 15) };
    bx <= x && x <= bx + w && by <= y && y <= by + h
}

/// One box: the box picture (a 320 x 200 scratch, box pixels at their screen places) and the number entry.
pub struct SalaryBox {
    img: Vec<u8>,
    /// MYBRUSH.PCC (40 x 16): the box loads it into the panel sheet at (220, 49), over elements 0x2C and 0x2D.
    brush: Vec<u8>,
    pub offer: Offer,
}

/// The number entry of 0x2573C without the pictures: the start offer, the offer, the step and the OK flag.
#[derive(Clone, Copy, Debug)]
pub struct Offer {
    pub start: i16,
    pub cur: i16,
    step: i16,
    ok: bool,
}

/// What one press of 0x2573C did: the box ends with the offer (NO = 0), or the OK button changed (shaded or not)
/// and the field shows the new offer, or only the field changed, or nothing happened.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Press {
    End(i16),
    Ok(bool),
    Field,
    Nothing,
}

impl Offer {
    pub fn new(start: i16) -> Offer {
        Offer { start, cur: start, step: step_of(start), ok: false }
    }

    /// Whether button k (0 NO, 1 OK, 2 down, 3 up) is tested this round (OK only above the start offer).
    pub fn active(&self, k: usize) -> bool {
        k < 4 && (k != 1 || self.cur > self.start)
    }

    /// A press of button k came up (0x2573C).
    pub fn press(&mut self, k: usize) -> Press {
        if !self.active(k) {
            return Press::Nothing;
        }
        match k {
            0 => Press::End(0),
            // 0x258BA: a result below 0 does not end the box (an OK on a negative offer is lost).
            1 => if self.cur >= 0 { Press::End(self.cur) } else { Press::Nothing },
            2 => {
                if self.cur > self.start {
                    self.cur = self.cur.wrapping_sub(self.step);
                }
                if self.cur == self.start {
                    self.ok = false;
                    Press::Ok(false)
                } else {
                    Press::Field
                }
            }
            _ => {
                if self.cur < MAX {
                    self.cur = self.cur.wrapping_add(self.step);
                }
                if !self.ok {
                    self.ok = true;
                    Press::Ok(true)
                } else {
                    Press::Field
                }
            }
        }
    }
}

/// The answer of a box that gets the presses `keys` (0 NO, 1 OK, 2 down, 3 up) in order: the offer of the first press
/// that ends it; the presses run out first: NO (0).
pub fn script_offer(start: i16, keys: &[usize]) -> i16 {
    let mut o = Offer::new(start);
    for &k in keys {
        if let Press::End(v) = o.press(k) {
            return v;
        }
    }
    0
}

/// The step of the offer by the start offer: 5 below 500, 10 below 1000, 25 below 2500, 50 below 5000, else 100.
pub fn step_of(start: i16) -> i16 {
    match start {
        s if s < 500 => 5,
        s if s < 1000 => 10,
        s if s < 2500 => 25,
        s if s < 5000 => 50,
        _ => 100,
    }
}

impl SalaryBox {
    /// Build the box (SalaryDialog? 0x29074 up to the input): the panel picture `PANEL_P.PCC`, the face of `profile`
    /// (its top-left 56 x 50 at (8, 7)), the nickname centred at (36, 60), the text, the buttons (OK shaded), the
    /// dollar field with `start`.
    pub fn new(p: &mut Painter, main: &InvData, profile: u8, nick: &[u8], start: i16) -> SalaryBox {
        let nick = &nick[..nick.iter().position(|&b| b == 0).unwrap_or(nick.len())];
        let (bx, by, _, _) = BOX;
        // SPEC GAP: a missing MYBRUSH.PCC leaves the sheet as it is (zeros here: nothing drawn).
        let brush = archive_member(&main.main, "MYBRUSH.PCC").ok().and_then(|m| pcx(&m).ok())
            .map(|(w, h, px)| (0..16).flat_map(|r| (0..40).map(move |c| (r, c)))
                .map(|(r, c)| if r < h && c < w { px[r * w + c] } else { 0 }).collect())
            .unwrap_or_else(|| vec![0; 640]);
        let mut b = SalaryBox { img: vec![0; 64000], brush, offer: Offer::new(start) };
        let mut table0 = *p.table0;
        let mut q = Painter { d: p.d, buf: &mut b.img, table0: &mut table0, table29: p.table29 };
        crate::invdraw::load_pcx(&mut q, main, "PANEL_P.PCC", bx, by);
        if let Ok((w, h, px)) = archive_member(&main.main, &profile.to_string()).and_then(|m| pcx(&m)) {
            for r in 0..50.min(h) {
                for c in 0..56.min(w) {
                    q.put(bx + 8 + c as i32, by + 7 + r as i32, px[r * w + c]);
                }
            }
        }
        q.text(nick, bx + 36, by + 60, true, 0x2A, 3, 2);
        let (ls, _) = crate::msgbox::lines_wrap(&q.d.fonts[2].widths, TEXT, 129);
        for (i, l) in ls.iter().enumerate() {
            q.text(l, bx + 140, by + 13 + 11 * i as i32, true, 0x28, 8, 2);
        }
        crate::question::button(&mut q, 0, false);
        crate::question::button(&mut q, 1, false);
        shade_ok(&mut q);
        let brush = b.brush.clone();
        for k in 2..4 {
            arrow(&mut q, &brush, k, false);
        }
        field(&mut q, start);
        b
    }

    /// The whole box to the screen at (52, 37).
    pub fn show(&self, screen: &mut [u8]) {
        let (bx, by, bw, bh) = BOX;
        self.copy(screen, bx, by, bw, bh);
    }

    fn copy(&self, screen: &mut [u8], x: i32, y: i32, w: i32, h: i32) {
        for r in y..y + h {
            let o = (r * PW + x) as usize;
            screen[o..o + w as usize].copy_from_slice(&self.img[o..o + w as usize]);
        }
    }

    /// The tested button under the mouse at (x, y) (0x67CD8: the button's place + the pressed element's w, h).
    pub fn button_at(&self, x: i32, y: i32) -> Option<usize> {
        (0..4).find(|&k| self.offer.active(k) && button_hit(k, x, y))
    }

    /// The pressed (or up again) face of button k on the screen (0x35DA4 / 0x35C0C).
    pub fn face(&self, p: &mut Painter, k: usize, pressed: bool) {
        if k < 2 {
            crate::question::button(p, k, pressed);
        } else {
            arrow(p, &self.brush, k, pressed);
        }
    }

    /// A press of button k came up (0x2573C): Some(offer) ends the box (NO = 0); the box and the screen change.
    pub fn press(&mut self, p: &mut Painter, k: usize) -> Option<i16> {
        let r = self.offer.press(k);
        let (bx, by, _, _) = BOX;
        match r {
            Press::End(v) => return Some(v),
            Press::Nothing => return None,
            _ => {}
        }
        let mut table0 = *p.table0;
        {
            let mut q = Painter { d: p.d, buf: &mut self.img, table0: &mut table0, table29: p.table29 };
            match r {
                Press::Ok(false) => shade_ok(&mut q),
                Press::Ok(true) => crate::question::button(&mut q, 1, false),
                _ => {}
            }
            field(&mut q, self.offer.cur);
        }
        // 0x256B8: the OK button's 38 x 16 to the screen; 0x25414 with flag 1: the field's 46 x 16 at (122, 32).
        if matches!(r, Press::Ok(_)) {
            self.copy(p.buf, BUTTONS[1].0, BUTTONS[1].1, 38, 16);
        }
        self.copy(p.buf, bx + 122, by + 32, 46, 16);
        None
    }
}

/// The box over a screen picture with its own panel data (the window and JA1_RAISE_PNG).
pub struct Screen {
    b: SalaryBox,
    d: std::sync::Arc<crate::panel::PanelData>,
    c: std::sync::Arc<crate::data::Common>,
    table0: [u8; 256],
    /// The screen: `under` with the box.
    pub img: Vec<u8>,
}

impl Screen {
    /// The box of `profile` / `nick` with the offer `start` over `under` (320 x 200).
    pub fn new(iso: &str, exe: &str, profile: u8, nick: &[u8], start: i16, under: &[u8]) -> Result<Screen, i32> {
        let c = crate::common(iso)?;
        let d = crate::panel::panel_data(iso, exe)?;
        let inv = crate::invdraw::inv_data(iso, exe)?;
        let mut table0 = c.remap[0];
        let mut img = under.to_vec();
        img.resize(64000, 0);
        let b = {
            let mut p = Painter { d: &d, buf: &mut img, table0: &mut table0, table29: crate::panel::table29(&c) };
            let b = SalaryBox::new(&mut p, &inv, profile, nick, start);
            b.show(p.buf);
            b
        };
        Ok(Screen { b, d, c, table0, img })
    }

    pub fn offer(&self) -> Offer {
        self.b.offer
    }

    pub fn button_at(&self, x: i32, y: i32) -> Option<usize> {
        self.b.button_at(x, y)
    }

    /// The pressed (or up again) face of button k.
    pub fn face(&mut self, k: usize, pressed: bool) {
        let mut p = Painter { d: &self.d, buf: &mut self.img, table0: &mut self.table0, table29: crate::panel::table29(&self.c) };
        self.b.face(&mut p, k, pressed);
    }

    /// A press of button k came up: Some(offer) ends the box.
    pub fn press(&mut self, k: usize) -> Option<i16> {
        let mut p = Painter { d: &self.d, buf: &mut self.img, table0: &mut self.table0, table29: crate::panel::table29(&self.c) };
        self.b.press(&mut p, k)
    }
}

/// 0x2560C: OK shaded, two 19 x 16 blocks of the sheet at (242, 2) through table 0 with entry 0x1B = 0x70 (pixels
/// that map to 0 are not written).
fn shade_ok(q: &mut Painter) {
    let (x, y) = BUTTONS[1];
    let keep = q.table0[0x1B];
    q.table0[0x1B] = 0x70;
    for half in 0..2 {
        for r in 0..16 {
            for c in 0..19 {
                let v = q.table0[q.sheet(242 + c, 2 + r) as usize];
                if v != 0 {
                    q.put(x + 19 * half + c, y + r, v);
                }
            }
        }
    }
    q.table0[0x1B] = keep;
}

/// Button 0x11 (k 2) or 0x12 (k 3): element 0x2D (up) or 0x2C (pressed), 20 x 16 from MYBRUSH.PCC, then the arrow element
/// 0x19 / 0x1A at (4, 3) (pressed: (3, 2)); pixel 0 not written.
fn arrow(q: &mut Painter, brush: &[u8], k: usize, pressed: bool) {
    let (x, y) = BUTTONS[k];
    let sx = if pressed { 0 } else { 20 };
    for r in 0..16 {
        for c in 0..20 {
            let v = brush[(r * 40 + sx + c) as usize];
            if v != 0 {
                q.put(x + c, y + r, v);
            }
        }
    }
    let d = pressed as i32;
    q.element(if k == 2 { 0x19 } else { 0x1A }, x + 4 - d, y + 3 - d, None);
}

/// 0x25414: the dollar field (elements 0x13 at (122, 32) and 0x7B at (137, 32)), "$" at (124, 34) and the offer as 5
/// digits (left padding '^') at (131, 34), fg 0x28, shadow 8, font 2.
fn field(q: &mut Painter, v: i16) {
    let (bx, by, _, _) = BOX;
    q.element(0x13, bx + 0x7A, by + 0x20, None);
    q.element(0x7B, bx + 0x89, by + 0x20, None);
    q.text(b"$", bx + 0x7C, by + 0x22, false, 0x28, 8, 2);
    let mut s = (v as i32).to_string().into_bytes();
    while s.len() < 5 {
        s.insert(0, b'^');
    }
    q.text(&s, bx + 0x83, by + 0x22, false, 0x28, 8, 2);
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Ledger row ja1front:15862: only buttons 0..3 exist; a button 4 is never tested.
    #[test]
    fn only_four_buttons() {
        let o = Offer { start: 0, cur: 10, step: 5, ok: false };
        assert!(o.active(3) && !o.active(4));
    }

    #[test]
    fn steps() {
        assert_eq!([step_of(-3), step_of(499), step_of(500), step_of(999), step_of(1000)], [5, 5, 10, 10, 25]);
        assert_eq!([step_of(2499), step_of(2500), step_of(4999), step_of(5000)], [25, 50, 50, 100]);
    }

    #[test]
    fn script() {
        // OK is not tested at the start offer; up, up, down, OK = start + 5; NO = 0; no end = 0.
        assert_eq!(script_offer(400, &[1, 3, 3, 2, 1]), 405);
        assert_eq!(script_offer(400, &[3, 0]), 0);
        assert_eq!(script_offer(400, &[3, 3]), 0);
        assert_eq!(script_offer(24990, &[3, 3, 1]), 25090);
    }

    /// Ledger row c2aea50:ja1front:16671: in the window an up press does not end the box; OK ends it with the offer.
    #[test]
    fn only_ok_or_no_ends_the_window_box() {
        let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
        let (iso, exe) = (root.join("rom/JA1.gog"), root.join("build/game113b/JA.EXE"));
        if !iso.exists() || !exe.exists() {
            return;
        }
        let (iso, exe) = (iso.to_str().unwrap(), exe.to_str().unwrap());
        let mut s = Screen::new(iso, exe, 1, b"Ivan", 400, &vec![0u8; 64000]).unwrap();
        assert_eq!(s.press(3), None);
        assert_eq!(s.press(3), None);
        assert_eq!(s.press(2), None);
        assert_eq!(s.press(1), Some(405));
    }
}
