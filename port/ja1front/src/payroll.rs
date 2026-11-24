//! The payroll screen (docs/spec/strategic.md "The payroll screen (host op 301)"): the first morning step shows
//! the team's columns and the day's money; the picture slides up over the island map.
use crate::assign;
use crate::invdraw::{load_pcx, InvData};
use crate::panel::Painter;

/// A sound that starts with a shown screen: (main-bank effect, rate, volume, times).
pub type Sfx = (i32, i32, i32, i32);

/// One shown screen: the 320 x 200 pixels, the time in ms before the next screen, the sound that starts with it.
pub struct Step {
    pub screen: Vec<u8>,
    pub ms: u32,
    pub sound: Option<Sfx>,
}

/// The page sound of the first flip picture and the slide sound.
pub const PAGE_SOUND: Sfx = (4, 0x2B11, 20, 1);
pub const SLIDE_SOUND: Sfx = (5, 0x2B11, 0x58, 2);

/// The payroll picture into the painter's buffer (P): BASSMAP.PCC and the columns of the mercs in the sector.
/// `soldiers` = team slots 0..8 (9 records), `faces` = 8 face buffers of 32 x 32, `stat_changed` = 640 bytes.
pub fn picture(p: &mut Painter, main: &InvData, soldiers: &[u8], faces: &[u8], stat_changed: &[u8]) {
    load_pcx(p, main, "BASSMAP.PCC", 0, 0);
    for k in 0..assign::merc_count(soldiers) {
        assign::face(p, faces, k);
        assign::column(p, &soldiers[k * assign::SOLDIER..(k + 1) * assign::SOLDIER], k, stat_changed);
    }
}

/// PlayFlipAnim 0x51264 with "pflip", first 1, count 3, step 1, and the 60 ms wait after it: it waits 60 ms before it
/// draws each picture, so the old screen shows first and the page sound starts on the first picture.
pub fn flip_steps(p: &mut Painter, main: &InvData, screen: &[u8]) -> Vec<Step> {
    const ROW: usize = 320;
    let mut s = screen.to_vec();
    let mut out = vec![Step { screen: s.clone(), ms: 60, sound: None }];
    for n in 1..=3 {
        load_pcx(p, main, &format!("PFLIP{}.PCC", n), 0, 146);
        s[146 * ROW..].copy_from_slice(&p.buf[146 * ROW..64000]);
        out.push(Step { screen: s.clone(), ms: 60, sound: (n == 1).then_some(PAGE_SOUND) });
    }
    out
}

/// The whole screen (steps 1..7): `p` = the panel buffer P (left as the original leaves it), `screen` = S before
/// (the island map). Returns the shown screens in order; the last one is the screen at the end.
pub fn run(p: &mut Painter, main: &InvData, screen: &[u8], soldiers: &[u8], faces: &[u8], stat_changed: &[u8],
           acc: &[u8], prev: i32) -> Vec<Step> {
    const ROW: usize = 320;
    let back = screen[..143 * ROW].to_vec();
    let mut s = screen.to_vec();
    picture(p, main, soldiers, faces, stat_changed);
    let mut out = flip_steps(p, main, screen);
    s.copy_from_slice(&out[out.len() - 1].screen);
    panel(p, main, acc, prev);
    s[146 * ROW..].copy_from_slice(&p.buf[146 * ROW..64000]);
    out.push(Step { screen: s.clone(), ms: 1, sound: Some(SLIDE_SOUND) });
    for j in 0..24usize {
        let n = 136 - 6 * j as i32;
        if n >= 1 {
            let (src, n) = ((7 + 6 * j) * ROW, n as usize * ROW);
            s[3 * ROW..3 * ROW + n].copy_from_slice(&back[src..src + n]);
        }
        let top = (139 - 6 * j) * ROW;
        s[top..143 * ROW].copy_from_slice(&p.buf[top..143 * ROW]);
        out.push(Step { screen: s.clone(), ms: 1, sound: None });
    }
    out
}

/// Step 4 into P: POSTFIN.PCC at (0, 146), then payroll fields 43..51 (`prev` = `prev_balance`).
pub fn panel(p: &mut Painter, main: &InvData, acc: &[u8], prev: i32) {
    load_pcx(p, main, "POSTFIN.PCC", 0, 146);
    for k in 43..52 {
        crate::mappanel::pay_field(p, acc, prev, k);
    }
}
