//! panel.md "Speech balloon": the merc's face slides in over its strip portrait, the quote text shows beside it,
//! and the face slides out.

use crate::invdraw::InvData;
use crate::panel::{archive_member, pcx, Painter};

/// Face sheet pitch and height.
const FW: usize = 148;
const FH: usize = 69;
/// The balloon rectangle.
pub const BW: usize = 58;
pub const BH: usize = 52;
/// Frame sizes of the slide in and the slide out.
pub const SLIDE_IN: [(i32, i32); 4] = [(35, 32), (42, 38), (49, 44), (56, 50)];
pub const SLIDE_OUT: [(i32, i32); 4] = [(49, 44), (42, 38), (35, 32), (28, 26)];
/// Wait between two frames (ms).
pub const FRAME_MS: u32 = 30;

/// SpeechBalloon? 0x296DA / SpeechWait? 0x28682: the balloon with a face (9 frames: the face's way in) and the
/// text-only box (one held frame, Subtitles on) wait for the buttons' release before and after the speech wait.
pub fn waits_release(frames: usize, hold: usize) -> bool {
    frames == 9 || (hold == 0 && frames == 1)
}

/// What frame `n` of a balloon with `frames` frames and the held frame `hold` waits for: only the held frame waits
/// for the buttons' release (SpeechBalloon? 0x296DA, `waits_release`), and only the held frame of the balloon with a
/// face (9 frames) or of the text-only box (1 frame) animates the face during its wait (panel.md "Face animation").
#[derive(Debug, PartialEq, Eq)]
pub struct FrameWait {
    pub release: bool,
    pub face: bool,
}

pub fn frame_wait(n: usize, frames: usize, hold: usize) -> FrameWait {
    let held = n == hold;
    FrameWait { release: held && waits_release(frames, hold), face: held && (frames == 9 || frames == 1) }
}

/// Part `k` of the held frame (SpeechBalloon? 0x29544 runs SpeechWait? once per part): part 0 is the frame as shown;
/// each later part starts its text and voice and puts the clean frame back before its wait.
pub fn part_restarts(k: usize) -> bool {
    k > 0
}

/// The wait of the held frame `n == hold` ends with the voice when a voice speaks (panel.md step 7, sound.md "Merc
/// quotes"); the other frames wait their frame time.
pub fn held_voiced(n: usize, hold: usize, speaking: bool) -> bool {
    n == hold && speaking
}

/// SectorVictory? 0x3A0B0 after the quote of the last enemy's death (quote 0x16): the combat stops only after the
/// balloon's wait. The port ran that stop inside the pass (combat 0, turn clock 20000), so the balloon puts the combat
/// back for its wait (live, #2: the picture under that quote still has DONE).
pub fn victory_quote_holds_combat(quote: Option<i32>, combat: u8, turn_clock: i32) -> bool {
    quote == Some(0x16) && combat == 0 && turn_clock == 20000
}

/// One open balloon.
pub struct Balloon {
    pub bx: i32,
    pub by: i32,
    tx: i32,
    ty: i32,
    face: Vec<u8>,
    save: Vec<u8>,
    /// The decoded quote text.
    pub text: Vec<u8>,
    text_on: bool,
    /// Quote 9: the hurt face (mode b = 1 of panel.md "Face animation").
    hurt: bool,
}

thread_local! {
    /// The face of the last balloon made by `slide` (the window animates it during the wait).
    static LAST: std::cell::RefCell<Option<crate::faceanim::FaceSpot>> = const { std::cell::RefCell::new(None) };
}

/// The face of the last balloon made here (panel.md "Face animation"): the sheet at the balloon rectangle + (1, 1).
pub fn last_face() -> Option<crate::faceanim::FaceSpot> {
    LAST.with(|l| l.borrow().clone())
}

/// panel.md "Speech balloon" step 7: the wait text (the weapon text of the merc message: JAMMED ...) over the face
/// `spot` on `buf` (the screen as it shows): centred at face point (28, 20), fg 0xFF, shadow 8, font 2, only inside
/// the 56 x 50 face.
pub fn wait_text(iso: &str, exe: &str, buf: &mut [u8], spot: &crate::faceanim::FaceSpot, text: &[u8]) -> Result<(), i32> {
    let (c, d) = (crate::common(iso)?, crate::panel::panel_data(iso, exe)?);
    let mut scr = buf.to_vec();
    let mut table0 = c.remap[0];
    let mut p = Painter { d: &d, buf: &mut scr, table0: &mut table0, table29: crate::panel::table29(&c) };
    p.text(text, spot.x + 28, spot.y + 20, true, 0xFF, 8, 2);
    for j in 0..50 {
        for i in 0..56 {
            let (x, y) = (spot.x + i, spot.y + j);
            if (0..320).contains(&x) && (0..200).contains(&y) {
                let at = (y * 320 + x) as usize;
                buf[at] = scr[at];
            }
        }
    }
    Ok(())
}

/// The face that the next wait animates (the merc-face box sets it).
pub fn set_last_face(spot: crate::faceanim::FaceSpot) {
    LAST.with(|l| *l.borrow_mut() = Some(spot));
}

impl Balloon {
    /// Steps 1 .. 4 for record `r` and quote `q`: `quotes` = DAYQUOTE.EDT; the background is taken from `screen`.
    pub fn open(main: &InvData, quotes: &[u8], r: &[u8], q: i32, text_on: bool, screen: &[u8]) -> Balloon {
        let text = quote(quotes, r[0x179] as usize, q);
        Balloon::open_text(main, text, r, q == 9, text_on, screen)
    }

    /// Steps 1 .. 4 for record `r` with the decoded quote `text`; `hurt` = the face of quote 9 (the merc message's
    /// quote 9; the native guide's sector line passes false).
    pub fn open_text(main: &InvData, text: Vec<u8>, r: &[u8], hurt: bool, text_on: bool, screen: &[u8]) -> Balloon {
        let (k, c, p) = (r[0x17A] as i32, r[0x180], r[0x179] as usize);
        let mut face = face_sheet(main, &p.to_string());
        if hurt {
            for y in 0..23 {
                let s = y * FW + 56;
                let row = face[s..s + 46].to_vec();
                face[(y + 5) * FW + 5..(y + 5) * FW + 51].copy_from_slice(&row);
            }
        }
        let (ox, oy) = (if k < 4 { 0 } else { 272 }, 14 + 42 * (k % 4));
        let (fx, fy) = if c == 0 { (14, 3) } else { (3, 3) };
        let (x, y) = (ox + fx + 2, oy + fy + 2);
        let d = match k % 4 {
            0 => 4,
            3 => 14,
            _ => 10,
        };
        let mut b = Balloon::at(face, x, y, text, false, screen);
        (b.tx, b.ty, b.text_on, b.hurt) = (if k < 4 { 140 } else { 178 }, y - d - 1, text_on, hurt);
        b
    }

    /// Steps 3 (the rectangle) and 4 for the face sheet `face` at the face point (x, y); the text is not drawn
    /// unless `text_on` (then centred at x).
    pub fn at(face: Vec<u8>, x: i32, y: i32, text: Vec<u8>, text_on: bool, screen: &[u8]) -> Balloon {
        let (bx, by) = ((x - 15).clamp(1, 0x105), y - 14);
        let mut save = vec![0u8; BW * BH];
        for j in 0..BH {
            for i in 0..BW {
                save[j * BW + i] = get(screen, bx + i as i32, by + j as i32);
            }
        }
        Balloon { bx, by, tx: x, ty: y, face, save, text, text_on, hurt: false }
    }

    /// One slide frame (w, h) onto `p.buf` (step 5); `p` draws the frame.
    pub fn frame(&self, p: &mut Painter, w: i32, h: i32) {
        let mut pic = vec![0u8; 64000];
        for j in 0..BH {
            pic[j * 320..j * 320 + BW].copy_from_slice(&self.save[j * BW..(j + 1) * BW]);
        }
        if w > 28 {
            let (x0, y0) = ((58 - w) / 2, (52 - h) / 2);
            let sx = ((56u32 << 22) / w as u32) as u32;
            let sy = ((50u32 << 22) / h as u32) as u32;
            for j in 0..h as u32 {
                let row = (sy / 2).wrapping_add(j.wrapping_mul(sy)) >> 22;
                for i in 0..w as u32 {
                    let col = (sx / 2).wrapping_add(i.wrapping_mul(sx)) >> 22;
                    pic[(y0 as u32 + j) as usize * 320 + (x0 as u32 + i) as usize] =
                        self.face[row as usize * FW + col as usize];
                }
            }
            let mut q = Painter { d: p.d, buf: &mut pic, table0: p.table0, table29: p.table29 };
            q.frame(x0 - 1, y0 - 1, x0 + w, y0 + h, 0xFF);
        }
        for j in 0..BH as i32 {
            for i in 0..BW as i32 {
                let (x, y) = (self.bx + i, self.by + j);
                if (0..320).contains(&x) && (0..200).contains(&y) {
                    p.buf[(y * 320 + x) as usize] = pic[(j * 320 + i) as usize];
                }
            }
        }
    }

    /// Step 6: the text beside the face (only with the Text option).
    pub fn draw_text(&self, p: &mut Painter) {
        if !self.text_on {
            return;
        }
        let (ls, _) = crate::msgbox::lines_wrap(&p.d.fonts[2].widths, &self.text, 159);
        for (i, l) in ls.iter().enumerate() {
            p.text(l, self.tx, self.ty + 10 * i as i32, true, 0xFF, 1, 2);
        }
    }

    /// Step 7: the wait without a voice (ms).
    pub fn wait_ms(&self) -> u32 {
        80 * self.text.len() as u32
    }
}

/// Step 1: the face sheet with the MAIN.DAT member `member` (a PCC picture) at (0, 0).
pub fn face_sheet(main: &InvData, member: &str) -> Vec<u8> {
    let mut face = vec![0u8; FW * FH];
    // SPEC GAP: a face member that is missing or not a PCX leaves the sheet empty.
    if let Ok(m) = archive_member(&main.main, member) {
        if let Ok((w, h, px)) = pcx(&m) {
            for y in 0..h.min(FH) {
                for x in 0..w.min(FW) {
                    face[y * FW + x] = px[y * w + x];
                }
            }
        }
    }
    face
}

fn get(screen: &[u8], x: i32, y: i32) -> u8 {
    if (0..320).contains(&x) && (0..200).contains(&y) {
        screen[(y * 320 + x) as usize]
    } else {
        0
    }
}

/// Steps 1 .. 8 over `screen` (320 x 200): the 9 screens (4 slide-in frames, the screen at the wait, 4 slide-out
/// frames) and the wait without a voice (ms).
pub fn screens(iso: &str, exe: &str, r: &[u8], q: i32, text_on: bool, screen: &[u8]) -> Result<(Vec<Vec<u8>>, u32), i32> {
    let d = crate::panel::panel_data(iso, exe)?;
    let inv = crate::invdraw::inv_data(iso, exe)?;
    let quotes = crate::aim::edt(iso, exe, "DAYQUOTE.EDT");
    let b = Balloon::open(&inv, &quotes, r, q, text_on, screen);
    slide(iso, exe, &d, b, screen)
}

/// turns.md "Sector line" (host op 168, 0x297D4): steps 1 .. 8 for the native guide's record `r` with the decoded
/// NATQUOTE.EDT record `sector` (row 0), no hurt face.
pub fn sector_screens(iso: &str, exe: &str, r: &[u8], sector: i32, text_on: bool, screen: &[u8]) -> Result<(Vec<Vec<u8>>, u32), i32> {
    let d = crate::panel::panel_data(iso, exe)?;
    let inv = crate::invdraw::inv_data(iso, exe)?;
    let b = Balloon::open_text(&inv, nat_text(iso, exe, sector), r, false, text_on, screen);
    slide(iso, exe, &d, b, screen)
}

/// The balloon screens of a reaction: the sector line (`nat`, q = sector) or a merc message (DAY quote q).
pub fn reaction_screens(iso: &str, exe: &str, r: &[u8], q: i32, nat: bool, text_on: bool, screen: &[u8]) -> Result<(Vec<Vec<u8>>, u32), i32> {
    if nat { sector_screens(iso, exe, r, q, text_on, screen) } else { screens(iso, exe, r, q, text_on, screen) }
}

/// The decoded NATQUOTE.EDT record `sector` (PlayQuote row 0, quote = sector).
pub fn nat_text(iso: &str, exe: &str, sector: i32) -> Vec<u8> {
    shared_text(&crate::aim::edt(iso, exe, "NATQUOTE.EDT"), 200 * sector.max(0) as usize)
}

/// panel.md "Santino's balloon": steps 1 .. 8 for Santino (face member "66", face point (134, 50), no text) with
/// the quote text `text` (its length sets the wait without a voice).
pub fn santino_screens(iso: &str, exe: &str, text: &[u8], screen: &[u8]) -> Result<(Vec<Vec<u8>>, u32), i32> {
    let d = crate::panel::panel_data(iso, exe)?;
    let inv = crate::invdraw::inv_data(iso, exe)?;
    let b = Balloon::at(face_sheet(&inv, "66"), SANTINO.0, SANTINO.1, text.to_vec(), false, screen);
    slide(iso, exe, &d, b, screen)
}

/// Santino's face point.
pub const SANTINO: (i32, i32) = (0x86, 0x32);

/// The game's one quote text buffer (0x2299BC, 200 bytes, +1 encoding): the guide, Brenda, Santino, merc messages,
/// A.I.M., Walter and the sector signs all read their EDT record into it.
static SHARED: std::sync::Mutex<Vec<u8>> = std::sync::Mutex::new(Vec::new());

/// strategic.md "The guide's advice" step 2: the 200-byte record at byte `at` of the EDT file `edt` goes into the
/// shared buffer (a record past the end of the file reads nothing, so the buffer keeps the last quote's text; showing
/// a text does not change the buffer); the result is the decoded buffer.
pub fn shared_text(edt: &[u8], at: usize) -> Vec<u8> {
    crate::aim::decode(&shared_raw(edt, at))
}

/// The shared text buffer after the read of the record at `at` (still +1 encoded): a read past the file's end leaves the old text.
pub(crate) fn shared_raw(edt: &[u8], at: usize) -> Vec<u8> {
    let mut s = SHARED.lock().unwrap_or_else(|e| e.into_inner());
    s.resize(200, 0);
    if let Some(r) = edt.get(at..) {
        let n = r.len().min(200);
        s[..n].copy_from_slice(&r[..n]);
    }
    s.clone()
}

/// The target buffer 0x229B1C of StrReplace 0x252B0: what a run leaves in it stays for the next run.
static REPLACED: std::sync::Mutex<Vec<u8>> = std::sync::Mutex::new(Vec::new());

/// StrReplace 0x252B0 with its bug (issue #2 row 44; fan fix F04 removes it): the place counter of the part before `needle` is a signed byte, so from
/// place 128 on the bytes go in front of the target, and the end mark too. The text then goes on with the old bytes of
/// the target from place 128 up to their first 0, and `rep` and the rest come after them. Bytes in front of the target are not kept.
pub(crate) fn str_replace(src: &[u8], rep: &[u8], needle: &[u8]) -> Vec<u8> {
    let mut d = REPLACED.lock().unwrap_or_else(|e| e.into_inner());
    str_replace_in(&mut d, src, rep, needle)
}

/// `str_replace` on the target buffer `d`.
fn str_replace_in(d: &mut Vec<u8>, src: &[u8], rep: &[u8], needle: &[u8]) -> Vec<u8> {
    let cut = |b: &[u8]| b[..b.iter().position(|&c| c == 0).unwrap_or(b.len())].to_vec();
    let src = cut(src);
    d.resize(1024, 0);
    let Some(at) = src.windows(needle.len()).position(|v| v == needle) else {
        d[..src.len()].copy_from_slice(&src);
        d[src.len()] = 0;
        return src;
    };
    // Fan fix F04 (#70 row 64, #2 row 44; changelog 1.15l "Fixed a memory corruption problem in the display of some
    // merc's messages (which resulted in badly formed and cut text)", found by DarzaR): 1.15m reads the byte counter
    // unsigned (0x252EB and 0x25303: movsx -> movzx), so every byte of the part before `needle` lands in the target.
    if ja1logic::fanfix::on(4) {
        for (i, &b) in src[..at].iter().enumerate() {
            d[i & 0xFF] = b;
        }
        d[at & 0xFF] = 0;
    } else {
        for (i, &b) in src[..at].iter().enumerate() {
            if i < 128 {
                d[i] = b;
            }
        }
        if at < 128 {
            d[at] = 0;
        }
    }
    for part in [rep, &src[at + needle.len()..]] {
        let e = d.iter().position(|&c| c == 0).unwrap_or(0);
        d[e..e + part.len()].copy_from_slice(part);
        d[e + part.len()] = 0;
    }
    cut(&d)
}

/// A wait that only Enter, Escape, Space or a click ends.
pub const UNTIL_KEY: u32 = u32::MAX;

/// strategic.md "The guide's advice" step 2: a quote record past the end of its EDT file (the plant-repair quote
/// of an unlisted capacity) reads no text. With Subtitles on its box waits for a key or a click (seen live 2026-10-28).
/// With Subtitles off the face does not wait for a key (#2 row 62, seen live 2026-11-06): see npc_waits.
fn past_end(edt: &[u8], q: i32) -> bool {
    200 * q.max(0) as usize + 200 > edt.len()
}

/// panel.md "The guide's and Brenda's balloons": steps 1 .. 8 for host op 302 guide_say / 317 brenda_say with the
/// arguments `a` over `screen`: face member "64" (the guide) or "65" (Brenda) at Santino's face point, no text.
/// The wait without a voice is the wait of each part (80 ms per byte of its quote text), plus 1000 ms after each
/// part when the quote has a second part.
pub fn npc_screens(iso: &str, exe: &str, op: i32, a: &[i32], screen: &[u8]) -> Result<(Vec<Vec<u8>>, u32), i32> {
    let (who, c, qs) = crate::sound::npc_parts(op, a).ok_or(crate::E_ARG)?;
    let d = crate::panel::panel_data(iso, exe)?;
    let inv = crate::invdraw::inv_data(iso, exe)?;
    let edt = crate::aim::edt(iso, exe, &format!("{}QUOTE.EDT", crate::sound::CATS[c]));
    let text = |q: i32| {
        let at = 200 * q.max(0) as usize;
        shared_text(&edt, at)
    };
    let b = Balloon::at(face_sheet(&inv, &who.to_string()), SANTINO.0, SANTINO.1, text(qs[0]), false, screen);
    let wait = npc_waits(iso, exe, op, a)?.iter().map(|&(_, t, h)| if t == UNTIL_KEY { UNTIL_KEY } else { t + h as u32 })
        .fold(0u32, |a, b| a.saturating_add(b));
    let (v, _) = slide(iso, exe, &d, b, screen)?;
    Ok((v, wait))
}

/// panel.md "The guide's and Brenda's balloons", Subtitles off: the waits (SpeechWait? 0x28034 calls in SpeechBalloon?
/// 0x29544) of host op 302 / 317 with the arguments `a`, one per part: (quote, text time in ms, hold in ms after the
/// talk). Each part is its own wait: the face animation starts again and the talk ends with the part's voice, then
/// the hold passes with the face quiet (1000 ms when the quote has a second part, else 0). The text time of both
/// parts is 80 ms per byte of the FIRST part's text (0x29544 measures the text once, before the first wait).
/// A record past the end of the EDT file keeps the old text and plays the kept-bytes voice (#2 row 62): no key wait.
pub fn npc_waits(iso: &str, exe: &str, op: i32, a: &[i32]) -> Result<Vec<(i32, u32, i64)>, i32> {
    let (_, c, qs) = crate::sound::npc_parts(op, a).ok_or(crate::E_ARG)?;
    let edt = crate::aim::edt(iso, exe, &format!("{}QUOTE.EDT", crate::sound::CATS[c]));
    // Each PlayQuote 0x24F08 reads its record into the shared text buffer, in turn.
    let lens: Vec<u32> = qs.iter().map(|&q| shared_text(&edt, 200 * q.max(0) as usize).len() as u32).collect();
    let hold = if qs.len() > 1 { 1000 } else { 0 };
    // A record past the end (#2 row 62) reads no text: the wait without a voice is 80 ms per byte of the old text in the
    // shared buffer (SpeechWait? 0x28034, voice flag off); with a voice it ends with the kept-bytes stream, no key (seen live
    // 2026-11-06, #2 comment 12038).
    Ok(qs.iter().map(|&q| (q, 80 * lens[0], hold)).collect())
}

/// panel.md "The guide's and Brenda's balloons", Subtitles on: per part of host op 302 / 317 (arguments `a`) the
/// quote, one screen = `screen` with the text-only merc-face box (profile 0x40 / 0x41, nickname `Jack` / `Brenda`,
/// panel picture `panel`) and the wait without a voice (80 ms per byte of the part's text; no extra 1000 ms).
pub fn npc_sub_screens(iso: &str, exe: &str, op: i32, a: &[i32], screen: &[u8], panel: &str)
                       -> Result<Vec<(i32, Vec<u8>, u32)>, i32> {
    let (who, c, qs) = crate::sound::npc_parts(op, a).ok_or(crate::E_ARG)?;
    let d = crate::panel::panel_data(iso, exe)?;
    let inv = crate::invdraw::inv_data(iso, exe)?;
    let cm = crate::common(iso)?;
    let edt = crate::aim::edt(iso, exe, &format!("{}QUOTE.EDT", crate::sound::CATS[c]));
    let nick: &[u8] = if op == 317 { b"Brenda" } else { b"Jack" };
    Ok(qs.iter().map(|&q| {
        let at = 200 * q.max(0) as usize;
        let text = shared_text(&edt, at);
        let mut scr = screen.to_vec();
        let mut table0 = cm.remap[0];
        let mut p = Painter { d: &d, buf: &mut scr, table0: &mut table0, table29: crate::panel::table29(&cm) };
        crate::question::draw_face_text(&mut p, &inv, &text, who, nick, panel);
        (q, scr, if past_end(&edt, q) { UNTIL_KEY } else { 80 * text.len() as u32 })
    }).collect())
}

/// panel.md "Say it again (key L)": `screen` with the text-only merc-face box of the world's last quote on the
/// panel picture `panel`, and the shown text; None when the speaker byte names nobody (nothing shows).
pub fn say_again(iso: &str, exe: &str, w: &crate::world::World, panel: &str, screen: &[u8]) -> Result<Option<(Vec<u8>, Vec<u8>)>, i32> {
    let Some((k, name, text)) = again_text(iso, exe, w) else { return Ok(None) };
    let d = crate::panel::panel_data(iso, exe)?;
    let inv = crate::invdraw::inv_data(iso, exe)?;
    let cm = crate::common(iso)?;
    let mut scr = screen.to_vec();
    let mut table0 = cm.remap[0];
    let mut p = Painter { d: &d, buf: &mut scr, table0: &mut table0, table29: crate::panel::table29(&cm) };
    crate::question::draw_face_text(&mut p, &inv, &text, k, &name, panel);
    Ok(Some((scr, text)))
}

/// panel.md "Say it again (key L)" steps 1 .. 3: the speaker k, the name n and the text s of the world's last quote;
/// None when k names nobody.
pub fn again_text(iso: &str, exe: &str, w: &crate::world::World) -> Option<(u8, Vec<u8>, Vec<u8>)> {
    const PROFILE: usize = 0x5A;
    let q = &w.last_quote;
    let k = q[0];
    let nick = |p: usize| -> Vec<u8> {
        w.profiles.get(p * PROFILE..p * PROFILE + 10).unwrap_or(&[]).iter().copied().take_while(|&b| b != 0).collect()
    };
    let name = match k {
        0..=0x3F => nick(k as usize),
        0x40 => b"Jack".to_vec(),
        0x41 => b"Brenda".to_vec(),
        0x42 => b"Santino".to_vec(),
        _ => return None,
    };
    let cat = crate::sound::CATS.get(q[1] as usize)?;
    let rec = i16::from_le_bytes([q[2], q[3]]) as i64 + i16::from_le_bytes([q[4], q[5]]) as i64;
    let edt = crate::aim::edt(iso, exe, &format!("{}QUOTE.EDT", cat));
    let at = 200 * rec.max(0) as usize;
    let raw = shared_raw(&edt, at);
    // Steps 3 (+1 encoded, as the record): profile m's nickname; with m >= 60 the original skips the encode and uses
    // what g_textBuf holds (issue #2).
    let rep: Vec<u8> = if w.quote_member < 60 {
        nick(w.quote_member as usize).iter().map(|&b| b.wrapping_add(1)).collect()
    } else {
        w.text_buf.iter().copied().take_while(|&b| b != 0).collect()
    };
    let text = crate::aim::decode(&str_replace(&raw, &rep, b"NFNCFS"));
    Some((k, name, text))
}

/// AimComplainQuote 0x44D28 (host op 289 cp_quote): the boxes of one complaint of speaker profile p about the merc with the
/// nickname `other`, as (quote, decoded text). The first complaint of a morning (said flag still 0) has PST quote 7 first,
/// as it stands. Quote q goes through StrReplace 0x252B0 (MEMBER = the nickname) and the result back into the shared buffer.
pub fn complain_texts(iso: &str, exe: &str, p: u8, q: i32, other: &[u8], first: bool) -> Vec<(i32, Vec<u8>)> {
    let edt = crate::aim::edt(iso, exe, "PSTQUOTE.EDT");
    let row = 15 * p as usize;
    let mut v = Vec::new();
    if first {
        v.push((7, shared_text(&edt, 200 * (row + 7))));
    }
    let raw = shared_raw(&edt, 200 * (row as i64 + q as i64).max(0) as usize);
    let rep: Vec<u8> = other.iter().map(|&b| b.wrapping_add(1)).collect();
    let done = str_replace(&raw, &rep, b"NFNCFS");
    // The copy back of 0x44E31 ends after the 0 byte; the old bytes after it stay in the buffer.
    let mut s = SHARED.lock().unwrap_or_else(|e| e.into_inner());
    let n = done.len().min(200);
    s[..n].copy_from_slice(&done[..n]);
    if n < 200 {
        s[n] = 0;
    }
    v.push((q, crate::aim::decode(&done)));
    v
}

/// HatedCheck? 0x44C10 (host op 291 hate_quote): PST quote 6 of speaker profile p. StrReplace fills only its own target
/// (the name goes through a stack copy, g_textBuf stays) and the box shows the shared buffer as it was read. No PST quote 6
/// holds MEMBER, so a player sees no difference from 0x44D28's copy back.
pub fn hate_text(iso: &str, exe: &str, p: u8, other: &[u8]) -> Vec<u8> {
    let edt = crate::aim::edt(iso, exe, "PSTQUOTE.EDT");
    let raw = shared_raw(&edt, 200 * (15 * p as usize + 6));
    let rep: Vec<u8> = other.iter().map(|&b| b.wrapping_add(1)).collect();
    str_replace(&raw, &rep, b"NFNCFS");
    crate::aim::decode(&raw)
}

/// The boxes of a merc's morning quote as (quote, decoded text). Quote 6: HatedCheck? 0x44C10. Quotes 13 and 14:
/// RaiseSalaryPrompt? 0x448A8 after the offer (no replace). The others: AimComplainQuote? 0x44D28 and the reason quote
/// 1..5 of 0x448A8, which replaces MEMBER with `other` and copies the result back in the same way.
pub fn say_boxes(iso: &str, exe: &str, p: u8, q: i32, other: &[u8], first: bool) -> Vec<(i32, Vec<u8>)> {
    match q {
        6 => vec![(6, hate_text(iso, exe, p, other))],
        13 | 14 => vec![(q, shared_text(&crate::aim::edt(iso, exe, "PSTQUOTE.EDT"), 200 * (15 * p as usize + q as usize)))],
        _ => complain_texts(iso, exe, p, q, other, first),
    }
}

/// The game's two text buffers: the shared buffer 0x2299BC and the target 0x229B1C of StrReplace, for complainquote_diff.
pub fn quote_buffers() -> (Vec<u8>, Vec<u8>) {
    (SHARED.lock().unwrap_or_else(|e| e.into_inner()).clone(), REPLACED.lock().unwrap_or_else(|e| e.into_inner()).clone())
}

/// 0x50908 at the morning's face point (x0, 20) with the text `text` (decoded): the screens and the wait of `pay_screens`.
#[allow(clippy::too_many_arguments)]
pub fn text_screens(iso: &str, exe: &str, x0: i32, p: u8, nick: &[u8], sub: bool, text: Vec<u8>, screen: &[u8])
                    -> Result<(Vec<Vec<u8>>, u32), i32> {
    talk_box(iso, exe, x0, 20, p, nick, sub, text, screen)
}

/// 0x50908: a merc talks. Subtitles off: the balloon with the face at (x0, y0). Subtitles on: one screen, the merc-face
/// box with the text only. Both give the wait of 80 ms per text byte.
#[allow(clippy::too_many_arguments)]
pub fn talk_box(iso: &str, exe: &str, x0: i32, y0: i32, p: u8, nick: &[u8], sub: bool, text: Vec<u8>, screen: &[u8])
                -> Result<(Vec<Vec<u8>>, u32), i32> {
    let d = crate::panel::panel_data(iso, exe)?;
    let inv = crate::invdraw::inv_data(iso, exe)?;
    let wait = 80 * text.len() as u32;
    if sub {
        let c = crate::common(iso)?;
        let mut scr = screen.to_vec();
        let mut table0 = c.remap[0];
        let mut q = Painter { d: &d, buf: &mut scr, table0: &mut table0, table29: crate::panel::table29(&c) };
        crate::question::draw_face_text(&mut q, &inv, &text, p, nick, "PANEL_P.PCC");
        return Ok((vec![scr], wait));
    }
    let b = Balloon::at(face_sheet(&inv, &p.to_string()), x0, y0, text, false, screen);
    let (v, _) = slide(iso, exe, &d, b, screen)?;
    Ok((v, wait))
}

/// strategic.md "Not enough funds", Dismiss (ops 276 / 277): the dismissed merc (profile p) speaks PST quote 0
/// (text record 15 p). Subtitles off: steps 1 .. 8 of "Speech balloon" with face member p at the face point
/// (x0, 20), no text; the 9 screens and the wait without a voice (80 ms per byte of the quote text). Subtitles on:
/// one screen, the merc-face box with the text only ("Merc-face question box" with ECX 0, PANEL_P.PCC), the same
/// wait; the box shows the nickname `nick` (profile +0).
pub fn pay_screens(iso: &str, exe: &str, x0: i32, p: u8, nick: &[u8], sub: bool, screen: &[u8]) -> Result<(Vec<Vec<u8>>, u32), i32> {
    face_quote_screens(iso, exe, x0, 20, p, nick, sub, "PSTQUOTE.EDT", 15 * p as usize, screen)
}

/// strategic.md "Firing a merc", host op 328 fire_quote: the fired AIM merc (profile p, face x x0) speaks AIM quote 9
/// (text record 14 p + 9) in the balloon of 0x50908, the same screens and wait as `pay_screens`.
pub fn fire_screens(iso: &str, exe: &str, x0: i32, y0: i32, p: u8, nick: &[u8], sub: bool, screen: &[u8]) -> Result<(Vec<Vec<u8>>, u32), i32> {
    face_quote_screens(iso, exe, x0, y0, p, nick, sub, "AIMQUOTE.EDT", 14 * p as usize + 9, screen)
}

/// The balloon of 0x50908 (face point (x0, y0)) with the text record `rec` of the EDT file `edt`.
#[allow(clippy::too_many_arguments)]
fn face_quote_screens(iso: &str, exe: &str, x0: i32, y0: i32, p: u8, nick: &[u8], sub: bool, edt: &str, rec: usize, screen: &[u8])
                      -> Result<(Vec<Vec<u8>>, u32), i32> {
    let edt = crate::aim::edt(iso, exe, edt);
    talk_box(iso, exe, x0, y0, p, nick, sub, record_text(&edt, rec), screen)
}

/// The text of record `rec` of an EDT quote file (records of 200 bytes, as `./t speech text`).
pub fn record_text(edt: &[u8], rec: usize) -> Vec<u8> {
    shared_text(edt, 200 * rec)
}

/// PlayHost::say voices only the first balloon of a pass; balloon `bn` of the pass speaks when it shows if it is not
/// the first, or if a button release wait before it (TimeCompress(0) at the top of ShowMercMessage? 0x299B8) stopped
/// that first voice (`release_ms` > 0).
pub fn voice_when_shown(bn: usize, release_ms: u128) -> bool {
    bn > 0 || release_ms > 0
}

/// After a balloon's view move: inside a pass the slide waits in the list (the window plays it after the pass);
/// outside a pass `slides` plays it now. False when the window closed in the slide.
pub fn goes_on_after_view_move(in_pass: bool, slides: impl FnOnce() -> bool) -> bool {
    in_pass || slides()
}

/// The ticks of the balloons that waited inside a pass (balloon_win): the drag ticks and the locked ticks the window
/// runs after the pass (each only when not 0); `ticks_done` moves on by both.
pub fn owed_after_pass((drag, held): (u64, u64), ticks_done: &mut u64) -> (Option<u64>, Option<u64>) {
    *ticks_done += drag + held;
    ((drag > 0).then_some(drag), (held > 0).then_some(held))
}

/// SpeechBalloon? 0x29544 in the guide balloon window: with two or more parts the first part's voice starts here
/// (the second starts after the first wait); one part has its voice from the host.
pub fn first_voice_here(parts: usize) -> bool {
    parts > 1
}

/// The nickname (bytes up to the first 0, at most 10) of profile `p` (0x5A bytes each); none for p < 0.
pub fn profile_nick(profiles: &[u8], p: i32) -> Vec<u8> {
    if p < 0 {
        return Vec::new();
    }
    profiles.get(p as usize * 0x5A..p as usize * 0x5A + 10).unwrap_or(&[]).iter().copied().take_while(|&b| b != 0).collect()
}

#[cfg(test)]
mod record_tests {
    /// Ledger row c2aea50:ja1front:2359: the face balloon and the text-only box wait for the release; a held frame
    /// that is not the first, or a frame count of 2..8, does not.
    #[test]
    fn face_and_text_boxes_wait_for_the_release() {
        assert!(super::waits_release(9, 4));
        // Ledger rows d057aca:ja1front:2064/2065/2079: only the held frame waits for the release and moves the face.
        assert_eq!(super::frame_wait(4, 9, 4), super::FrameWait { release: true, face: true });
        assert_eq!(super::frame_wait(3, 9, 4), super::FrameWait { release: false, face: false });
        assert_eq!(super::frame_wait(0, 1, 0), super::FrameWait { release: true, face: true });
        assert_eq!(super::frame_wait(2, 5, 2), super::FrameWait { release: false, face: false });
        // Ledger row d057aca:ja1front:2072: part 0 runs with the frame as shown, later parts start again.
        assert!(!super::part_restarts(0));
        // Ledger rows d057aca:ja1front:936/937: only the held frame waits for the voice.
        assert!(super::held_voiced(4, 4, true));
        assert!(!super::held_voiced(3, 4, true));
        assert!(!super::held_voiced(4, 4, false));
        // Ledger row d057aca:ja1front:919: only the last-death quote after the pass's stop holds the combat.
        assert!(super::victory_quote_holds_combat(Some(0x16), 0, 20000));
        assert!(!super::victory_quote_holds_combat(Some(0x16), 1, 20000));
        assert!(!super::victory_quote_holds_combat(Some(0x16), 0, 19000));
        assert!(!super::victory_quote_holds_combat(Some(0x15), 0, 20000));
        assert!(!super::victory_quote_holds_combat(None, 0, 20000));
        assert!(super::part_restarts(1));
        assert!(super::waits_release(9, 0));
        assert!(super::waits_release(1, 0));
        assert!(!super::waits_release(1, 1));
        assert!(!super::waits_release(5, 0));
    }

    /// Ledger rows c2aea50:ja1front:2141/2143: only a guide balloon of two or more parts starts its first voice here.
    #[test]
    fn a_two_part_balloon_starts_its_first_voice() {
        assert!(!super::first_voice_here(1));
        assert!(super::first_voice_here(2));
        assert!(super::first_voice_here(3));
    }

    /// Ledger row c2aea50:ja1front:2179: profile 0 has a nickname too; p < 0 has none.
    #[test]
    fn profile_0_has_a_nickname() {
        let mut pr = vec![0u8; 0x5A * 2];
        pr[..4].copy_from_slice(b"IVAN");
        pr[0x5A..0x5A + 10].copy_from_slice(b"ABCDEFGHIJ");
        assert_eq!(super::profile_nick(&pr, 0), b"IVAN");
        assert_eq!(super::profile_nick(&pr, 1), b"ABCDEFGHIJ");
        assert!(super::profile_nick(&pr, -1).is_empty());
    }

    /// Ledger rows c2aea50:ja1front:1679/1682/1687/1689: after a pass the window runs the drag and locked ticks that
    /// the balloons owe, and its tick count moves on by both.
    #[test]
    fn owed_balloon_ticks_run_after_the_pass() {
        let mut t = 100;
        assert_eq!(super::owed_after_pass((3, 0), &mut t), (Some(3), None));
        assert_eq!(t, 103);
        assert_eq!(super::owed_after_pass((0, 4), &mut t), (None, Some(4)));
        assert_eq!(t, 107);
        assert_eq!(super::owed_after_pass((2, 5), &mut t), (Some(2), Some(5)));
        assert_eq!(t, 114);
        assert_eq!(super::owed_after_pass((0, 0), &mut t), (None, None));
        assert_eq!(t, 114);
    }

    /// Ledger row c2aea50:ja1front:940: the first balloon of a pass has its voice from PlayHost::say; it speaks
    /// again only after a button release wait stopped it. Later balloons speak when they show.
    #[test]
    fn only_later_or_stopped_balloons_speak_when_shown() {
        assert!(!super::voice_when_shown(0, 0));
        assert!(super::voice_when_shown(0, 12));
        assert!(super::voice_when_shown(1, 0));
    }

    /// Ledger row c2aea50:ja1front:1022: inside a pass the slide waits (no window slide now, the balloon loop goes on);
    /// outside a pass the slide plays now and a closed window ends the loop.
    #[test]
    fn a_slide_in_a_pass_waits() {
        let mut ran = false;
        assert!(super::goes_on_after_view_move(true, || { ran = true; false }));
        assert!(!ran);
        assert!(super::goes_on_after_view_move(false, || true));
        assert!(!super::goes_on_after_view_move(false, || false));
    }

    /// Ledger row 0b4d529:ja1front:5140 (strategic.md "Not enough funds", Dismiss: PST quote 0 of profile p = text
    /// record 15 p of PSTQUOTE.EDT): the dismissed merc's box shows record 15 p.
    #[test]
    fn the_dismissed_merc_says_record_15_p() {
        let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
        let (iso, exe) = (root.join("rom/JA1.gog"), root.join("build/game113b/JA.EXE"));
        if !iso.exists() || !exe.exists() {
            return;
        }
        let (iso, exe) = (iso.to_str().unwrap(), exe.to_str().unwrap());
        let screen = vec![0u8; 320 * 200];
        for p in [2u8, 5] {
            let got = super::pay_screens(iso, exe, 100, p, b"NICK", true, &screen).unwrap();
            let want = super::face_quote_screens(iso, exe, 100, 20, p, b"NICK", true, "PSTQUOTE.EDT", 15 * p as usize, &screen).unwrap();
            assert!(got == want, "profile {p}");
            let other = super::face_quote_screens(iso, exe, 100, 20, p, b"NICK", true, "PSTQUOTE.EDT", 15 + p as usize, &screen).unwrap();
            assert!(got != other, "profile {p}: records 15 p and 15 + p must differ for this test");
        }
    }

    /// Ledger row c2aea50:ja1front:5248: record k of an EDT file starts at byte 200 k.
    #[test]
    fn edt_records_are_200_bytes() {
        let mut edt = vec![0u8; 800];
        edt[600..603].copy_from_slice(b"abc");
        edt[200..202].copy_from_slice(b"xy");
        assert_eq!(super::record_text(&edt, 3), super::shared_text(&edt, 600));
        assert_eq!(super::record_text(&edt, 1), super::shared_text(&edt, 200));
        assert_ne!(super::record_text(&edt, 3), super::record_text(&edt, 1));
    }

    /// StrReplace 0x252B0: the end mark of the part before the needle goes to dst[(s8) n] (movsx at 0x25302). With the
    /// needle at place 128 it lands in front of the target, so the old bytes of the target from 128 stay in the text.
    #[test]
    fn a_needle_at_place_128_keeps_the_old_bytes() {
        let mut d = Vec::new();
        super::str_replace_in(&mut d, &[b'a'; 200], b"R", b"$N");
        let mut src = vec![b'b'; 128];
        src.extend(b"$Nc");
        let mut want = vec![b'b'; 128];
        want.extend([b'a'; 72]);
        want.extend(b"Rc");
        assert_eq!(super::str_replace_in(&mut d, &src, b"R", b"$N"), want);
        // At place 127 the end mark is in the target: no old bytes.
        let mut src = vec![b'b'; 127];
        src.extend(b"$Nc");
        let mut want = vec![b'b'; 127];
        want.extend(b"Rc");
        assert_eq!(super::str_replace_in(&mut d, &src, b"R", b"$N"), want);
    }

    /// A pixel read off the screen (left, right, above or below) is 0; no read wraps to the next row.
    #[test]
    fn get_off_the_screen_is_0() {
        let screen = vec![9u8; 64000];
        assert_eq!(super::get(&screen, 319, 199), 9);
        assert_eq!(super::get(&screen, 320, 0), 0);
        assert_eq!(super::get(&screen, -1, 5), 0);
        assert_eq!(super::get(&screen, 5, 200), 0);
    }

    /// A quote record (200 bytes) is past the end only when its last byte is past the end of the EDT file.
    #[test]
    fn a_record_is_past_the_end_when_it_does_not_fit() {
        let edt = vec![0u8; 400];
        assert!(!super::past_end(&edt, 0));
        assert!(!super::past_end(&edt, 1));
        assert!(super::past_end(&edt, 2));
        assert!(super::past_end(&edt, 5));
        assert!(super::past_end(&edt[..399], 1));
    }

    /// The face that the wait animates sits where the last slide-in frame (56 x 50, no scaling) drew it: balloon + (1, 1).
    #[test]
    fn the_animated_face_sits_on_the_slide_face() {
        let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
        let (iso, exe) = (root.join("rom/JA1.gog"), root.join("build/game113b/JA.EXE"));
        if !iso.exists() || !exe.exists() {
            return;
        }
        let (iso, exe) = (iso.to_str().unwrap(), exe.to_str().unwrap());
        let d = crate::panel::panel_data(iso, exe).unwrap();
        let b = super::Balloon { bx: 100, by: 60, tx: 0, ty: 0, face: vec![7u8; super::FW * super::FH],
                                 save: vec![0u8; super::BW * super::BH], text: Vec::new(), text_on: false, hurt: false };
        let (frames, _) = super::slide(iso, exe, &d, b, &vec![0u8; 64000]).unwrap();
        let full = &frames[super::SLIDE_IN.len() - 1];
        let spot = super::last_face().unwrap();
        let mut again = full.clone();
        spot.paint(&mut again, 0, 0);
        assert!(again == *full);
    }
}

/// The name of the speaker of host op 302 / 317 in the `balloon NAME X Y` script line.
pub fn npc_name(op: i32) -> &'static str {
    if op == 317 {
        "brenda"
    } else {
        "guide"
    }
}

fn slide(iso: &str, _exe: &str, d: &crate::panel::PanelData, b: Balloon, screen: &[u8]) -> Result<(Vec<Vec<u8>>, u32), i32> {
    let c = crate::common(iso)?;
    let mut scr = screen.to_vec();
    let mut table0 = c.remap[0];
    let mut p = Painter { d, buf: &mut scr, table0: &mut table0, table29: crate::panel::table29(&c) };
    let mut out = Vec::with_capacity(9);
    let spot = crate::faceanim::FaceSpot { sheet: b.face.clone(), x: b.bx + 1, y: b.by + 1, b: b.hurt as i32 };
    LAST.with(|l| *l.borrow_mut() = Some(spot));
    for (w, h) in SLIDE_IN {
        b.frame(&mut p, w, h);
        out.push(p.buf.to_vec());
    }
    b.draw_text(&mut p);
    out.push(p.buf.to_vec());
    for (w, h) in SLIDE_OUT {
        b.frame(&mut p, w, h);
        out.push(p.buf.to_vec());
    }
    Ok((out, b.wait_ms()))
}

/// Step 2: the decoded DAYQUOTE.EDT record 33 p + q.
fn quote(quotes: &[u8], p: usize, q: i32) -> Vec<u8> {
    let at = (33 * p + q.clamp(0, 32) as usize) * 200;
    shared_text(quotes, at)
}

/// The quote text of record `r` (profile +0x179) and quote `q`.
pub fn quote_text(iso: &str, exe: &str, r: &[u8], q: i32) -> Vec<u8> {
    quote(&crate::aim::edt(iso, exe, "DAYQUOTE.EDT"), r[0x179] as usize, q)
}

/// Santino's quote `q` (the decoded SANQUOTE.EDT record q, 200 bytes).
pub fn santino_text(iso: &str, exe: &str, q: i32) -> Vec<u8> {
    let at = 200 * q.max(0) as usize;
    shared_text(&crate::aim::edt(iso, exe, "SANQUOTE.EDT"), at)
}
