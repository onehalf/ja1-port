//! The item info box (docs/spec/panel.md "Item info box"): the item's picture, name, numbers and description in
//! the question box's frame, or the full picture of a picture item (letters, photos, map pieces).
use crate::invdraw::{self, load_pcx, InvData};
use crate::panel::{Painter, PW};
use crate::question::BOX;

/// The Control Panel options Item Info and Subtitles where no world gives them (the values in the original's state
/// tac60); the window's inventory and assignment screen read the world's switches 6 and 5 (ctlpanel::get).
pub const OPT_INFO: bool = true;
pub const OPT_SUB: bool = false;

/// Subtitles for the captions of speech (the listening device): OPT_SUB, or on when JA1_SUBTITLES=1.
pub fn subtitles() -> bool {
    OPT_SUB || std::env::var("JA1_SUBTITLES").is_ok_and(|v| v == "1")
}

/// The inputs of one box (host op 231 item_info without the mouse flag).
#[derive(Clone, Copy, Debug)]
pub struct Info {
    pub profile: u8,
    pub t: u8,
    pub count: i8,
    pub value: i16,
    /// The numbers flag a.
    pub numbers: bool,
    /// Control Panel option 6 Item Info.
    pub opt_info: bool,
    /// Control Panel option 5 Subtitles.
    pub opt_sub: bool,
}

/// What `draw` did.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Shown {
    /// t = 0, or Item Info off for an item that is not a picture item: the screen is as before.
    Nothing,
    /// The box is on the screen (the saved rectangle `question::BOX` goes back after the wait).
    Box,
    /// A picture item's picture is on the screen (result 1: the caller draws the screen again).
    Picture,
}

impl Shown {
    /// The result of the original (1 = draw the screen again).
    pub fn result(self) -> i32 {
        (self == Shown::Picture) as i32
    }
}

fn text_of(rec: &[u8], at: usize, len: usize) -> Vec<u8> {
    let b = rec.get(at..(at + len).min(rec.len())).unwrap_or(&[]);
    b[..b.iter().position(|&c| c == 0).unwrap_or(b.len())].to_vec()
}

/// Steps 1..6 on the screen of `p`.
pub fn draw(p: &mut Painter, inv: &InvData, i: Info) -> Shown {
    if i.t == 0 {
        return Shown::Nothing;
    }
    let t = if i.t == 0x59 && i.profile >= 60 { 0x5A } else { i.t };
    let rec = inv.quotes.get(t as usize * 270..t as usize * 270 + 270).unwrap_or(&[]);
    let (name, desc) = (text_of(rec, 0, 30), text_of(rec, 30, 240));
    let it = |k: usize| inv.item_types.get(t as usize * 12 + k).copied().unwrap_or(0);
    let mut scratch = vec![0u8; 64000];
    let mut table0 = *p.table0;
    if matches!(it(4), 0x1A | 0x74 | 0x52) {
        {
            let mut q = Painter { d: p.d, buf: &mut scratch, table0: &mut table0, table29: p.table29 };
            let n = if t == 0x5A { "item89.pcc".to_string() } else { format!("item{}.pcc", t) };
            load_pcx(&mut q, inv, &n, 0, 0);
            if i.opt_sub && (t == 0x59 || t == 0x5A) {
                let (ls, _) = crate::msgbox::lines_wrap_enc(&q.d.fonts[2].widths, &desc, 144, true);
                for (k, l) in ls.iter().enumerate() {
                    q.text(l, 28, 80 + 11 * k as i32, false, 1, 0, 2);
                }
            }
        }
        for y in 0..150 {
            for x in 0..200 {
                let v = scratch[(y * PW + x) as usize];
                if v != 0 {
                    p.put(60 + x, 20 + y, v);
                }
            }
        }
        return Shown::Picture;
    }
    if !i.opt_info {
        return Shown::Nothing;
    }
    let text = if t == 0x6F {
        let mut amount = b"$".to_vec();
        if i.value != 0 {
            amount.extend_from_slice(i.value.to_string().as_bytes());
        }
        let amount: Vec<u8> = amount.iter().map(|b| b.wrapping_add(1)).collect();
        match desc.windows(5).position(|w| w == b"BNPVO") {
            Some(k) => [&desc[..k], &amount[..], &desc[k + 5..]].concat(),
            None => desc.clone(),
        }
    } else {
        desc.clone()
    };
    let (bx, by, bw, bh) = BOX;
    {
        let mut q = Painter { d: p.d, buf: &mut scratch, table0: &mut table0, table29: p.table29 };
        load_pcx(&mut q, inv, "PANEL_P.PCC", bx, by);
        for y in 0..68 {
            for x in 0..64 {
                let v = q.buf[((by + 6 + y) * PW + bx + 69 + x) as usize];
                q.buf[((by + 6 + y) * PW + bx + 6 + x) as usize] = v;
            }
        }
        invdraw::picture(&mut q, inv, it(4), bx + 10, by + 10, 0x0A);
        if i.value < 10 && (it(9) != 0 || it(7) != 0) {
            q.text(b"useless", bx + 31, by + 16, true, 0xED, 1, 0);
        } else if i.count < 0 {
            q.text(b"jammed", bx + 31, by + 16, true, 0xED, 1, 0);
        }
        let name: Vec<u8> = name.iter().map(|&b| if (b as i8) > 0x20 { b - 1 } else { b }).collect();
        q.text(&name, bx + 60, by + 14, false, 0xFF, 8, 1);
        let clip = it(5) as i8;
        if i.numbers && clip != 99 && it(6) == 1 {
            invdraw::number(&mut q, (i.count as i32).abs(), 2, bx + 138, by + 14, 0x28, 8, 1);
            q.text(b"/", bx + 150, by + 14, false, 0x28, 8, 1);
            invdraw::number(&mut q, clip as i32, 2, bx + 156, by + 14, 0x28, 8, 1);
        }
        if i.numbers && it(10) != 0 {
            q.element(0x42, bx + 174, by + 11, None);
            invdraw::cond_bar(&mut q, i.value as u8 as i8 as i32, bx + 176, by + 13);
            invdraw::number(&mut q, i.value as i32, 3, bx + 181, by + 14, 0x28, 8, 1);
            q.text(b"%", bx + 199, by + 14, false, 0x28, 8, 1);
        }
        let (ls, _) = crate::msgbox::lines_wrap_enc(&q.d.fonts[0].widths, &text, 194, true);
        for (k, l) in ls.iter().enumerate() {
            q.text(l, bx + 10, by + 30 + 8 * k as i32, false, 0x28, 8, 0);
        }
    }
    for y in by..by + bh {
        let o = (y * PW + bx) as usize;
        p.buf[o..o + bw as usize].copy_from_slice(&scratch[o..o + bw as usize]);
    }
    Shown::Box
}

/// panel.md "The item's voice": the steps of each voice kind (item table byte 3), 7 codes per kind; the code of
/// step 0 is never read.
const STEPS: [[u8; 7]; 7] = [[0; 7], [1, 0, 0, 0, 0, 0, 0], [1, 2, 0, 0, 0, 0, 0], [1, 2, 5, 0, 0, 0, 0], [1, 4, 0, 0, 0, 0, 0],
    [1, 3, 0, 0, 0, 0, 0], [0, 6, 7, 11, 8, 9, 10]];

/// One sample of the item's voice: a member of the archive `INV\<archive>`; `voc` = a VOC member (the cry channel).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct VoicePart {
    pub archive: &'static str,
    pub member: String,
    pub voc: bool,
}

/// The grade letter of the item's signed count (InvGradeName 0x25BDC, panel.md "The item's voice"): A..F.
pub fn grade_letter(v: i8) -> char {
    match v {
        100.. => 'F',
        76..=99 => 'E',
        51..=75 => 'D',
        26..=50 => 'C',
        11..=25 => 'B',
        _ => 'A',
    }
}

/// InvGradeName 0x25BDC: the voice member name. `code` 2 = C, 3 = L, 4 = U: the prefix replaces `base`; any other
/// code keeps it. Then the grade letter of the signed count and ".8" go at the end of the text.
pub fn inv_grade_name(base: &str, code: u8, v: i8) -> String {
    let mut s = match code {
        2 => "C".to_string(),
        3 => "L".to_string(),
        4 => "U".to_string(),
        _ => base.to_string(),
    };
    s.push(grade_letter(v));
    s.push_str(".8");
    s
}

/// panel.md "The item's voice": the samples that play in turn for the box of `i` (`b` = the voice flag), in the
/// order of the steps; `has(archive, member)` tells whether the member exists (a missing one plays nothing). None =
/// no name sample: nothing plays and the voice never ends by itself.
pub fn voice(i: &Info, item_types: &[u8], b: bool, has: &dyn Fn(&str, &str) -> bool) -> Option<Vec<VoicePart>> {
    if i.t == 0 {
        return None;
    }
    let t = if i.t == 0x59 && i.profile >= 60 { 0x5A } else { i.t };
    let it = |k: usize| item_types.get(t as usize * 12 + k).copied().unwrap_or(0);
    let (kind, class, clip) = (it(3) as usize, it(6), it(5) as i8);
    let (n, v) = (i.count, i.value);
    let name_arc = match t {
        0x6F => "MONEY",
        0..=54 => "1_54",
        55..=81 => "55_81",
        _ => "82_135",
    };
    let mut parts = Vec::new();
    let mut cur = name_arc;
    let push = |parts: &mut Vec<VoicePart>, arc: &'static str, member: String, voc: bool| {
        if has(arc, &member) {
            parts.push(VoicePart { archive: arc, member, voc });
        }
    };
    let and = t == 0x6F && v > 99;
    if t != 0x6F {
        let mut name = t.to_string();
        if class > 1 {
            name += &format!("_{}", if n == 99 { 1 } else { n as i32 });
        }
        name += ".8";
        if !has(name_arc, &name) {
            return None;
        }
        parts.push(VoicePart { archive: name_arc, member: name, voc: false });
    }
    let g = v as i8;
    let mut b = b;
    let mut r = v as i32;
    for s in 1..7 {
        match STEPS.get(kind).map(|row| row[s]).unwrap_or(0) {
            0 | 1 => break,
            2 => {
                if class > 1 && n > 1 {
                    b = false;
                }
                if b && n >= 0 {
                    cur = "EXTRA";
                    push(&mut parts, cur, inv_grade_name("", 2, g), false);
                }
            }
            c @ (3 | 4) => {
                if b && n == 1 {
                    cur = "EXTRA";
                    push(&mut parts, cur, inv_grade_name("", if c == 3 { 3 } else { 4 }, g), false);
                }
            }
            5 => {
                if b {
                    push(&mut parts, cur, format!("b{}.8", if n == 99 { clip as i32 } else { n as i32 }), false);
                }
            }
            c @ (6 | 7 | 8) => {
                let unit = [1000, 100, 10][c as usize - 6];
                let q = r / unit;
                if q > (c == 8) as i32 {
                    push(&mut parts, "MONEY", format!("M{}.8", q * unit), false);
                    r -= q * unit;
                }
            }
            9 => {
                if r > 0 {
                    push(&mut parts, "MONEY", format!("M{}.8", r), false);
                }
            }
            10 => push(&mut parts, "MONEY", "dollars.voc".into(), true),
            11 => {
                if r > 0 && and {
                    push(&mut parts, "MONEY", "and.voc".into(), true);
                }
            }
            _ => {}
        }
    }
    Some(parts)
}

/// ItemInfoBox 0x260C6, for a box with text (Item Info on; a picture item and Item Info off have no mouse step): with the
/// mouse flag `m` the place goes to MouseSavePos2?; a mouse outside the box moves to (237, 102). The answer = the move.
pub fn mouse_open(m: bool, mouse: (i32, i32)) -> Option<(i32, i32)> {
    if m {
        crate::pointer::mouse_save(mouse);
    }
    crate::confirm::open_warp(mouse)
}

/// ItemInfoBox 0x26BB0: at the end the mouse goes back to the saved place when it is less than 10 pixels from `after`,
/// its place after the move, on both axes, and the flag `m` is on. The answer = the place.
pub fn mouse_close(m: bool, after: (i32, i32), now: (i32, i32)) -> Option<(i32, i32)> {
    (m && crate::question::prompt_back(after, now)).then(crate::pointer::mouse_saved)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn info(t: u8, count: i8, value: i16) -> Info {
        Info { profile: 0, t, count, value, numbers: true, opt_info: true, opt_sub: false }
    }

    fn names(i: Info, types: &[u8], b: bool) -> Option<Vec<String>> {
        voice(&i, types, b, &|_, m| m != "M31000.8").map(|v| v.into_iter().map(|p| format!("{}\\{}", p.archive, p.member)).collect())
    }

    #[test]
    fn voice_steps() {
        let mut types = vec![0u8; 136 * 12];
        types[111 * 12 + 3] = 6;
        types[1 * 12 + 3] = 3;
        types[1 * 12 + 5] = 6;
        types[20 * 12 + 3] = 2;
        types[20 * 12 + 6] = 2;
        types[71 * 12 + 3] = 4;
        let m = |v| names(info(111, 1, v), &types, true).unwrap().join(" ");
        assert_eq!(m(2345), "MONEY\\M2000.8 MONEY\\M300.8 MONEY\\and.voc MONEY\\M40.8 MONEY\\M5.8 MONEY\\dollars.voc");
        assert_eq!(m(15), "MONEY\\M15.8 MONEY\\dollars.voc");
        assert_eq!(m(300), "MONEY\\M300.8 MONEY\\dollars.voc");
        assert_eq!(m(0), "MONEY\\dollars.voc");
        assert_eq!(m(31005), "MONEY\\and.voc MONEY\\M5.8 MONEY\\dollars.voc");
        // a gun: condition grade, then rounds (n = 99: the clip byte); no voice flag: the name only
        assert_eq!(names(info(1, 99, 80), &types, true).unwrap().join(" "), "1_54\\1.8 EXTRA\\CE.8 EXTRA\\b6.8");
        assert_eq!(names(info(1, 3, 10), &types, false).unwrap().join(" "), "1_54\\1.8");
        // a stack of 3 (class 2): the name with _3, and the flag goes off (no condition)
        assert_eq!(names(info(20, 3, 100), &types, true).unwrap().join(" "), "1_54\\20_3.8");
        assert_eq!(names(info(20, 99, 100), &types, true).unwrap().join(" "), "1_54\\20_1.8");
        // kind 4: U + grade only for n = 1
        assert_eq!(names(info(71, 1, 30), &types, true).unwrap().join(" "), "55_81\\71.8 EXTRA\\UC.8");
        assert_eq!(names(info(0, 1, 30), &types, true), None);
        // Ledger row ja1front:13735: without the voice flag kind 4 says the name only, also for n = 1.
        assert_eq!(names(info(71, 1, 30), &types, false).unwrap().join(" "), "55_81\\71.8");
    }
}
