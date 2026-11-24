//! The drown and injection popups (host op 71, LoadPopups? 0x288DC): a picture that grows over a
//! merc's slot when he falls in deep water (flag 1, anim word 0x1D5) or a collapsed merc revives
//! with a medical item (flag 0 "stab", anim word 0x1D6).
//!
//! The original: POPUPS.DAT record = 50 frame bytes at flag x 50 (0xFF ends); FRAME.PCC (FRAMEG.PCC
//! when the slot is the selected slot) into a 120 x 60 buffer (pitch 120); DAT\STAB.DAT /
//! DAT\DROWN.DAT member STAB%02d.PCC / DROWN%02d.PCC (116 x 56) over it at (2, 2); then per frame the
//! buffer scales to w x h (w 24..120 step 4, h 12..60 step 2) at the slot place and goes to the
//! screen. The frame pictures, the place and the growth are the port; the scaling (0x52B7C) is the
//! shared `tilepopup::scale`.
use crate::panel::{archive_member, pcx, slot_origin};
use std::io;
use std::path::Path;

pub const W: i32 = 120;
pub const H: i32 = 60;
/// The frame picture offset inside the 120 x 60 buffer: (2, 2).
const FRAME_OFF: usize = 0xF2;

fn err(msg: String) -> io::Error {
    io::Error::new(io::ErrorKind::InvalidData, msg)
}

fn game_dir(exe: &str) -> &Path {
    Path::new(exe).parent().unwrap_or_else(|| Path::new(""))
}

/// The base frame name: FRAMEG.PCC when the popup's slot is the selected slot (0x284F11), else FRAME.PCC.
fn base_name(slot: u8, selected: u8) -> &'static str {
    if slot == selected {
        "FRAMEG.PCC"
    } else {
        "FRAME.PCC"
    }
}

/// The DAT archive and member prefix of flag 0 (STAB) or 1 (DROWN).
fn frame_arc(flag: i32) -> (&'static str, &'static str) {
    if flag == 1 {
        ("DROWN.DAT", "DROWN")
    } else {
        ("STAB.DAT", "STAB")
    }
}

/// The POPUPS.DAT frame list of flag 0 / 1 (50 bytes at flag x 50; the list stops at the first 0xFF).
pub fn frame_list(exe: &str, flag: i32) -> io::Result<Vec<u8>> {
    let data = std::fs::read(game_dir(exe).join("DAT").join("POPUPS.DAT"))?;
    let off = flag.max(0) as usize * 50;
    let rec = data.get(off..off + 50).unwrap_or(&[]);
    Ok(rec.iter().take_while(|&&b| b != 0xFF).copied().collect())
}

/// One frame of the popup: the 120 x 60 buffer scaled to w x h at (x, y) on the screen.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Frame {
    pub x: i32,
    pub y: i32,
    pub w: i32,
    pub h: i32,
    /// The 120 x 60 source block (base frame + frame picture).
    pub src: Vec<u8>,
}

/// The frames of the popup (LoadPopups? 0x288DC): the base frame and the frame pictures, scaled by the
/// growing w / h, placed by the slot (`selected` = 0x284F11 picks FRAMEG.PCC). Returns the frames in
/// play order.
pub fn frames(exe: &str, slot: u8, selected: u8, flag: i32) -> io::Result<Vec<Frame>> {
    let dir = game_dir(exe);
    let main = std::fs::read(dir.join("DAT").join("MAIN.DAT"))?;
    let (bw, bh, mut buf) = pcx(&archive_member(&main, base_name(slot, selected))?)?;
    if bw != W as usize || bh != H as usize {
        return Err(err(format!("{} is {} x {}, want {} x {}", base_name(slot, selected), bw, bh, W, H)));
    }
    let (arc_name, prefix) = frame_arc(flag);
    let arc = std::fs::read(dir.join("DAT").join(arc_name))?;
    let list = frame_list(exe, flag)?;
    let (sx, sy) = slot_origin(slot);
    let (cx, cy) = (sx + 24, sy + 23);
    let (mut w, mut h) = (20i32, 10i32);
    let mut out = Vec::with_capacity(list.len());
    for n in list {
        let name = format!("{}{:02}.PCC", prefix, n);
        let (fw, fh, px) = pcx(&archive_member(&arc, &name)?)?;
        for y in 0..fh {
            for x in 0..fw {
                buf[y * W as usize + x + FRAME_OFF] = px[y * fw + x];
            }
        }
        w = (w + 4).min(W);
        h = (h + 2).min(H);
        let mut x = cx - w / 2;
        if slot < 4 {
            if x < 10 {
                x = 10;
            }
        } else if slot != 4 {
            x = 0x136 - w;
        }
        let y = cy - h / 2;
        out.push(Frame { x, y, w, h, src: buf.clone() });
    }
    Ok(out)
}

/// The scaled w x h pixels of `src` (120 x 60) at (x, y) on a 320 x 200 screen, written linearly as
/// the original ScaledBlit does (dst = screen + x + y * 320, w bytes each row): a row that runs past
/// column 319 continues at column 0 of the next row (slot 4 reaches this; its x = 296 - w / 2).
/// Writes outside the screen buffer are dropped.
pub fn blit(frame: &Frame, screen: &mut [u8]) {
    let scaled = crate::tilepopup::scale(&frame.src, W, H, frame.w, frame.h);
    let pw = crate::panel::PW as i64;
    for r in 0..frame.h.max(0) {
        for c in 0..frame.w.max(0) {
            let o = (frame.y + r) as i64 * pw + (frame.x + c) as i64;
            if o >= 0 && (o as usize) < screen.len() {
                screen[o as usize] = scaled[(r * frame.w + c) as usize];
            }
        }
    }
}
