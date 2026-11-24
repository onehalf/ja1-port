//! The office's switch box (strategic.md "The switch box (Control Panel)"): lever switches for the 16 Control Panel
//! switches, opened from hot spot 6. The box logs what it shows and plays as `Step`s; the window shows them, the
//! script writes lines and the last picture.

use crate::office::{Pic, OfficeData};
use crate::panel::Painter;

const PW: usize = 320;
/// The 50 x 74 door at (0, 9), frames j at PANBRUSH (50 j, 0).
const DOOR: (i32, i32, i32, i32) = (0, 9, 50, 74);
/// The close lever's place and its 9 frames (22 x 130 at PANBRUSH (200 + 22 f, 0)).
const LEVER: (i32, i32) = (244, 31);
/// Main bank effects: the switch, the close lever, the door going off.
pub const FX_SWITCH: i32 = 2;
pub const FX_LEVER: i32 = 3;
pub const FX_DOOR: i32 = 0;

/// One thing the box shows or plays, in order.
#[derive(Clone, Debug, PartialEq)]
pub enum Step {
    /// A screen picture (320 x 200), shown at least `ms` milliseconds (0 = no wait).
    Show(Vec<u8>, u32),
    /// A main bank effect (effect, rate, volume), once; heard = Sound FX is on when it starts.
    Sfx(i32, i32, i32, bool),
    /// Music switched on (true) or off; `act` = a theme starts (on) or stops (off).
    Music(bool, bool),
}

/// The place of switch i (0..19).
pub fn place(i: usize) -> (i32, i32) {
    if i < 10 { (28, 26 + 15 * i as i32) } else { (137, 26 + 15 * (i as i32 - 10)) }
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

/// Switch i with value v on `p.buf`: the lever, and for i < 16 the lamp and the name.
fn draw_switch(d: &OfficeData, p: &mut Painter, i: usize, v: u8) {
    let (x, y) = place(i);
    part(p.buf, &d.panbrush, if v == 0 { 26 } else { 0 }, 74, 26, 7, x, y);
    if i < 16 {
        part(p.buf, &d.panbrush, if v == 0 { 52 } else { 58 }, 74, 6, 4, x + 10, y + 7);
        p.text(crate::ctlpanel::NAMES[i], x + 30, y + 2, false, if v == 0 { 0x44 } else { 0x28 }, 3, 1);
    }
}

/// Nearest scaling of the 282 x 200 box `src` (pitch 320) to w x h at (0, y) of `dst` (speech balloon rule).
fn zoom(dst: &mut [u8], src: &[u8], w: u32, h: u32, y: usize) {
    let (sx, sy) = ((282u32 << 22) / w, (200u32 << 22) / h);
    for j in 0..h {
        let row = (sy / 2).wrapping_add(j.wrapping_mul(sy)) >> 22;
        for i in 0..w {
            let col = (sx / 2).wrapping_add(i.wrapping_mul(sx)) >> 22;
            if let Some(px) = dst.get_mut((y + j as usize) * PW + i as usize) {
                *px = src[row as usize * PW + col as usize];
            }
        }
    }
}

/// The open box.
pub struct SwitchBox {
    /// The picture on the screen (P).
    pub pic: Vec<u8>,
    /// The room after the door opened (R).
    room: Vec<u8>,
    /// The 50 x 74 rectangle under the door (U), row by row.
    under: Vec<u8>,
}

impl SwitchBox {
    /// Opening steps 1..5 on the office picture `s` (the room with its highlight, no label); `opts` = the 20 bytes.
    pub fn open(d: &OfficeData, p: &mut Painter, opts: &mut [u8; 20], steps: &mut Vec<Step>) -> SwitchBox {
        let (dx, dy, dw, dh) = DOOR;
        let mut s = p.buf.to_vec();
        let mut under = Vec::with_capacity((dw * dh) as usize);
        for r in 0..dh {
            let o = ((dy + r) * PW as i32 + dx) as usize;
            under.extend_from_slice(&s[o..o + dw as usize]);
        }
        for j in 0..4 {
            part(&mut s, &d.panbrush, 50 * j, 0, dw, dh, dx, dy);
            steps.push(Step::Show(s.clone(), 90));
        }
        let room = s.clone();
        part(&mut s, &d.swbox, 0, 0, 282, 200, 0, 0);
        for i in 0..20 {
            if opts[i] > 1 {
                opts[i] = 0;
            }
            p.buf.copy_from_slice(&s);
            draw_switch(d, p, i, opts[i]);
            s.copy_from_slice(p.buf);
        }
        let boxp = s;
        let mut z = room.clone();
        let mut y = 24i32;
        for st in 1..=16u32 {
            zoom(&mut z, &boxp, 4 + 17 * st, 20 + 11 * st, y as usize);
            steps.push(Step::Show(z.clone(), 0));
            if y > 0 {
                y = (y - 4).max(0);
            }
        }
        for r in 0..200 {
            z[r * PW..r * PW + 282].copy_from_slice(&boxp[r * PW..r * PW + 282]);
        }
        p.buf.copy_from_slice(&z);
        steps.push(Step::Show(z.clone(), 0));
        SwitchBox { pic: z, room, under }
    }

    /// A left press at (x, y). `sound` / `music` = a sound / music driver is installed; `wanted` / `playing` = the
    /// music's wanted and playing themes. Returns the toggled switches (k, new value) and true at the close lever.
    #[allow(clippy::too_many_arguments)]
    pub fn press(&mut self, d: &OfficeData, p: &mut Painter, x: i32, y: i32, opts: &mut [u8; 20], sound: bool,
                 music: bool, wanted: i32, playing: i32, steps: &mut Vec<Step>) -> (Vec<(usize, u8)>, bool) {
        let mut done = vec![];
        let hit = (0..16).find(|&k| {
            let (sx, sy) = place(k);
            sx <= x && x <= sx + 26 && sy <= y && y <= sy + 7
        });
        if let Some(k) = hit {
            if (k != 1 || music) && (k != 0 || sound) {
                if k == 0 && opts[0] != 0 && opts[5] == 0 {
                    done.push(self.toggle(d, p, 5, opts, wanted, playing, steps));
                }
                if k != 5 || opts[0] != 0 {
                    done.push(self.toggle(d, p, k, opts, wanted, playing, steps));
                }
            }
        }
        let (lx, ly) = LEVER;
        if lx <= x && x <= lx + 22 && ly <= y && y <= ly + 65 {
            steps.push(Step::Sfx(FX_LEVER, 0x2B11, 20, opts[0] != 0));
            let mut s = self.pic.clone();
            for f in 0..9 {
                part(&mut s, &d.panbrush, 200 + 22 * f, 0, 22, 130, lx, ly);
                steps.push(Step::Show(s.clone(), 90));
            }
            self.pic = s;
            return (done, true);
        }
        (done, false)
    }

    #[allow(clippy::too_many_arguments)]
    fn toggle(&mut self, d: &OfficeData, p: &mut Painter, k: usize, opts: &mut [u8; 20], wanted: i32, playing: i32,
              steps: &mut Vec<Step>) -> (usize, u8) {
        // The switch sound starts with the old Sound FX value (not heard when it is off).
        steps.push(Step::Sfx(FX_SWITCH, 0x2B11, 20, opts[0] != 0));
        opts[k] ^= 1;
        let v = opts[k];
        p.buf.copy_from_slice(&self.pic);
        draw_switch(d, p, k, v);
        self.pic.copy_from_slice(p.buf);
        if k == 1 {
            steps.push(Step::Music(v != 0, if v != 0 { wanted > 0 } else { playing > 0 }));
        }
        if k == 0 && v != 0 {
            steps.push(Step::Sfx(FX_SWITCH, 0x2B11, 20, true));
        }
        steps.push(Step::Show(self.pic.clone(), 0));
        (k, v)
    }

    /// Close steps 1 and 2 with `plain` = the office picture (the room, no highlight); `sound_fx` = switch 0.
    pub fn close(self, d: &OfficeData, plain: &[u8], sound_fx: bool, steps: &mut Vec<Step>) {
        let mut scr = self.pic;
        for r in 0..200 {
            scr[r * PW..r * PW + 318].copy_from_slice(&self.room[r * PW..r * PW + 318]);
        }
        steps.push(Step::Show(scr, 0));
        steps.push(Step::Sfx(FX_DOOR, 8000, 88, sound_fx));
        let (dx, dy, dw, dh) = DOOR;
        let mut q = plain.to_vec();
        for j in (0..4).rev() {
            part(&mut q, &d.panbrush, 50 * j, 0, dw, dh, dx, dy);
            steps.push(Step::Show(q.clone(), 90));
        }
        for r in 0..dh {
            let o = ((dy + r) * PW as i32 + dx) as usize;
            q[o..o + dw as usize].copy_from_slice(&self.under[(r * dw) as usize..((r + 1) * dw) as usize]);
        }
        steps.push(Step::Show(q, 0));
    }
}
