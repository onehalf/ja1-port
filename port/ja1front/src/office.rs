//! The office (strategic.md "The office", world_load.md section 9 step 2): the room picture with the calendar's
//! number, the hot spots with their labels and highlights, the Sleep message and the dark room after Sleep.

use std::path::Path;
use std::sync::Arc;

use crate::panel::{self, Painter, PanelData};
use crate::world::World;
use crate::data::Common;
use crate::saveslots::{self, Headers, Outcome, SlotList};
use crate::switchbox::{Step, SwitchBox};

const PW: usize = 320;
const PH: usize = 200;

/// Hot spot rectangles (x0, y0, x1, y1), inclusive.
const RECTS: [(i32, i32, i32, i32); 8] = [(178, 40, 217, 60), (178, 62, 217, 82), (152, 113, 202, 123),
    (152, 91, 202, 123), (0, 122, 102, 162), (53, 60, 112, 80), (0, 21, 24, 70), (270, 25, 319, 166)];
const LABELS: [&str; 8] = ["Save Game", "Restore Game", "Contact A.I.M.", "Contact A.I.M.", "Sleep", "View Team",
    "Control Panel", "Leave"];
/// Highlight size w x h and place of hot spots 1..7 (3 as 2; 0 has none).
const SIZE: [(i32, i32); 8] = [(0, 0), (60, 24), (72, 33), (72, 33), (100, 39), (50, 32), (38, 66), (50, 147)];
const AT: [(i32, i32); 8] = [(0, 0), (169, 60), (152, 91), (152, 91), (0, 130), (53, 61), (0, 18), (270, 21)];
/// The four highlight frames of each hot spot: places in OFFBRUSH.PCX.
const FRAMES: [[(i32, i32); 4]; 8] = [
    [(0, 0); 4],
    [(100, 103), (160, 103), (220, 103), (280, 103)],
    [(200, 66), (272, 66), (552, 0), (552, 33)],
    [(200, 66), (272, 66), (552, 0), (552, 33)],
    [(0, 0), (0, 39), (0, 78), (100, 0)],
    [(100, 39), (150, 39), (100, 71), (150, 71)],
    [(200, 0), (238, 0), (276, 0), (314, 0)],
    [(352, 0), (402, 0), (452, 0), (502, 0)],
];

pub const SLEEP_MSG: &str = "You try to sleep, but the worry of not having any men on duty tomorrow keeps you awake...";
/// Hot spot 4 = Sleep.
pub const SLEEP: usize = 4;
/// Hot spot 6 = Control Panel.
pub const CONTROL_PANEL: usize = 6;
/// Hot spot 7 = Leave.
pub const LEAVE: usize = 7;
/// The message box of the office: darkening table 31, frame colour 0xFF.
const MSG_TABLE: usize = 31;

pub type Pic = (usize, usize, Vec<u8>);

/// The pictures of DAT/OFFICE.DAT.
pub struct OfficeData {
    room: Pic,
    room_pal: [u8; 768],
    dark: Pic,
    dark_pal: [u8; 768],
    case1: Pic,
    case1d: Pic,
    case2: Pic,
    case2d: Pic,
    brush: Pic,
    view: Pic,
    view_pal: [u8; 768],
    /// TEXTSLAB.PCC, the row picture of the slot list (strategic.md "Save Game and Restore Game").
    pub slab: Pic,
    /// PANBRUSH.PCX and SWICHBOX.PCC (strategic.md "The switch box (Control Panel)").
    pub panbrush: Pic,
    pub swbox: Pic,
}

fn member(arc: &[u8], name: &str) -> Result<Vec<u8>, i32> {
    panel::archive_member(arc, name).map_err(|_| crate::E_FILE)
}

fn picture(arc: &[u8], name: &str) -> Result<Pic, i32> {
    panel::pcx(&member(arc, name)?).map_err(|_| crate::E_FILE)
}

/// The palette of a PCX file: its last 768 bytes, each shifted right by 2 (6-bit values).
fn palette(arc: &[u8], name: &str) -> Result<[u8; 768], i32> {
    let f = member(arc, name)?;
    let tail = f.get(f.len().saturating_sub(768)..).filter(|t| t.len() == 768).ok_or(crate::E_FILE)?;
    let mut pal = [0u8; 768];
    for (i, v) in tail.iter().enumerate() {
        pal[i] = v >> 2;
    }
    Ok(pal)
}

impl OfficeData {
    pub fn load(exe_path: &str) -> Result<OfficeData, i32> {
        let dir = Path::new(exe_path).parent().map(|p| p.to_path_buf()).unwrap_or_default();
        let arc = std::fs::read(dir.join("DAT").join("OFFICE.DAT")).map_err(|_| crate::E_FILE)?;
        Ok(OfficeData {
            room: picture(&arc, "ROOM.PCX")?,
            room_pal: palette(&arc, "ROOM.PCX")?,
            dark: picture(&arc, "ROOMDARK.PCX")?,
            dark_pal: palette(&arc, "ROOMDARK.PCX")?,
            case1: picture(&arc, "CASE1.PCC")?,
            case1d: picture(&arc, "CASE1DRK.PCC")?,
            case2: picture(&arc, "CASE2.PCC")?,
            case2d: picture(&arc, "CASE2DRK.PCC")?,
            brush: picture(&arc, "OFFBRUSH.PCX")?,
            view: picture(&arc, "VIEWISLE.PCX")?,
            view_pal: palette(&arc, "VIEWISLE.PCX")?,
            slab: picture(&arc, "TEXTSLAB.PCC")?,
            panbrush: picture(&arc, "PANBRUSH.PCX")?,
            swbox: picture(&arc, "SWICHBOX.PCC")?,
        })
    }

    /// The room picture and its palette (the setup screen draws the switch box over it).
    pub fn room(&self) -> (&Pic, [u8; 768]) {
        (&self.room, self.room_pal)
    }

    pub fn room_palette(&self) -> [u8; 768] {
        self.room_pal
    }
}

/// Copy all pixels of `pic` to (x, y) of the 320 x 200 buffer (clipped at the buffer).
pub(crate) fn put(buf: &mut [u8], pic: &Pic, x: i32, y: i32) {
    let (w, h, px) = pic;
    for r in 0..*h as i32 {
        for c in 0..*w as i32 {
            let (dx, dy) = (x + c, y + r);
            if (0..PW as i32).contains(&dx) && (0..PH as i32).contains(&dy) {
                buf[dy as usize * PW + dx as usize] = px[r as usize * w + c as usize];
            }
        }
    }
}

/// The game-over screen (ending.md "The game-over screen" steps 2, 3): VIEWISLE.PCX and `GAME OVER!` at (135, 50),
/// not centred, colour 0xFF, shadow 1, font 2; returns its palette (6-bit).
pub fn game_over(d: &OfficeData, p: &mut Painter) -> [u8; 768] {
    put(p.buf, &d.view, 0, 0);
    p.text(b"GAME OVER!", 135, 50, false, 0xFF, 1, 2);
    d.view_pal
}

/// The room picture (dark or not) with the case and the calendar's number; returns its palette (6-bit).
pub fn room(d: &OfficeData, p: &mut Painter, link: u8, day: i32, dark: bool) -> [u8; 768] {
    put(p.buf, if dark { &d.dark } else { &d.room }, 0, 0);
    match (link != 0, dark) {
        (true, false) => put(p.buf, &d.case1, 152, 91),
        (true, true) => put(p.buf, &d.case1d, 152, 91),
        (false, false) => put(p.buf, &d.case2, 127, 137),
        (false, true) => put(p.buf, &d.case2d, 127, 137),
    }
    if day != 0 {
        p.text(day.to_string().as_bytes(), 195, 46, true, if dark { 0x1A } else { 0xFF }, 1, 2);
    }
    if dark { d.dark_pal } else { d.room_pal }
}

/// Highlight frame j (0..3) of hot spot k (1..7) at its place.
pub fn frame(d: &OfficeData, buf: &mut [u8], k: usize, j: usize) {
    let (w, h) = SIZE[k];
    let (x, y) = AT[k];
    let (sx, sy) = FRAMES[k][j];
    let (bw, bh, px) = &d.brush;
    for r in 0..h {
        for c in 0..w {
            let (u, v) = ((sx + c) as usize, (sy + r) as usize);
            let (dx, dy) = (x + c, y + r);
            if u < *bw && v < *bh && (0..PW as i32).contains(&dx) && (0..PH as i32).contains(&dy) {
                buf[dy as usize * PW + dx as usize] = px[v * bw + u];
            }
        }
    }
}

/// The hot spot under (x, y): the first k whose rectangle holds it; 8 = none.
pub fn hot(x: i32, y: i32) -> usize {
    RECTS.iter().position(|&(x0, y0, x1, y1)| x0 <= x && x <= x1 && y0 <= y && y <= y1).unwrap_or(8)
}

/// The label of hot spot k (empty when it does not apply) with the A.I.M. link and the highlight h.
pub fn label(k: usize, link: u8, h: usize) -> &'static str {
    match k {
        2 if link == 0 => "",
        3 if link == 0 || h != 2 => "",
        0..=7 => LABELS[k],
        _ => "",
    }
}

/// The label at the mouse: x = min(mouse x, 295) (above 296: 295), y = mouse y - 10, fg 0xFE, shadow 1, font 1.
pub fn draw_label(p: &mut Painter, s: &str, mx: i32, my: i32) {
    let x = if mx > 0x128 { 0x127 } else { mx };
    p.text(s.as_bytes(), x, my - 10, false, 0xFE, 1, 1);
}

/// Team size: slots in use from slot 0 up to the first with inCurSector (+0x182) = 0, at most 8.
/// Hot spot 5 = View Team (the team ledger).
pub const VIEW_TEAM: usize = 5;

/// A number with `d` digits, padded on the left with `^` (panel.md "Number").
#[allow(clippy::too_many_arguments)]
fn number(p: &mut Painter, v: i32, d: usize, x: i32, y: i32, fg: u8, shadow: u8, font: usize) {
    let mut t = v.to_string().into_bytes();
    while t.len() < d {
        t.insert(0, b'^');
    }
    p.text(&t, x, y, false, fg, shadow, font);
}

/// The team ledger, steps 1 to 3 (strategic.md "The team ledger"): the page on the screen `p` with the mercs of
/// `soldiers` (team slots 0..7) and their 32 x 32 `faces`; the health bars are drawn in the room buffer `room` and
/// copied from there. Returns the page's palette (6-bit).
pub fn ledger_team(d: &OfficeData, p: &mut Painter, room: &mut [u8], soldiers: &[u8], faces: &[u8], day: i32)
    -> [u8; 768] {
    put(p.buf, &d.view, 0, 0);
    p.text(b"Current Team", 95, 8, true, 0xFF, 1, 2);
    p.text(b"Day #", 203, 8, false, 0xFF, 1, 2);
    number(p, day, 3, 233, 8, 0xFF, 8, 2);
    for k in 0..8 {
        let Some(s) = soldiers.get(k * 0x200..(k + 1) * 0x200) else { break };
        if s[0x182] == 0 {
            break;
        }
        let (x, y) = (if k < 4 { 12 } else { 100 }, 45 * (k as i32 % 4));
        let gap = {
            let mut t0 = *p.table0;
            let mut rp = Painter { d: p.d, buf: &mut *room, table0: &mut t0, table29: p.table29 };
            crate::assign::health_bar(&mut rp, s, x + 30, y + 22, false)
        };
        // The drawn rows only (the gap keeps the page picture); fewer than 0 rows = stop point (the original
        // runs off memory), copy nothing.
        for r in gap..25 {
            for c in 0..3 {
                let (bx, by) = (x + 30 + c, y + 22 + r);
                if let Some(&v) = room.get(by as usize * PW + bx as usize) {
                    p.put(bx, by, v);
                }
            }
        }
        if let Some(f) = faces.get(k * 1024..(k + 1) * 1024) {
            for r in 0..25 {
                for c in 0..28 {
                    p.put(x + c, y + 22 + r, f[(r as usize + 2) * 32 + c as usize + 2]);
                }
            }
        }
        p.frame(x - 1, y + 21, x + 28, y + 47, 0x5B);
        let nick = &s[0x16E..0x16E + 9];
        let nick = &nick[..nick.iter().position(|&b| b == 0).unwrap_or(9)];
        p.text(nick, x + 14, y + 49, true, 0x0E, 5, 0);
        for (i, l) in [&b"Sal"[..], b"Hea", b"Lvl", b"Kills"].iter().enumerate() {
            p.text(l, x + 34, y + 23 + 6 * i as i32, false, 0x0E, 5, 0);
        }
        number(p, u16::from_le_bytes([s[0x112], s[0x113]]) as i32, 5, x + 55, y + 23, 0x28, 8, 0);
        number(p, s[0x183] as i8 as i32, 4, x + 59, y + 29, 0x28, 8, 0);
        number(p, s[0x192] as i8 as i32, 4, x + 59, y + 35, 0x28, 8, 0);
        number(p, s[0x1D9] as i32, 4, x + 59, y + 41, 0x28, 8, 0);
    }
    d.view_pal
}

/// The team ledger, step 4: the accounts (`accounts` bytes as counted after step 3) and the difficulty.
pub fn ledger_accounts(p: &mut Painter, acc: &[u8], difficulty: u8) {
    let a = |o: usize| acc.get(o..o + 4).map(|b| i32::from_le_bytes([b[0], b[1], b[2], b[3]])).unwrap_or(0);
    let label = |p: &mut Painter, s: &[u8], y: i32| p.text(s, 203, y, false, 0xE5, 1, 0);
    label(p, b"DIFFICULTY:", 24);
    let dt: &[u8] = match difficulty {
        0 => b"  EASY",
        1 => b"NORMAL",
        2 => b"  HARD",
        _ => b"UNKNOWN",
    };
    p.text(dt, 285, 24, false, 0xFF, 8, 0);
    for (y, l, o) in [(32, &b"Sectors Won:"[..], 0x78), (40, b"Sectors Lost:", 0x7C), (48, b"Sectors Owned:", 0x74)] {
        label(p, l, y);
        number(p, a(o), 4, 293, y, 0xFF, 8, 0);
    }
    label(p, b"Last Sector Gained:", 56);
    if a(0x80) == 59 {
        p.text(b"  NONE", 285, 56, false, 0xFF, 8, 0);
    } else {
        number(p, a(0x80).wrapping_add(1), 4, 293, 56, 0xFF, 8, 0);
    }
    for (y, l, o) in [(64, &b"Factory Capacity:"[..], 0x84), (72, b"Total Trees Owned:", 0x18)] {
        label(p, l, y);
        number(p, a(o), 4, 293, y, 0xFF, 8, 0);
    }
    p.text(b"Expenses", 203, 87, false, 0xFF, 1, 1);
    for (y, l, o) in [(97, &b"Guards"[..], 0x5C), (104, b"Tappers", 0x58), (111, b"Team", 0x38)] {
        p.text(l, 208, y, false, 0xFF, 1, 0);
        number(p, a(o), 5, 289, y, 0xFF, 8, 0);
    }
    p.text(b"TOTAL", 203, 121, false, 0xFF, 1, 1);
    number(p, a(0x5C).wrapping_add(a(0x58)).wrapping_add(a(0x38)), 5, 279, 121, 0xFF, 8, 1);
    p.text(b"Revenue", 203, 137, false, 0xFF, 1, 1);
    p.text(b"Trees", 208, 147, false, 0xFF, 1, 0);
    number(p, a(0x20), 5, 289, 147, 0xFF, 8, 0);
    p.text(b"TOTAL", 203, 157, false, 0xFF, 1, 1);
    number(p, a(0x20), 5, 279, 157, 0xFF, 8, 1);
    p.text(b"FUNDS", 203, 177, false, 0xFF, 1, 1);
    number(p, a(0x24), 6, 273, 177, 0xFF, 8, 1);
}

/// Does key `k` (an ASCII value, 0 = an extended key) close the ledger? (Return, Escape, Space; letters as capitals.)
pub fn ledger_key_closes(k: u8) -> bool {
    matches!(k.to_ascii_uppercase(), 13 | 27 | 32)
}

/// A fade of the colours `pal` (6-bit) over `n` steps from level `from` to `to` (0..256), as in ending.md op 24 / 25:
/// levels l = n · from / 256 .. n · to / 256 toward the end, each colour × l / n; one palette per vertical retrace.
/// The team ledger fades with n = 24 (strategic.md "The team ledger", Fades).
pub fn fade(pal: &[u8; 768], n: i32, from: i32, to: i32) -> Vec<[u8; 768]> {
    let n = n.max(1);
    // 0x52934 / 0x52941: a negative step count, from or to becomes 0.
    let (from, to) = (from.max(0), to.max(0));
    let (l0, l1) = (n * from / 256, n * to / 256);
    let step = if l1 >= l0 { 1 } else { -1 };
    let mut out = Vec::new();
    let mut l = l0;
    loop {
        let mut d = [0u8; 768];
        for (o, &c) in d.iter_mut().zip(pal.iter()) {
            *o = (c as i32 * l / n) as u8;
        }
        out.push(d);
        if l == l1 {
            return out;
        }
        l += step;
    }
}

pub fn team_size(w: &World) -> i32 {
    (0..8).take_while(|&k| w.soldiers.get(k * 0x200 + 0x182).copied().unwrap_or(0) != 0).count() as i32
}

/// strategic.md "The office", Alt+X: the line on the text screen at the program end (0x90DF4).
pub const NORMAL_EXIT: &str = "Normal exit.";

/// What one poll did.
#[derive(Default)]
pub struct Poll {
    /// Highlight pictures shown one after the other (each at least 90 ms) before the end picture.
    pub frames: Vec<Vec<u8>>,
    /// Highlight sounds (sound.md "Main bank" effect, volume): each starts before the frame with the given index
    /// (index = frames.len(): after the last frame); rate 8000, once.
    pub sounds: Vec<(usize, i32, i32)>,
    /// OfficeHighlight 0x4C0C8 calls in order: (hot spot, on = 1 / off = 0).
    pub highlights: Vec<(usize, bool)>,
    /// A left press on hot spot k (8 = none).
    pub click: Option<usize>,
    /// A message box opened.
    pub msg: bool,
    /// Sleep ended the office: the dark room is the picture now (shown 700 ms).
    pub sleep: bool,
    /// A press on View Team: the caller opens the team ledger (`open_ledger`).
    pub ledger: bool,
    /// A press on Save Game (1) or Restore Game (0): the caller opens the slot list (`open_slots`).
    pub slots: Option<i32>,
    /// A press on Leave: the office ends with result 7 (world_load.md section 12).
    pub leave: bool,
    /// A press on Contact A.I.M. while the link is not 0: the caller opens the A.I.M. screen (aim.rs).
    pub aim: bool,
    /// Field 3: the picture fades before the music fade (0x4F313); field 2: after it (0x4F297).
    pub aim_fade_first: bool,
    /// A press on Control Panel: the caller opens the switch box (`open_switchbox`).
    pub switchbox: bool,
}

/// The office loop's state.
pub struct Office {
    data: OfficeData,
    pd: Arc<PanelData>,
    c: Arc<Common>,
    link: u8,
    day: i32,
    team: i32,
    /// The screen without the label: the room and the highlight.
    base: Vec<u8>,
    /// The rectangle under the highlight, saved when it went on.
    saved: Vec<u8>,
    /// The hot spot with a highlight (0 = none).
    pub h: usize,
    mouse: Option<(i32, i32)>,
    /// The label is on the screen (it went on at the last mouse move and the room was redrawn since).
    label_on: bool,
    /// The next poll runs the moved branch without a move: the original's redraw flag (local_28) after the Sleep
    /// message (0x4F3CE).
    redraw: bool,
    pub msg: Option<&'static str>,
    /// Key V's text (OfficeScreen 0x4F50A, from JA.EXE: data::VERSION_OFFICE).
    version: &'static str,
    pub sleep: bool,
    /// The shown palette (6-bit), rotated by `cycle`.
    pub pal: [u8; 768],
    /// The face buffers of team slots 0..7 (32 x 32 each).
    faces: Vec<u8>,
    /// The team ledger page while it is open (strategic.md "The team ledger").
    pub ledger: Option<Vec<u8>>,
    /// panel.md "Say it again (key L)" on the ledger page: the page under the box while the box shows.
    ledger_under: Option<Vec<u8>>,
    /// The slot list while it is open (strategic.md "Save Game and Restore Game") and the screen it draws on.
    pub slots: Option<SlotList>,
    slot_scr: Vec<u8>,
    /// The switch box while it is open (strategic.md "The switch box (Control Panel)").
    pub sbox: Option<SwitchBox>,
}

/// The 20 switch bytes (save offsets 21538..21557) of `w`.
pub fn switch_bytes(w: &World) -> [u8; 20] {
    let mut o = [0u8; 20];
    for (k, v) in o.iter_mut().enumerate() {
        *v = crate::ctlpanel::get(w, k);
    }
    o
}

pub fn set_switch_bytes(w: &mut World, o: &[u8; 20]) {
    for (k, &v) in o.iter().enumerate() {
        crate::ctlpanel::set(w, k, v);
    }
}

/// strategic.md "Highlight sounds": the main bank effects of hot spot k going on and off (99 = none).
const SOUND_ON: [i32; 8] = [99, 12, 9, 9, 8, 14, 10, 6];
const SOUND_OFF: [i32; 8] = [99, 13, 9, 9, 8, 15, 11, 7];

/// The highlight sound of hot spot k (on or off) joins the poll before its next frame.
fn sound(out: &mut Poll, k: usize, on: bool) {
    let fx = if on { SOUND_ON[k] } else { SOUND_OFF[k] };
    if fx != 99 {
        out.sounds.push((out.frames.len(), fx, if SOUND_ON[k] == 8 { 0x76 } else { 0x58 }));
    }
}

/// OfficeHighlight 0x4C0C8 with `on` = 1: save the rectangle under hot spot k, the sound, frames 0..3.
pub fn highlight_on(d: &OfficeData, base: &mut [u8], saved: &mut Vec<u8>, k: usize, out: &mut Poll) {
    let ((w, h), (x, y)) = (SIZE[k], AT[k]);
    saved.clear();
    for r in y..y + h {
        for c in x..x + w {
            saved.push(base.get(r as usize * PW + c as usize).copied().unwrap_or(0));
        }
    }
    out.highlights.push((k, true));
    sound(out, k, true);
    for j in 0..4 {
        frame(d, base, k, j);
        out.frames.push(base.to_vec());
    }
}

/// OfficeHighlight 0x4C0C8 with `on` = 0: the sound (hot spots 2, 3 and 6: after the last frame's wait), frames 3..0,
/// the saved rectangle back.
pub fn highlight_off(d: &OfficeData, base: &mut [u8], saved: &[u8], k: usize, out: &mut Poll) {
    out.highlights.push((k, false));
    let late = matches!(k, 2 | 3 | 6);
    if !late {
        sound(out, k, false);
    }
    for j in (0..4).rev() {
        frame(d, base, k, j);
        out.frames.push(base.to_vec());
    }
    if late {
        sound(out, k, false);
    }
    let ((w, h), (x, y)) = (SIZE[k], AT[k]);
    let mut i = 0;
    for r in y..y + h {
        for c in x..x + w {
            if let (Some(v), Some(d)) = (saved.get(i).copied(), base.get_mut(r as usize * PW + c as usize)) {
                *d = v;
            }
            i += 1;
        }
    }
}

/// The two clocks of the highlight's frame wait: timer 0x22A3FC (None = long ago) and timer 0x22A404 of the palette.
#[derive(Default)]
pub struct HighlightClock {
    pub timer: Option<i64>,
    pub cycled: i64,
}

/// One step of `play_frames`.
pub enum HighlightStep<'a> {
    Sfx { fx: i32, rate: i32, vol: i32, loops: i32 },
    /// A highlight frame goes to the screen.
    Show(&'a [u8]),
    /// One pass of the wait loop (MousePoll? 0x678E8).
    Tick,
    /// The palette rotated during the wait: the frame shows again with the new colours.
    Cycled(&'a [u8]),
}

/// The frames and sounds of a poll in time. OfficeHighlightFrame 0x4BFFC shows the frame, then waits until timer
/// 0x22A3FC reads 90 ms and restarts it. So the first frame after a pause has no wait of its own. OfficePaletteCycle
/// 0x4C26C runs in each pass of that wait.
pub fn play_frames(p: &Poll, pal: &mut [u8; 768], ck: &mut HighlightClock, now: &mut dyn FnMut() -> i64,
                   step: &mut dyn FnMut(HighlightStep, &[u8; 768])) {
    for i in 0..=p.frames.len() {
        for &(_, fx, vol) in p.sounds.iter().filter(|s| s.0 == i) {
            step(HighlightStep::Sfx { fx, rate: 8000, vol, loops: 1 }, pal);
        }
        let Some(f) = p.frames.get(i) else { break };
        step(HighlightStep::Show(f), pal);
        while ck.timer.is_some_and(|t| now() - t < 90) {
            step(HighlightStep::Tick, pal);
            let t = now();
            if cycle_palette(pal, t - ck.cycled) {
                ck.cycled = t;
                step(HighlightStep::Cycled(f), pal);
            }
        }
        ck.timer = Some(now());
    }
}

impl Office {
    pub fn new(iso: &str, exe: &str, w: &World) -> Result<Office, i32> {
        // music.md "Who plays what": office entry loads MENUS and plays theme 1.
        crate::music::set_exe(exe);
        crate::music::screen("MENUS", 1);
        crate::sound::screen_main(iso, exe);
        let (c, pd) = (crate::common(iso)?, panel::panel_data(iso, exe)?);
        let data = OfficeData::load(exe)?;
        let link = w.accounts.get(0xA3).copied().unwrap_or(0);
        let faces = crate::mapui::team_faces(iso, exe, w)?;
        let mut o = Office { data, pd, c, link, day: w.day, team: team_size(w), base: vec![0; PW * PH], saved: vec![],
                             h: 0, mouse: None, label_on: false, redraw: false, msg: None, version: crate::data::exe_text(exe, crate::data::VERSION_OFFICE), sleep: false, pal: [0; 768], faces, ledger: None, ledger_under: None,
                             slots: None, slot_scr: vec![], sbox: None };
        o.pal = o.draw_room(false);
        Ok(o)
    }

    fn draw_room(&mut self, dark: bool) -> [u8; 768] {
        let mut table0 = self.c.remap[0];
        let mut base = std::mem::take(&mut self.base);
        let pal = {
            let mut p = Painter { d: &self.pd, buf: &mut base, table0: &mut table0, table29: panel::table29(&self.c) };
            room(&self.data, &mut p, self.link, self.day, dark)
        };
        self.base = base;
        pal
    }

    /// The mouse position at the office entry or after a sub-screen: the next poll at the same point is no move
    /// (OfficeScreen captures the mouse at the top of each pass, before MousePoll).
    pub fn set_mouse(&mut self, x: i32, y: i32) {
        self.mouse = Some((x, y));
    }

    /// View Team (strategic.md "The team ledger"): the page with the team of `w`, the count of all accounts (the
    /// world keeps it), then the accounts; the page's palette is shown (no palette cycle while it is open).
    pub fn open_ledger(&mut self, w: &mut World) {
        let mut scr = vec![0u8; PW * PH];
        let mut base = std::mem::take(&mut self.base);
        let day = w.accounts.get(4..8).map(|b| i32::from_le_bytes([b[0], b[1], b[2], b[3]])).unwrap_or(0);
        let mut table0 = self.c.remap[0];
        let pal = {
            let mut p = Painter { d: &self.pd, buf: &mut scr, table0: &mut table0, table29: panel::table29(&self.c) };
            ledger_team(&self.data, &mut p, &mut base, &w.soldiers, &self.faces, day)
        };
        self.base = base;
        crate::mapui::count_accounts(w);
        {
            let mut p = Painter { d: &self.pd, buf: &mut scr, table0: &mut table0, table29: panel::table29(&self.c) };
            ledger_accounts(&mut p, &w.accounts, w.difficulty);
        }
        self.pal = pal;
        self.ledger = Some(scr);
    }

    /// panel.md "Say it again (key L)" on the open ledger page (DrawOfficeLedger 0x4CE6C): the last quote's box
    /// (PANEL_B.PCC) over the page until a key or a click (`ledger_again_end`). Returns the shown speaker and text,
    /// or None when the speaker byte names nobody (nothing shows).
    pub fn ledger_again(&mut self, iso: &str, exe: &str, w: &World) -> Result<Option<(u8, Vec<u8>)>, i32> {
        let Some(page) = self.ledger.as_ref() else { return Ok(None) };
        let Some((k, name, text)) = crate::balloon::again_text(iso, exe, w) else { return Ok(None) };
        let inv = crate::invdraw::inv_data(iso, exe)?;
        let mut scr = page.clone();
        let mut table0 = self.c.remap[0];
        let mut p = Painter { d: &self.pd, buf: &mut scr, table0: &mut table0, table29: panel::table29(&self.c) };
        crate::question::draw_face_text(&mut p, &inv, &text, k, &name, "PANEL_B.PCC");
        self.ledger_under = self.ledger.replace(scr);
        Ok(Some((k, text)))
    }

    /// The say-again box shows on the ledger page.
    pub fn ledger_again_shown(&self) -> bool {
        self.ledger_under.is_some()
    }

    /// The say-again box goes: the page under it comes back.
    pub fn ledger_again_end(&mut self) {
        if let Some(u) = self.ledger_under.take() {
            self.ledger = Some(u);
        }
    }

    /// Save Game (mode 1) or Restore Game (mode 0): the slot list opens over the screen as it is, with the slot
    /// headers of `dir` (none: no slot in use), the save name of `w` (save bytes 2..27) and its `day`.
    pub fn open_slots(&mut self, mode: i32, dir: Option<&Path>, w: &World) {
        let hdr = match dir {
            Some(d) => Headers::read(d),
            None => Headers { days: [0xFFFF; 10], ..Default::default() },
        };
        let mut name = [0u8; 26];
        if let Some(b) = w.base.get(2..28) {
            name.copy_from_slice(b);
        }
        self.slot_scr = self.picture();
        self.slots = Some(SlotList::new(mode, hdr, name, w.day));
    }

    /// One poll of the open slot list (buttons held: bit 0 left, bit 1 right) with the key bytes typed before it.
    /// "No saved games exist!" closes the list and opens the message box; the other outcomes leave the list for
    /// `close_slots`.
    pub fn slots_poll(&mut self, x: i32, y: i32, buttons: i32, keys: &[u8]) -> Option<Outcome> {
        let sl = self.slots.as_mut()?;
        for &k in keys {
            sl.key(k);
        }
        let mut table0 = self.c.remap[0];
        let mut p = Painter { d: &self.pd, buf: &mut self.slot_scr, table0: &mut table0, table29: panel::table29(&self.c) };
        let r = sl.poll(&mut p, &self.data.slab, x, y, buttons);
        if r == Some(Outcome::NoSaves) {
            self.slots = None;
            self.msg = saveslots::restore_steps(Outcome::NoSaves).0;
        }
        r
    }

    /// The list ended: the room is drawn again (the highlight state stays, as after the ledger).
    pub fn close_slots(&mut self) -> Option<SlotList> {
        let sl = self.slots.take();
        self.pal = self.draw_room(false);
        sl
    }

    /// Control Panel (strategic.md "The switch box (Control Panel)"): the box opens over the office picture; the
    /// steps to show and play.
    pub fn open_switchbox(&mut self, w: &mut World) -> Vec<Step> {
        let mut opts = switch_bytes(w);
        let mut steps = vec![];
        let mut scr = self.base.clone();
        let mut table0 = self.c.remap[0];
        let b = {
            let mut p = Painter { d: &self.pd, buf: &mut scr, table0: &mut table0, table29: panel::table29(&self.c) };
            SwitchBox::open(&self.data, &mut p, &mut opts, &mut steps)
        };
        set_switch_bytes(w, &opts);
        self.pal = self.data.room_pal;
        self.sbox = Some(b);
        steps
    }

    /// A left press on the open box (a sound and a music driver are installed); the toggled switches (k, new value)
    /// and true when the box closed (the office then shows the plain room; the next poll acts as a mouse move).
    pub fn switchbox_press(&mut self, w: &mut World, x: i32, y: i32, steps: &mut Vec<Step>) -> (Vec<(usize, u8)>, bool) {
        let Some(mut b) = self.sbox.take() else { return (vec![], false) };
        let mut opts = switch_bytes(w);
        let (wanted, playing) = crate::music::with(|m| (m.wanted(), m.playing()));
        let mut scr = vec![0u8; PW * PH];
        let mut table0 = self.c.remap[0];
        let r = {
            let mut p = Painter { d: &self.pd, buf: &mut scr, table0: &mut table0, table29: panel::table29(&self.c) };
            b.press(&self.data, &mut p, x, y, &mut opts, w.sound_setup.sound_driver, w.sound_setup.music_driver, wanted, playing, steps)
        };
        set_switch_bytes(w, &opts);
        if r.1 {
            self.pal = self.draw_room(false);
            b.close(&self.data, &self.base, opts[0] != 0, steps);
            self.mouse = None;
        } else {
            self.sbox = Some(b);
        }
        r
    }

    /// The ledger closes: the room (not dark) is drawn again with the palette of ROOM.PCX; the highlight state stays.
    pub fn close_ledger(&mut self) {
        self.ledger = None;
        self.ledger_under = None;
        self.pal = self.draw_room(false);
    }

    /// The picture on the screen: the base, the label at the mouse (after the first move), the message box; the
    /// ledger page while it is open.
    pub fn picture(&self) -> Vec<u8> {
        if let Some(l) = &self.ledger {
            return l.clone();
        }
        if self.slots.is_some() {
            return self.slot_scr.clone();
        }
        if let Some(b) = &self.sbox {
            return b.pic.clone();
        }
        let mut img = self.base.clone();
        let mut table0 = self.c.remap[0];
        let mut p = Painter { d: &self.pd, buf: &mut img, table0: &mut table0, table29: panel::table29(&self.c) };
        if let Some((s, x, ty)) = self.label_now() {
            p.text(s.as_bytes(), x, ty, false, 0xFE, 1, 1);
        }
        if let Some(m) = self.msg {
            crate::msgbox::draw(&mut p, m.as_bytes(), &self.c.remap[MSG_TABLE], 0xFF);
        }
        img
    }

    /// The label picture and place after the last poll (OfficeScreen draws it at every mouse move and keeps it on
    /// the screen until the room is drawn again): None while the room is dark, a box shows or no move happened.
    pub fn label_now(&self) -> Option<(&'static str, i32, i32)> {
        if self.sleep || self.msg.is_some() {
            return None;
        }
        self.label_drawn()
    }

    /// The label the poll drew before a box opened or Sleep went dark: OfficeScreen draws it in its moved branch
    /// (also on the redraw pass after a highlight change) and only then runs the click switch.
    pub fn label_drawn(&self) -> Option<(&'static str, i32, i32)> {
        if !self.label_on {
            return None;
        }
        let (x, y) = self.mouse?;
        let x = if x > 0x128 { 0x127 } else { x };
        Some((label(hot(x, y), self.link, self.h), x, y - 10))
    }

    /// The office picture with the message box of `s` over it and no hover label (strategic.md "After the list":
    /// "Saving..." / "Restoring game..." while the slot file is written or read).
    pub fn busy_picture(&self, s: &str) -> Vec<u8> {
        let mut img = self.base.clone();
        let mut table0 = self.c.remap[0];
        let mut p = Painter { d: &self.pd, buf: &mut img, table0: &mut table0, table29: panel::table29(&self.c) };
        crate::msgbox::draw(&mut p, s.as_bytes(), &self.c.remap[MSG_TABLE], 0xFF);
        img
    }

    /// 0x4C26C: see `cycle_palette`.
    pub fn cycle(&mut self, elapsed_ms: i64) -> bool {
        cycle_palette(&mut self.pal, elapsed_ms)
    }

    /// One poll: the mouse at (x, y), `press` = a left press, `key_v` = the key V.
    pub fn poll(&mut self, x: i32, y: i32, press: bool, key_v: bool) -> Poll {
        let mut out = Poll::default();
        if self.sleep {
            return out;
        }
        if self.msg.is_some() {
            // A press or a key closes the box and is used up; the label is drawn again.
            if press || key_v {
                self.msg = None;
                self.mouse = Some((x, y));
            }
            return out;
        }
        if self.mouse != Some((x, y)) || self.redraw {
            self.redraw = false;
            self.mouse = Some((x, y));
            self.label_on = true;
            let k = hot(x, y);
            if self.h != 0 && self.h != k && !(self.h == 2 && k == 3 && self.link != 0) {
                highlight_off(&self.data, &mut self.base, &self.saved, self.h, &mut out);
                self.h = 0;
            }
            if self.h == 0 && (1..=7).contains(&k) && k != 3 && (k != 2 || self.link != 0) {
                highlight_on(&self.data, &mut self.base, &mut self.saved, k, &mut out);
                self.h = k;
            }
        }
        if press {
            let k = hot(x, y);
            out.click = Some(k);
            if k == SLEEP {
                if self.team == 0 {
                    self.msg = Some(SLEEP_MSG);
                    self.redraw = true;
                    out.msg = true;
                } else {
                    self.pal = self.draw_room(true);
                    self.h = 0;
                    self.sleep = true;
                    out.sleep = true;
                }
            }
            if k == VIEW_TEAM {
                out.ledger = true;
            }
            if k == 0 || k == 1 {
                out.slots = Some(if k == 0 { saveslots::SAVE } else { saveslots::RESTORE });
            }
            if k == LEAVE {
                out.leave = true;
            }
            if (k == 2 || (k == 3 && self.h == 2)) && self.link != 0 {
                out.aim = true;
                out.aim_fade_first = k == 3;
            }
            if k == CONTROL_PANEL {
                out.switchbox = true;
            }
        } else if key_v {
            self.msg = Some(self.version);
            out.msg = true;
        }
        out
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Ledger row d057aca:ja1front:14063: with no save folder the Save Game list has every slot empty (day 0xFFFF,
    /// as Headers::read gives a missing slot file), so the list shows "Empty" rows.
    #[test]
    fn no_save_folder_gives_empty_save_slots() {
        let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
        let (iso, exe) = (root.join("rom/JA1.gog"), root.join("build/game113b/JA.EXE"));
        if !iso.exists() || !exe.exists() {
            return;
        }
        let (iso, exe) = (iso.to_str().unwrap(), exe.to_str().unwrap());
        let w = World::new_game(iso, exe, 0, 0).unwrap();
        let mut o = Office::new(iso, exe, &w).unwrap();
        o.open_slots(1, None, &w);
        let sl = o.slots.as_ref().unwrap();
        assert_eq!((sl.hdr.days, sl.hdr.used), ([0xFFFF; 10], [false; 10]));
    }

    /// OfficeScreen draws the hover label at a mouse move and keeps it until the room is drawn again: the dark room
    /// of Sleep and a message box show no label (`label_drawn` still holds the one drawn before).
    #[test]
    fn the_dark_room_and_a_box_show_no_label() {
        let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
        let (iso, exe) = (root.join("rom/JA1.gog"), root.join("build/game113b/JA.EXE"));
        if !iso.exists() || !exe.exists() {
            return;
        }
        let (iso, exe) = (iso.to_str().unwrap(), exe.to_str().unwrap());
        let w = World::new_game(iso, exe, 0, 0).unwrap();
        let mut o = Office::new(iso, exe, &w).unwrap();
        (o.label_on, o.mouse) = (true, Some((100, 100)));
        assert!(o.label_now().is_some() && o.label_now() == o.label_drawn());
        o.sleep = true;
        assert!(o.label_now().is_none() && o.label_drawn().is_some());
        (o.sleep, o.msg) = (false, Some("x"));
        assert!(o.label_now().is_none() && o.label_drawn().is_some());
    }

    /// put clips at the buffer edges: a picture at x = 318 keeps its third column off the screen (no wrap into the
    /// next row).
    #[test]
    fn put_clips_at_the_right_edge() {
        let mut buf = vec![0u8; PW * PH];
        let pic: Pic = (3, 2, vec![5; 6]);
        put(&mut buf, &pic, 318, 0);
        assert_eq!(&buf[318..320], &[5, 5]);
        assert_eq!(&buf[320 + 318..320 + 320], &[5, 5]);
        assert_eq!(buf.iter().filter(|&&b| b == 5).count(), 4);
    }

    /// A hot-spot frame reads only the brush picture: the part past its right edge is not drawn.
    #[test]
    fn a_frame_stops_at_the_brush_edge() {
        let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
        let exe = root.join("build/game113b/JA.EXE");
        let Ok(mut d) = OfficeData::load(exe.to_str().unwrap()) else { return };
        let (k, j) = (1, 0);
        let (sx, sy) = FRAMES[k][j];
        let (bw, bh) = (sx as usize + 2, sy as usize + 4);
        d.brush = (bw, bh, vec![7; bw * bh + 64]);
        let mut buf = vec![0u8; PW * PH];
        frame(&d, &mut buf, k, j);
        let (x, y) = AT[k];
        let drawn: Vec<usize> = (0..PW * PH).filter(|&i| buf[i] == 7).collect();
        let want: Vec<usize> = (0..4).flat_map(|r| (0..2).map(move |c| (y + r) as usize * PW + (x + c) as usize)).collect();
        assert_eq!(drawn, want);
    }

    /// Ledger row c2aea50:ja1front:14059: the office palette cycle (0x4C26C) turns only after 80 ms on its timer.
    #[test]
    fn the_office_palette_turns_after_80_ms() {
        let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
        let (iso, exe) = (root.join("rom/JA1.gog"), root.join("build/game113b/JA.EXE"));
        if !iso.exists() || !exe.exists() {
            return;
        }
        let (iso, exe) = (iso.to_str().unwrap(), exe.to_str().unwrap());
        let w = World::new_game(iso, exe, 0, 0).unwrap();
        let mut o = Office::new(iso, exe, &w).unwrap();
        let before = o.pal;
        assert!(!o.cycle(80));
        assert_eq!(o.pal, before);
        assert!(o.cycle(81));
        assert_ne!(o.pal, before);
    }

    #[test]
    fn fade_levels() {
        // ending.md op 24 / 25: n + 1 levels from the full colours to black (or back), truncated colour × l / n.
        let pal = [63u8; 768];
        let out = fade(&pal, 24, 256, 0);
        assert_eq!(out.len(), 25);
        assert_eq!(out[0], pal);
        assert_eq!(out[24], [0u8; 768]);
        assert_eq!(out[23][0], 63 / 24);
        let back = fade(&pal, 24, 0, 256);
        assert_eq!(back.len(), 25);
        assert_eq!(back[1][0], 2);
        assert_eq!(back[24], pal);
        // A part fade: n · from / 256 truncated.
        assert_eq!(fade(&pal, 24, 128, 0).len(), 13);
    }
}

/// 0x4C26C: more than 80 ms on the timer: palette entries 0xE1..0xEF rotate by one and the caller restarts the timer
/// (true).
pub fn cycle_palette(pal: &mut [u8; 768], elapsed_ms: i64) -> bool {
    if elapsed_ms <= 80 {
        return false;
    }
    crate::eod::rotate(pal, 0xE1, 0xEF);
    true
}
