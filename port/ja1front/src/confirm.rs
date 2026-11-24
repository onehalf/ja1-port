//! ConfirmDialog 0x275E4 (panel.md "The travel animation"): the question box with a wait row in its picture frame.
//! After the answer the box holds the row of that answer (yes row or no row, none = the box ends at once).
//! `run` keeps the order of the original's calls. The window and confirm_diff both drive it, through `Host`.
use crate::invdraw::InvData;
use crate::panel::Painter;
use crate::sanim::Anim;

/// The original's three stack flags. Only the two restore flags matter here: the area is always saved in the
/// original's calls, and the port keeps the screen under the box in `Confirm::under`.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct Flags {
    /// Put the screen under the box back at the end (second flag).
    pub restore: bool,
    /// Put the mouse back at the end (third flag, 0x67E30), where it was at the start of the box.
    pub mouse: bool,
}

/// StrategicScreen (0x436C6, 0x43D93) and CorpseExportPrompt (0x463E7, 0x464B2) pass 1, 1, 1.
pub const MAP: Flags = Flags { restore: true, mouse: true };
/// AimScreen (0x20FC3, 0x2107B, 0x210BA, 0x210EB) passes 1, 0, 0.
pub const AIM: Flags = Flags { restore: false, mouse: false };

/// 0x2764B: a mouse outside the box (x 0x34..=0x10B, y 0x25..=0x74) moves to (0xED, 0x66).
pub fn open_warp(mouse: (i32, i32)) -> Option<(i32, i32)> {
    let (x, y) = mouse;
    if (0x34..=0x10B).contains(&x) && (0x25..=0x74).contains(&y) {
        None
    } else {
        Some(crate::question::MOUSE)
    }
}

/// What the box needs from its caller. The calls come in the order of the original.
pub trait Host {
    /// FormatSanimName starts a row: its sound (slot, rate 0x2B11, volume 20, loop value).
    fn sound(&mut self, slot: i32, sound_loop: i32);
    /// The mouse moves to a screen point.
    fn warp(&mut self, at: (i32, i32));
    /// 0x2538C: the box waits for the answer; `imgs` = the box with no button, NO and OK pressed.
    fn ask(&mut self, imgs: &[Vec<u8>], wait: Option<&Anim>) -> bool;
    /// PlaySanim 0x258D0: true when the frame time `ms` passed since the last frame step.
    fn due(&mut self, ms: u32) -> bool;
    /// A picture goes to the screen.
    fn show(&mut self, img: &[u8]);
    /// KeyOrClick 0x67E44: a key or a mouse button came.
    fn stopped(&mut self) -> bool;
    /// StopAllSounds 0x5CE08.
    fn stop_sounds(&mut self);
}

/// One box: its text, rows and panel picture, and the screen under it.
pub struct Confirm {
    pub iso: String,
    pub exe: String,
    pub text: Vec<u8>,
    pub panel: &'static str,
    pub wait: Option<Anim>,
    pub yes: Option<Anim>,
    pub no: Option<Anim>,
    pub flags: Flags,
    pub under: Vec<u8>,
    pub mouse: (i32, i32),
}

impl Confirm {
    fn paint(&self, img: &mut [u8], f: impl FnOnce(&mut Painter, &InvData)) -> Result<(), i32> {
        let (c, d, inv) = (crate::common(&self.iso)?, crate::panel::panel_data(&self.iso, &self.exe)?,
                           crate::invdraw::inv_data(&self.iso, &self.exe)?);
        let mut table0 = c.remap[0];
        let mut p = Painter { d: &d, buf: img, table0: &mut table0, table29: crate::panel::table29(&c) };
        f(&mut p, &inv);
        Ok(())
    }

    /// The box with no button, NO and OK pressed, frame 0 of the wait row in the picture frame.
    fn wait_imgs(&self) -> Result<Vec<Vec<u8>>, i32> {
        let mut out = Vec::new();
        for pressed in [None, Some(0), Some(1)] {
            let mut img = self.under.clone();
            self.paint(&mut img, |p, inv| {
                crate::question::draw_panel(p, inv, &self.text, None, None, self.panel);
                if let Some(k) = pressed {
                    crate::question::button(p, k, true);
                }
            })?;
            if let Some(a) = &self.wait {
                a.put(&mut img, 0);
            }
            out.push(img);
        }
        Ok(out)
    }

    /// 0x27762: the box after the answer: the panel picture, the text of row `go` and its frame 0.
    fn go_img(&self, go: &Anim) -> Result<Vec<u8>, i32> {
        let mut img = self.under.clone();
        self.paint(&mut img, |p, inv| crate::question::draw_go_on(p, inv, &go.text, self.panel))?;
        go.put(&mut img, 0);
        Ok(img)
    }
}

/// The box from its start to its end; returns the answer. `screen` = the screen at the end.
pub fn run(c: &Confirm, h: &mut dyn Host) -> Result<(bool, Vec<u8>), i32> {
    let sound = |h: &mut dyn Host, a: &Anim| {
        if a.sound != -1 {
            h.sound(a.sound as i32, a.sound_loop as i32);
        }
    };
    if let Some(a) = &c.wait {
        sound(h, a);
    }
    crate::pointer::mouse_save(c.mouse); // 0x27646
    if let Some(at) = open_warp(c.mouse) {
        h.warp(at);
    }
    let imgs = c.wait_imgs()?;
    let yes = h.ask(&imgs, c.wait.as_ref());
    let mut last = imgs[0].clone();
    if let Some(go) = if yes { &c.yes } else { &c.no } {
        sound(h, go);
        let base = c.go_img(go)?;
        h.show(&base);
        last.clone_from(&base);
        // 0x277F6: the row runs while frames - 1 > the current frame; one key or click ends it after any pass.
        let mut k = 0;
        while k < go.shown() {
            if h.due(go.ms) {
                last.clone_from(&base);
                go.put(&mut last, k);
                h.show(&last);
                k += 1;
            }
            if h.stopped() {
                break;
            }
        }
    }
    h.stop_sounds();
    if c.flags.restore {
        last.clone_from(&c.under);
    }
    if c.flags.mouse {
        h.warp(c.mouse);
    }
    Ok((yes, last))
}
