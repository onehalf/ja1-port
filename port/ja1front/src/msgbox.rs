//! The message box (docs/spec/panel.md "Message box"): a short message in a dark box in the middle of the screen.
use crate::panel::{Painter, PW};

/// The screen rectangle saved before the box and put back after it: x, y, w, h.
pub const SAVED: (i32, i32, i32, i32) = (52, 37, 216, 80);

/// The darkening table of the strategic screen.
pub const MAP_TABLE: usize = 32;

const FONT: usize = 2;
const LINE_H: i32 = 11;
const WRAP: i32 = 189;

/// The frame colour for darkening table `table`.
pub fn frame_colour(table: usize) -> u8 {
    if table == 31 {
        0xFF
    } else {
        0x68
    }
}

/// The lines of `s` as the box breaks them, and the line count for the bottom border (one more than the finished
/// lines: a text that ends with a line break gets an empty band before the bottom border).
pub fn lines(widths: &[u8; 131], s: &[u8]) -> (Vec<Vec<u8>>, i32) {
    lines_wrap(widths, s, WRAP)
}

/// `lines` with the wrap width `wrap` (the question box uses 129).
pub fn lines_wrap(widths: &[u8; 131], s: &[u8], wrap: i32) -> (Vec<Vec<u8>>, i32) {
    lines_wrap_enc(widths, s, wrap, false)
}

/// `lines_wrap`; with `enc` every byte of a word is taken − 1 (EDT texts, panel.md "Item info box"; the spaces
/// between words stay as they are).
pub fn lines_wrap_enc(widths: &[u8; 131], s: &[u8], wrap: i32, enc: bool) -> (Vec<Vec<u8>>, i32) {
    let width = |b: u8| widths.get((b as usize).wrapping_sub(0x20)).copied().unwrap_or(0) as i32;
    let mut out = Vec::new();
    let (mut line, mut l) = (Vec::new(), 0);
    let mut pos = 0;
    loop {
        while s.get(pos) == Some(&b' ') {
            pos += 1;
        }
        let mut u = 0;
        let mut word = Vec::new();
        while let Some(&b) = s.get(pos) {
            if b == b' ' || b == 0 {
                break;
            }
            let b = if enc { b.wrapping_sub(1) } else { b };
            word.push(b);
            u = if b == 0xB1 || b == 0xB3 || b < 0x20 { 1 } else { u + width(b) };
            pos += 1;
        }
        if u <= 0 {
            break;
        }
        let word = &word[..];
        if l + u > wrap || word == [0xB1] {
            out.push(std::mem::take(&mut line));
            if word == [0xB1] {
                l = 0;
            } else {
                line.extend_from_slice(word);
                l = u;
            }
        } else {
            if l != 0 {
                line.push(b' ');
                l += width(b' ');
            }
            line.extend_from_slice(word);
            l += u;
        }
    }
    let rows = out.len() as i32 + 1;
    if l != 0 {
        out.push(line);
    }
    (out, rows)
}

fn darken(buf: &mut [u8], table: &[u8; 256], x0: i32, y: i32, w: i32, h: i32) {
    for yy in y..y + h {
        for xx in x0..x0 + w {
            let i = (yy * PW + xx) as usize;
            if let Some(p) = buf.get_mut(i) {
                *p = table[*p as usize];
            }
        }
    }
}

/// Draws the box of `s` (zero-ended or not) with darkening table `dark` and frame colour `colour`.
pub fn draw(p: &mut Painter, s: &[u8], dark: &[u8; 256], colour: u8) {
    let s = &s[..s.iter().position(|&b| b == 0).unwrap_or(s.len())];
    let widths = p.d.fonts[FONT].widths;
    let t: i32 = s.iter().map(|&b| widths.get((b as usize).wrapping_sub(0x20)).copied().unwrap_or(0) as i32).sum();
    let w = if t > 190 { 206 } else { t + 16 };
    let x0 = (PW - w) / 2;
    let (ls, rows) = lines(widths, s);
    darken(p.buf, dark, x0, 58, w, LINE_H / 2);
    for (i, l) in ls.iter().enumerate() {
        let y = 63 + LINE_H * i as i32;
        darken(p.buf, dark, x0, y, w, LINE_H);
        p.text(l, 161, y, true, 0xFF, 8, FONT);
    }
    let yb = 63 + LINE_H * rows;
    darken(p.buf, dark, x0, yb, w, LINE_H / 2);
    p.frame(x0, 58, x0 + w + 1, yb + 6, 1);
    p.frame(x0 - 1, 57, x0 + w, yb + 5, colour);
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Ledger row ja1front:13001: a control byte below 0x20 in a word counts 1 pixel, so a word made of one goes
    /// into the line and the text after it still wraps in.
    #[test]
    fn a_control_byte_word_counts_one_pixel() {
        let w = [6u8; 131];
        let (l, rows) = lines_wrap(&w, b"a \x05 b", 189);
        assert_eq!((l, rows), (vec![b"a \x05 b".to_vec()], 1));
    }

    #[test]
    fn breaks() {
        let mut w = [6u8; 131];
        w[0] = 3;
        // 31 words of 6 px: 6 + 30 * 9 > 189: the line holds 21 words (6 + 20 * 9 = 186).
        let s = b"a ".repeat(31);
        let (l, rows) = lines(&w, &s);
        assert_eq!((l.len(), rows), (2, 2));
        assert_eq!(l[0].len(), 41);
        assert_eq!(lines(&w, b"a \xB1 b"), (vec![b"a".to_vec(), b"b".to_vec()], 2));
        assert_eq!(lines(&w, b"a \xB1"), (vec![b"a".to_vec()], 2));
        assert_eq!(game_bytes(&game_text(b"\"a\" \xB1 \x80")), b"\"a\" \xB1 \x80".to_vec());
        assert_eq!(lines(&w, b"   "), (Vec::<Vec<u8>>::new(), 1));
        assert_eq!(lines(&w, b"tw  tv_  PH  \xB1  \xB1"), (vec![b"tw tv_ PH".to_vec(), vec![]], 3));
    }
}

/// The darkening table of the tactical screen (panel.md "Tactical messages"): the sector's tileset table.
pub const TACTICAL_TABLE: usize = 28;

/// The game bytes of a box text kept as a String with one char per byte (game_text): 0xB1 and the Cyrillic
/// letters stay single bytes.
pub fn game_bytes(t: &str) -> Vec<u8> {
    t.chars().map(|c| c as u32 as u8).collect()
}

/// A String with one char per game byte (Latin-1), so game_bytes gives the bytes back.
pub fn game_text(b: &[u8]) -> String {
    b.iter().map(|&c| c as char).collect()
}

/// panel.md "Tactical messages": the message box of `s` on the 320 x 200 tactical `screen` of tileset `tileset`
/// (darkening table 28 = the tileset table, frame colour 0x68). Steps 1, 2 only: the caller waits and puts the
/// screen back.
pub fn draw_tactical(iso: &str, exe: &str, tileset: u8, s: &[u8], screen: &mut [u8]) -> Result<(), i32> {
    let c = crate::common(iso)?;
    let d = crate::panel::panel_data(iso, exe)?;
    let t = crate::tileset_data(iso, tileset)?;
    let mut table0 = c.remap[0];
    let mut p = Painter { d: &d, buf: screen, table0: &mut table0, table29: crate::panel::table29(&c) };
    draw(&mut p, s, &t.table28, frame_colour(TACTICAL_TABLE));
    Ok(())
}
