//! panel.md "The travel animation": the animation rows of the "Get to work?" box (`DAT\SANIM2.DAT`).

use crate::iso::Iso;
use crate::panel::{archive_member, pcx, PW};
use std::path::Path;

/// The rows table and the texts in JA.EXE (file offsets).
const ROWS_AT: usize = 0xDB4DE;
const TEXTS_AT: usize = 0xDD8C3;
/// The frame's place on the screen: the box (52, 37) + (12, 13).
pub const AT: (usize, usize) = (64, 50);

/// One animation row with its frames.
pub struct Anim {
    pub w: usize,
    pub h: usize,
    /// The frame time in ms.
    pub ms: u32,
    /// The frame count n (frames 0 .. n − 2 are shown).
    pub n: usize,
    /// The sound slot (−1 = none) and its loop value.
    pub sound: i16,
    pub sound_loop: u16,
    /// Frames 0 .. n − 1, w × h bytes each (a frame past the pictures is empty).
    pub frames: Vec<Vec<u8>>,
    /// The row's text (no terminating 0); empty = none.
    pub text: Vec<u8>,
}

impl Anim {
    /// Frame k (0-based) into the 320-wide `img` at `AT`.
    pub fn put(&self, img: &mut [u8], k: usize) {
        let Some(f) = self.frames.get(k) else { return };
        for y in 0..self.h.min(200 - AT.1) {
            let o = (AT.1 + y) * PW as usize + AT.0;
            let n = self.w.min(PW as usize - AT.0);
            img[o..o + n].copy_from_slice(&f[y * self.w..y * self.w + n]);
        }
    }

    /// The number of frame steps a row shows: n − 1 (frame n − 1 never shows).
    pub fn shown(&self) -> usize {
        self.n.saturating_sub(1)
    }
}

/// The day start's archive (the other one, `SANIM.DAT`, serves other screens; both hold pictures 37 .. 41).
pub const DAY_ARC: &str = "SANIM2.DAT";

/// The archive `name` in `DAT/` beside JA.EXE, else on the CD.
fn archive(iso: &str, exe: &str, name: &str) -> Result<Vec<u8>, i32> {
    let dir = Path::new(exe).parent().map(|p| p.to_path_buf()).unwrap_or_default();
    if let Ok(d) = std::fs::read(dir.join("DAT").join(name)) {
        return Ok(d);
    }
    let mut cd = Iso::open(iso).map_err(|_| crate::E_FILE)?;
    cd.read(&format!("DAT/{}", name)).map_err(|_| crate::E_FILE)
}

/// Row `r` with its frames from archive `arc` (`DAY_ARC` for the day start); `number` (when given) goes as "%2d"
/// into characters 22 and 23 of the text.
pub fn load(iso: &str, exe: &str, arc: &str, r: i32, number: Option<i32>) -> Result<Anim, i32> {
    let x = std::fs::read(exe).map_err(|_| crate::E_FILE)?;
    if !(0..256).contains(&r) {
        return Err(crate::E_ARG);
    }
    let row = x.get(ROWS_AT + 12 * r as usize..ROWS_AT + 12 * r as usize + 12).ok_or(crate::E_FILE)?;
    let (i, m, c, n, w, h) = (row[0] as usize, row[1] as usize, row[2] as usize, row[3] as usize, row[4] as usize,
                              row[5] as usize);
    let ms = u16::from_le_bytes([row[6], row[7]]) as u32;
    let sound = i16::from_le_bytes([row[8], row[9]]);
    let sound_loop = u16::from_le_bytes([row[10], row[11]]);
    let t = x.get(TEXTS_AT + 40 * i..TEXTS_AT + 40 * i + 40).ok_or(crate::E_FILE)?;
    let mut text = t.to_vec();
    if let Some(v) = number {
        let s = format!("{:2}", v).into_bytes();
        text[22] = s[0];
        text[23] = s[1];
    }
    text.truncate(text.iter().position(|&b| b == 0).unwrap_or(40));
    let arc = archive(iso, exe, arc)?;
    let mut frames = Vec::with_capacity(n);
    let mut pic: Option<(usize, usize, usize, Vec<u8>)> = None;
    for k in 0..n {
        let mut f = vec![0u8; w * h];
        if c > 0 {
            let j = m + k / c;
            if pic.as_ref().map(|p| p.0) != Some(j) {
                pic = archive_member(&arc, &format!("SANIM.{:03}", j)).ok()
                    .and_then(|d| pcx(&d).ok()).map(|(pw, ph, px)| (j, pw, ph, px));
            }
            if let Some((_, pw, ph, px)) = &pic {
                cut_frame(&mut f, w, h, px, *pw, *ph, w * (k % c));
            }
        }
        frames.push(f);
    }
    Ok(Anim { w, h, ms, n, sound, sound_loop, frames, text })
}

/// Frame `f` (w x h) = the w x h part at column `x0` of the picture `px` (pw x ph); what lies past the picture's
/// right or bottom edge stays 0.
fn cut_frame(f: &mut [u8], w: usize, h: usize, px: &[u8], pw: usize, ph: usize, x0: usize) {
    for y in 0..h.min(ph) {
        for xx in 0..w {
            if x0 + xx < pw {
                f[y * w + xx] = px[y * pw + x0 + xx];
            }
        }
    }
}

#[cfg(test)]
mod put_tests {
    use super::*;

    /// A frame that reaches past the picture's right edge keeps 0 there (no pixel of the next picture row).
    #[test]
    fn cut_frame_stops_at_the_picture_edge() {
        let (pw, ph) = (5, 2);
        let px: Vec<u8> = (1..=10).collect();
        let mut f = vec![0u8; 3 * 3];
        cut_frame(&mut f, 3, 3, &px, pw, ph, 3);
        assert_eq!(f, vec![4, 5, 0, 9, 10, 0, 0, 0, 0]);
    }

    /// Ledger row ja1front:13766: a frame wider than the room right of AT is cut at the screen's right edge.
    #[test]
    fn put_cuts_at_the_right_edge() {
        let w = PW as usize;
        let a = Anim { w, h: 1, ms: 0, n: 2, sound: -1, sound_loop: 0, frames: vec![vec![1; w]], text: Vec::new() };
        let mut img = vec![0u8; PW as usize * 200];
        a.put(&mut img, 0);
        let o = AT.1 * w;
        assert!(img[o + AT.0..o + w].iter().all(|&b| b == 1));
        assert!(img[o + w..].iter().all(|&b| b == 0));
    }
}
