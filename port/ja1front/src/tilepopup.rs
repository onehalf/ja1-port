//! Tile popups (docs/spec/cursor.md "Tile popups"): a ground item zooms up from its tile, or a sign shows its text,
//! after a right click in mode 3 / on a sign value, or the key Z.
use crate::panel::{Painter, PW};

/// The popup of one tile, worked out from the world (`which`).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Popup {
    /// The item popup: the tile's screen point (sx, sy), raised object (t1 > 0x74), item picture, the count text
    /// (Some when it is drawn).
    Item { sx: i32, sy: i32, raised: bool, pic: u8, count: Option<i32> },
    /// The sign popup of sector `sector` at the tile's screen point.
    Sign { sx: i32, sy: i32, sector: i32 },
}

/// The view terms: view_x, view_y (map pixels of the window corner), view left, view top, view_w, view_h.
pub type View = (i32, i32, i32, i32, i32, i32);

/// "Which popup" for tile `t`: `tiles` = the 56 x 56 x 8 tile map, `ground` = the 200 ground entries (12 bytes),
/// `item_types` = the item table (12 bytes per type). None when nothing shows (also off the window).
pub fn which(tiles: &[u8], ground: &[u8], item_types: &[u8], t: i32, v: View, sector: i32) -> Option<Popup> {
    if !(0..3136).contains(&t) {
        return None;
    }
    let tb = |i: usize| tiles.get(8 * t as usize + i).copied().unwrap_or(0);
    let (x, y) = (16 * (t % 56), 14 * (t / 56));
    let (vx, vy, left, top, vw, vh) = v;
    let shown = vx <= x && x < vx + vw && vy <= y && y < vy + vh;
    let (sx, sy) = (x - vx + left, y - vy + top);
    let g = tb(6) as usize;
    if g < 200 {
        let e = ground.get(12 * g..12 * g + 12)?;
        if e[0] == 0 || e[6] as i8 <= 0 || !shown {
            return None;
        }
        let it = |i: usize| item_types.get(12 * e[0] as usize + i).copied().unwrap_or(0);
        let c = e[1] as i8 as i32;
        Some(Popup::Item { sx, sy, raised: tb(1) > 0x74, pic: it(4), count: (c > 1 && it(6) > 1).then_some(c) })
    } else if tb(1) == 0x94 && shown {
        Some(Popup::Sign { sx, sy, sector })
    } else {
        None
    }
}

/// The screens of a popup over `base` (320 x 200) with the cursor's saved 16 x 14 block `save`: one per step wait,
/// the end wait's screen, then the screen after the popup. Returns them and the index of the end wait's screen.
pub fn screens(iso: &str, exe: &str, p: &Popup, base: &[u8], save: &[u8]) -> Result<(Vec<Vec<u8>>, usize), i32> {
    let d = crate::panel::panel_data(iso, exe)?;
    let inv = crate::invdraw::inv_data(iso, exe)?;
    let c = crate::common(iso)?;
    let mut table0 = c.remap[0];
    let t29 = crate::panel::table29(&c);
    let mut scr = base.to_vec();
    scr.resize(64000, 0);
    let (sx, sy) = match *p {
        Popup::Item { sx, sy, .. } | Popup::Sign { sx, sy, .. } => (sx, sy),
    };
    // The cursor goes away.
    for r in 0..14 {
        for col in 0..16 {
            put(&mut scr, sx + col, sy + r, save.get((16 * r + col) as usize).copied().unwrap_or(0));
        }
    }
    let mut out = Vec::new();
    let wait;
    match *p {
        Popup::Item { raised, pic, count, .. } => {
            let (x, y) = if raised { (sx - 3, sy - 3) } else { (sx, sy) };
            let (bx, by) = (x - 13, y - 1);
            let b = grab(&scr, bx, by, 42, 16);
            let mut pb = vec![0u8; 64000];
            {
                let mut q = Painter { d: &d, buf: &mut pb, table0: &mut table0, table29: t29 };
                crate::invdraw::picture(&mut q, &inv, pic, 0, 0, 0);
            }
            let pic = grab(&pb, 0, 0, 42, 16);
            let frame = |scr: &mut Vec<u8>, w: i32, h: i32| {
                let mut f = b.clone();
                let s = scale(&pic, 42, 16, w, h);
                let (ox, oy) = ((42 - w) / 2, (16 - h) / 2);
                for j in 0..h {
                    for i in 0..w {
                        let v = s[(j * w + i) as usize];
                        if v != 0 {
                            f[((oy + j) * 42 + ox + i) as usize] = v;
                        }
                    }
                }
                clipped(scr, &f, 42, 16, bx, by, false);
            };
            let (mut w, mut h) = (2, 0);
            loop {
                if w < 42 {
                    w += 5;
                }
                if h < 16 {
                    h += 2;
                }
                frame(&mut scr, w, h);
                out.push(scr.clone());
                if h >= 16 {
                    break;
                }
            }
            if let Some(n) = count {
                let mut s = n.to_string().into_bytes();
                while s.len() < 3 {
                    s.insert(0, b'^');
                }
                let mut q = Painter { d: &d, buf: &mut scr, table0: &mut table0, table29: t29 };
                q.text(&s, x + 16, y + 10, false, 0xFF, 8, 0);
            }
            wait = out.len();
            out.push(scr.clone());
            while h > 2 {
                w -= 5;
                h -= 2;
                frame(&mut scr, w, h);
                out.push(scr.clone());
            }
            unclipped(&mut scr, &b, 42, 16, bx, by);
        }
        Popup::Sign { sector, .. } => {
            let (x0, y0) = (sx - 42, sy - 24);
            let b = grab(&scr, x0, y0, 94, 55);
            let mut sb = vec![0u8; 64000];
            {
                let mut q = Painter { d: &d, buf: &mut sb, table0: &mut table0, table29: t29 };
                crate::invdraw::load_pcx(&mut q, &inv, "sign.pcc", 0, 0);
                let edt = crate::aim::edt(iso, exe, "SIGQUOTE.EDT");
                let at = 200 * sector.max(0) as usize;
                let text = crate::balloon::shared_text(&edt, at);
                let (ls, _) = crate::msgbox::lines_wrap(&q.d.fonts[1].widths, &text, 85);
                for (fg, cx) in [(0x29u8, 47), (7, 46)] {
                    for (i, l) in ls.iter().enumerate() {
                        q.text(l, cx, 14 + 12 * i as i32, true, fg, 0, 1);
                    }
                }
            }
            let sign = grab(&sb, 0, 0, 94, 55);
            let frame = |scr: &mut Vec<u8>, w: i32, h: i32| {
                let mut f = b.clone();
                let s = scale(&sign, 94, 55, w, h);
                let (ox, oy) = ((94 - w) / 2, (55 - h) / 2);
                for j in 0..h {
                    for i in 0..w {
                        f[((oy + j) * 94 + ox + i) as usize] = s[(j * w + i) as usize];
                    }
                }
                clipped(scr, &f, 94, 55, x0, y0, true);
            };
            let (mut w, mut h) = (4, 1);
            loop {
                if w < 94 {
                    w += 10;
                }
                if h < 55 {
                    h += 6;
                }
                frame(&mut scr, w, h);
                out.push(scr.clone());
                if h >= 55 {
                    break;
                }
            }
            wait = out.len();
            out.push(scr.clone());
            while h > 7 {
                w -= 10;
                h -= 6;
                frame(&mut scr, w, h);
                out.push(scr.clone());
            }
            unclipped(&mut scr, &b, 94, 55, x0, y0);
        }
    }
    out.push(scr);
    Ok((out, wait))
}

fn put(scr: &mut [u8], x: i32, y: i32, v: u8) {
    // SPEC GAP: the original writes outside the screen buffer too; such pixels are dropped here.
    let o = y as i64 * PW as i64 + x as i64;
    if o >= 0 && (o as usize) < scr.len() {
        scr[o as usize] = v;
    }
}

/// The w x h screen block at (x, y), rows of the 320-byte screen (no clipping: offsets outside the screen read 0).
fn grab(scr: &[u8], x: i32, y: i32, w: i32, h: i32) -> Vec<u8> {
    let mut v = Vec::with_capacity((w * h) as usize);
    for r in 0..h {
        for c in 0..w {
            let o = (y + r) as i64 * PW as i64 + (x + c) as i64;
            v.push(if o >= 0 { scr.get(o as usize).copied().unwrap_or(0) } else { 0 });
        }
    }
    v
}

fn unclipped(scr: &mut [u8], b: &[u8], w: i32, h: i32, x: i32, y: i32) {
    for r in 0..h {
        for c in 0..w {
            put(scr, x + c, y + r, b[(r * w + c) as usize]);
        }
    }
}

/// The clipped copy of a w0 x h0 block to (bx, by); `row_skip` = the sign's top clip (rows start at h0 - h).
fn clipped(scr: &mut [u8], f: &[u8], w0: i32, h0: i32, mut bx: i32, mut by: i32, row_skip: bool) {
    let (mut w, mut h, mut c0, mut r0) = (w0, h0, 0, 0);
    if bx < 0 {
        w = bx + w0;
        c0 = w0 - w;
        bx = 0;
    } else if bx + w0 > 319 {
        w = 320 - bx;
    }
    if by < 0 {
        h = by + h0;
        if row_skip {
            r0 = h0 - h;
        }
        by = 0;
    } else if by + h0 > 199 {
        h = 200 - by;
    }
    for r in 0..h.max(0) {
        for c in 0..w.max(0) {
            put(scr, bx + c, by + r, f[((r0 + r) * w0 + c0 + c) as usize]);
        }
    }
}

/// The scaled copy of a sw x sh block to dw x dh (nearest point, 10.22 fixed point).
pub(crate) fn scale(src: &[u8], sw: i32, sh: i32, dw: i32, dh: i32) -> Vec<u8> {
    let (fx, fy) = (((sw as u32) << 22) / dw as u32, ((sh as u32) << 22) / dh as u32);
    let mut v = Vec::with_capacity((dw * dh) as usize);
    for j in 0..dh as u32 {
        let row = ((fy / 2).wrapping_add(j.wrapping_mul(fy)) >> 22) as i32;
        for i in 0..dw as u32 {
            let col = ((fx / 2).wrapping_add(i.wrapping_mul(fx)) >> 22) as i32;
            v.push(src.get((row * sw + col) as usize).copied().unwrap_or(0));
        }
    }
    v
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Ledger row ja1front:14073: a block that runs past row 199 is cut there, also on a taller buffer.
    #[test]
    fn clipped_stops_at_row_199() {
        let mut scr = vec![0u8; PW as usize * 210];
        clipped(&mut scr, &[7u8; 40], 4, 10, 0, 195, false);
        assert_eq!(scr[199 * PW as usize], 7);
        assert_eq!(scr[200 * PW as usize], 0);
    }

    #[test]
    fn frame_counts() {
        // Off the window: nothing.
        let mut tiles = vec![0u8; 3136 * 8];
        tiles[8 * 58 + 6] = 0xFF;
        tiles[8 * 58 + 1] = 0x94;
        let v = (0, 0, 32, 28, 256, 140);
        assert_eq!(which(&tiles, &[0; 2400], &[0; 1632], 58, v, 3), Some(Popup::Sign { sx: 64, sy: 42, sector: 3 }));
        assert!(which(&tiles, &[0; 2400], &[0; 1632], 58, (1000, 0, 32, 28, 256, 140), 3).is_none());
    }
}
