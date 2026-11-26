//! The tactical Control Panel (panel.md "The Control Panel (key F10, `opt` button)"): 16 switches as text over the
//! tactical screen. The logic (`Panel::pass`) changes the world; it logs its drawing as `Draw` steps, which
//! `paint` puts on a screen (the window paints each pass, the script paints the whole log on its PNG).

use crate::panel::Painter;
use crate::world::World;

/// The switch names (font 1).
pub const NAMES: [&[u8]; 16] = [b"Sound FX", b"Music", b"Fast Scroll", b"Alt Scroll", b"Shadows", b"Subtitles",
    b"Item Info", b"Show Paths", b"Pal Cycling", b"Fast Anims", b"Follow move", b"Step Sounds", b"Day Replays",
    b"Fast Firing", b"Safety Move", b"Merc OKs"];
/// World image offset of switch 0; byte 21554 is the first of the four bytes after the 16.
pub const OPT_BASE: usize = 21538;
const BUTTONS: [(&[u8], i32, i32); 2] = [(b"Quick Save", 55, 165), (b"Quick Restore", 170, 165)];
/// Results: Continue Game, Quick Save, Quick Restore, Escape.
pub const CONTINUE: i32 = 16;
pub const QUICK_SAVE: i32 = 100;
pub const QUICK_RESTORE: i32 = 101;

/// ControlPanel 0x4BA61..0x4BA94 after the input loop: a close result of 100 or more runs QuickSave 0x2D880 (100,
/// `Some(true)`) or QuickRestore 0x2C898 (any other, `Some(false)`); a result below 100 (Escape, Continue Game)
/// runs neither. The panel itself is a Lock user (clock switch 0, 0x4B7FD .. 0x4B831; Unlock 0x4BA25 .. 0x4BA48):
/// its callers run its open time as locked ticks (`PlayState::locked_ticks`).
pub fn after_close(r: i32) -> Option<bool> {
    if (r as i16) < 100 {
        None
    } else {
        Some(r as i16 == QUICK_SAVE as i16)
    }
}
pub const ESCAPE: i32 = -1;
/// The map view the mouse stays in while the panel is open: 0x4B854 sets the mouse limit to the view window (0x229CB8,
/// 0x229CC4, 0x229CD8, 0x229CDC of SetViewWindow 0x24880), so it is (48, 14, 271, 181) only in the normal view.
pub fn mouse_box(view_w: i32, view_h: i32, onscreen: i32) -> (i32, i32, i32, i32) {
    let left = if view_w < 320 { 48 } else { 0 };
    let top = if onscreen == 3 { 0 } else { 14 };
    (left, top, left - 1 + view_w, top - 1 + view_h)
}

/// Insert #13554 item 1: the Subtitles switch (5) as the text of speech reads it: the world's switch, or on with the
/// port option subtitles = on (config.rs, JA1_SUBTITLES=1). The switch byte stays as the game set it (saves keep it).
pub fn subtitles(w: &World) -> bool {
    sub_byte(w.update.opt_step_text)
}

/// The same for a copy of the switch byte (0x1163BD).
pub fn sub_byte(b: u8) -> bool {
    b != 0 || crate::iteminfo::subtitles()
}

/// Switch k of the world (0 / 1). Eight switches are world fields, the others stay in the save image.
pub fn get(w: &World, k: usize) -> u8 {
    match k {
        5 => w.update.opt_step_text,
        7 => w.update.opt_path_marks,
        8 => w.update.opt_ambient,
        9 => w.update.opt_half_delay,
        10 => w.update.opt_follow,
        11 => w.opt_sound,
        14 => w.opt_confirm_move,
        15 => w.update.opt_move_fx,
        _ => w.base.get(OPT_BASE + k).copied().unwrap_or(0),
    }
}

/// Switch 0 "Sound FX" (NAMES[0]) is off: the sound host then plays no effect (PlayVocSample's gate, g_options 0x1163B8).
pub fn fx_off(w: &World) -> bool {
    get(w, 0) == 0
}

pub fn set(w: &mut World, k: usize, v: u8) {
    match k {
        5 => w.update.opt_step_text = v,
        7 => w.update.opt_path_marks = v,
        8 => w.update.opt_ambient = v,
        9 => w.update.opt_half_delay = v,
        10 => w.update.opt_follow = v,
        11 => w.opt_sound = v,
        14 => w.opt_confirm_move = v,
        15 => w.update.opt_move_fx = v,
        _ => {
            if let Some(b) = w.base.get_mut(OPT_BASE + k) {
                *b = v;
            }
        }
    }
}

/// savegame.md "Which loads take Sound FX and Music from the file": the switches Sound FX and Music (0 and 1) of a
/// world, kept before a day save load and put back after it with `put_sound_switches`.
pub fn sound_switches(w: &World) -> [u8; 2] {
    [get(w, 0), get(w, 1)]
}

pub fn put_sound_switches(w: &mut World, v: [u8; 2]) {
    set(w, 0, v[0]);
    set(w, 1, v[1]);
}

/// Byte 21554 (the shadow of a hovered "Continue Game").
fn extra0(w: &World) -> u8 {
    w.base.get(OPT_BASE + 16).copied().unwrap_or(0)
}

/// The place of item k (0..15 switches, 16 Continue Game).
pub fn place(k: usize) -> (i32, i32) {
    match k {
        0..=7 => (55, 25 + 13 * k as i32),
        8..=15 => (170, 25 + 13 * (k as i32 - 8)),
        _ => (120, 142),
    }
}

/// One drawing step.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Draw {
    /// Switch name / Continue Game (k = 16).
    Name { k: usize, fg: u8, shadow: u8 },
    /// A switch's ON / OFF text.
    Value { k: usize, v: u8, fg: u8, shadow: u8 },
    /// Button j.
    Button { j: usize, fg: u8 },
    /// The 20 x 13 value area of switch k gets the picture under the panel back.
    Restore { k: usize },
}

/// One drawing call of a step: DrawText with (text, x, y, fg, shadow), font 1, or the 20 x 13 copy at (x, y).
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Call {
    Text { s: &'static [u8], x: i32, y: i32, fg: u8, shadow: u8 },
    Copy { x: i32, y: i32 },
}

impl Draw {
    /// The drawing call of this step.
    pub fn call(&self) -> Call {
        match *self {
            Draw::Name { k, fg, shadow } => {
                let (x, y) = place(k);
                Call::Text { s: if k < 16 { NAMES[k] } else { b"Continue Game" }, x, y, fg, shadow }
            }
            Draw::Value { k, v, fg, shadow } => {
                let (x, y) = place(k);
                Call::Text { s: if v != 0 { b"ON" } else { b"OFF" }, x: x + 75, y, fg, shadow }
            }
            Draw::Button { j, fg } => {
                let (s, x, y) = BUTTONS[j];
                Call::Text { s, x, y, fg, shadow: 8 }
            }
            Draw::Restore { k } => {
                let (x, y) = place(k);
                Call::Copy { x: x + 75, y }
            }
        }
    }
}

/// Paint the steps on `buf` (320 x 200); `under` = the screen before the panel.
pub fn paint(p: &mut Painter, under: &[u8], steps: &[Draw]) {
    for d in steps {
        match d.call() {
            Call::Text { s, x, y, fg, shadow } => p.text(s, x, y, false, fg, shadow, 1),
            Call::Copy { x, y } => {
                for (px, py, v) in under_block(under, x, y) {
                    p.put(px, py, v);
                }
            }
        }
    }
}

/// The 20 x 13 block of `under` (the screen before the panel) at (x, y), row by row: what a Restore step puts back.
pub fn under_block(under: &[u8], x: i32, y: i32) -> Vec<(i32, i32, u8)> {
    let mut out = Vec::new();
    for r in 0..13 {
        for c in 0..20 {
            let (px, py) = (x + c, y + r);
            if let Some(&v) = under.get((py * 320 + px) as usize) {
                out.push((px, py, v));
            }
        }
    }
    out
}

#[cfg(test)]
mod under_tests {
    /// Ledger row c2aea50:ja1front:690: the Sound FX switch (0) on means effects play; off means the sound host is quiet.
    #[test]
    fn the_sound_fx_switch_sets_fx_off() {
        let (exe, jag, save) = crate::world::tests::files(&[]);
        let mut w = crate::world::World::from_files(&exe, &jag, &save).unwrap();
        super::set(&mut w, 0, 1);
        assert!(!super::fx_off(&w));
        super::set(&mut w, 0, 0);
        assert!(super::fx_off(&w));
    }

    /// Ledger rows c2aea50:ja1front:5903/5904/5905: a Restore step puts back the 20 x 13 pixels of the old screen
    /// under its own place.
    #[test]
    fn restore_puts_back_the_block_under_it() {
        let under: Vec<u8> = (0..64000u32).map(|i| (i % 251) as u8).collect();
        let b = super::under_block(&under, 100, 50);
        assert_eq!(b.len(), 260);
        for &(x, y, v) in &b {
            assert!((100..120).contains(&x) && (50..63).contains(&y), "{x} {y}");
            assert_eq!(v, under[(y * 320 + x) as usize]);
        }
        assert_eq!(b[0], (100, 50, under[50 * 320 + 100]));
        assert_eq!(b[259], (119, 62, under[62 * 320 + 119]));
    }
}

/// What one pass asks of the caller.
#[derive(Default, Debug, PartialEq)]
pub struct PassOut {
    /// Click sounds to play (tactical effect slot 0, 11025 Hz, volume 20, once).
    pub clicks: u8,
    /// The panel closes with this result.
    pub close: Option<i32>,
    /// The Music switch changed: Some(new value).
    pub music: Option<bool>,
    /// Show Paths went off: hide the path marks.
    pub hide_marks: bool,
    /// The switch that toggled.
    pub toggled: Option<usize>,
}

pub struct Panel {
    /// Quick Save / Quick Restore shown.
    pub s: bool,
    pub r: bool,
    /// A sound / music driver is installed: without one the Sound FX / Music switch refuses the hover
    /// (ControlPanelInput 0x4BAA4, driver bytes 0x96399 / 0x9639A).
    pub sound: bool,
    pub music: bool,
    /// The hover item (-1 none, 0..16, 100 + j).
    pub hover: i32,
    /// Drawing steps not yet painted.
    pub log: Vec<Draw>,
}

fn normal(w: &World, k: i32, out: &mut Vec<Draw>) {
    match k {
        0..=15 => {
            let v = get(w, k as usize);
            out.push(Draw::Name { k: k as usize, fg: 0xFF, shadow: 8 });
            out.push(Draw::Value { k: k as usize, v, fg: if v != 0 { 0xFF } else { 0x6F }, shadow: v });
        }
        16 => out.push(Draw::Name { k: 16, fg: 0xFF, shadow: 8 }),
        100 | 101 => out.push(Draw::Button { j: (k - 100) as usize, fg: 0xFF }),
        _ => {}
    }
}

impl Panel {
    /// Opening steps 1, 3 and 4: `quick_save` = save byte 21285 (quick save in combat) or the save-anywhere byte,
    /// `restore` = QUICK.SAV exists; `sound` / `music` = a sound / music driver is installed.
    pub fn open(w: &World, quick_save: bool, restore: bool, sound: bool, music: bool) -> Panel {
        let s = w.combat == 0 || quick_save;
        let mut log = Vec::new();
        for k in 0..17 {
            normal(w, k, &mut log);
        }
        if s {
            normal(w, 100, &mut log);
        }
        if restore {
            normal(w, 101, &mut log);
        }
        Panel { s, r: restore, sound, music, hover: -1, log }
    }

    /// The valid target under the mouse (pass steps 1 and 2).
    pub fn target(&self, w: &World, mx: i32, my: i32) -> i32 {
        let hit = |x0: i32, y0: i32, x1: i32, y1: i32| mx >= x0 && mx <= x1 && my >= y0 && my <= y1;
        let mut t = -1;
        for k in 0..17 {
            let (x, y) = place(k);
            if hit(x, y - 4, x + 95, y + 9) {
                let (fx, sub) = (get(w, 0) != 0, get(w, 5) != 0);
                let refused = (k == 1 && !self.music) || (k == 0 && (!self.sound || (fx && !sub)))
                    || (k == 5 && sub && !fx);
                if !refused {
                    t = k as i32;
                }
                break;
            }
        }
        for (j, &(_, x, y)) in BUTTONS.iter().enumerate() {
            if hit(x, y - 4, x + 80, y + 5) && (if j == 0 { self.s } else { self.r }) {
                t = 100 + j as i32;
            }
        }
        t
    }

    /// One pass with the mouse at (mx, my) (held in `mouse_box`), `left` = a left press (its release comes before the
    /// click acts), `esc` = Escape.
    pub fn pass(&mut self, w: &mut World, mx: i32, my: i32, left: bool, esc: bool) -> PassOut {
        let (b0, b1, b2, b3) = mouse_box(w.view_w, w.view_h, w.opt_onscreen as i32);
        let (mx, my) = (mx.clamp(b0, b2), my.clamp(b1, b3));
        let mut out = PassOut::default();
        let t = self.target(w, mx, my);
        if t != self.hover {
            normal(w, self.hover, &mut self.log);
            match t {
                0..=15 => {
                    let v = get(w, t as usize);
                    self.log.push(Draw::Name { k: t as usize, fg: 0x16, shadow: 8 });
                    self.log.push(Draw::Value { k: t as usize, v, fg: 0x16, shadow: v });
                }
                16 => self.log.push(Draw::Name { k: 16, fg: 0x16, shadow: extra0(w) }),
                100 | 101 => self.log.push(Draw::Button { j: (t - 100) as usize, fg: 0x16 }),
                _ => {}
            }
            self.hover = t;
        }
        let h = self.hover;
        if left && h != -1 {
            out.clicks += 1;
            if h >= 16 {
                out.close = Some(h);
            }
        }
        if left && (0..16).contains(&h) {
            let k = h as usize;
            if k == 7 && get(w, 7) != 0 {
                out.hide_marks = true;
            }
            let v = get(w, k) ^ 1;
            set(w, k, v);
            self.log.push(Draw::Restore { k });
            // After a toggle the shadow is 8 (0x4BF37); a hover or plain draw uses the value (0x4BDA3, 0x4BC99).
            self.log.push(Draw::Value { k, v, fg: 0x16, shadow: 8 });
            if k == 1 {
                out.music = Some(v != 0);
            }
            if k == 0 && v != 0 {
                out.clicks += 1;
            }
            out.toggled = Some(k);
        }
        if esc {
            out.close = Some(ESCAPE);
        }
        out
    }
}
