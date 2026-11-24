//! The merc panel (docs/spec/panel.md): loading the panel sheet, fonts and faces, and drawing one slot.
use crate::data::{Common, Tileset};
use crate::iso::Iso;
use crate::soldier::REC;
use std::collections::HashMap;
use std::io;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex, OnceLock};

pub const PW: i32 = 320;
pub const PH: i32 = 200;
/// The stat popup picture (panel.md "Stat popup").
pub const POP_W: i32 = 100;
pub const POP_H: i32 = 42;
const POP_X: i32 = 173;
const POP_Y: i32 = 18;
const SHEET_H: usize = 160;

fn bad(msg: String) -> io::Error {
    io::Error::new(io::ErrorKind::InvalidData, msg)
}

// ---------------------------------------------------------------------------------------------------------------
// Files

/// A member of a Sir-tech archive (speech.md "Archives"), by name (case-insensitive); the original's ArcSeek
/// 0x534A4 (directory at +0x14, records of 0x20).
pub fn archive_member(arc: &[u8], name: &str) -> io::Result<Vec<u8>> {
    let (off, size) =
        crate::sab::find_member(arc, name).ok_or_else(|| io::Error::new(io::ErrorKind::NotFound, name.to_string()))?;
    arc.get(off..off + size).map(|s| s.to_vec()).ok_or_else(|| bad(format!("member {} past the end", name)))
}

/// An 8-bit PCX image: (width, height, pixels row-major at pitch = width).
// SPEC GAP: the spec names the files as PCX but does not describe the format; this is the common ZSoft PCX
// (128-byte header, RLE: a byte >= 0xC0 gives count = byte & 0x3F of the next byte), one plane of 8 bits.
pub fn pcx(data: &[u8]) -> io::Result<(usize, usize, Vec<u8>)> {
    if data.len() < 128 {
        return Err(bad("PCX header truncated".into()));
    }
    let w16 = |a: usize| u16::from_le_bytes([data[a], data[a + 1]]) as usize;
    let (x0, y0, x1, y1) = (w16(4), w16(6), w16(8), w16(10));
    if x1 < x0 || y1 < y0 {
        return Err(bad("PCX bad size".into()));
    }
    let (w, h) = (x1 - x0 + 1, y1 - y0 + 1);
    let planes = data[65].max(1) as usize;
    let bpl = w16(66).max(w);
    let total = bpl * planes * h;
    let mut raw = Vec::with_capacity(total);
    let mut p = 128;
    while raw.len() < total && p < data.len() {
        let b = data[p];
        p += 1;
        if b >= 0xC0 {
            let v = *data.get(p).ok_or_else(|| bad("PCX data truncated".into()))?;
            p += 1;
            for _ in 0..(b & 0x3F) {
                raw.push(v);
            }
        } else {
            raw.push(b);
        }
    }
    if raw.len() < total {
        return Err(bad("PCX data short".into()));
    }
    let mut out = vec![0u8; w * h];
    for y in 0..h {
        let s = y * bpl * planes;
        out[y * w..(y + 1) * w].copy_from_slice(&raw[s..s + w]);
    }
    Ok((w, h, out))
}

fn game_dir(exe_path: &str) -> PathBuf {
    Path::new(exe_path).parent().map(|p| p.to_path_buf()).unwrap_or_default()
}

/// One font: raw sheet, pitch, glyph height and the 131 widths.
pub struct Font {
    pub sheet: Vec<u8>,
    pub pitch: usize,
    pub height: usize,
    pub widths: &'static [u8; 131],
}

/// A byte at `off` of font sheet `font`. The original holds the three files one after the other (0x22570C), and a
/// glyph at x < 0 with w enlarged reads past its sheet into the next one. Past the third file the original reads the
/// pointers 0x229790 (only with x below about -470, longer than any text); that gives 0 here.
fn font_byte(fonts: &[Font; 3], font: usize, off: i64) -> u8 {
    let mut off = off.max(0) as usize;
    for f in &fonts[font..] {
        match f.sheet.get(off) {
            Some(&b) => return b,
            None => off -= f.sheet.len(),
        }
    }
    0
}

/// The bytes that TextWidth 0x35880 reads as a width for byte 0x00..0x1F in font 0: every third byte below the char
/// table (0x944E0.., code bytes of the data block and the name "JA_big.fnt").
const WIDTH_BELOW: [u8; 32] = [
    52, 255, 14, 5, 52, 255, 15, 25, 52, 255, 16, 5, 52, 255, 17, 30, 52, 255, 74, 115, 108, 102, 0, 95, 69, 46, 116, 65,
    105, 102, 0, 1,
];
/// The same for byte 0xA3..0xFF in font 2: every third byte above the char table (0x949D7.., the font sizes and the
/// box tables after them).
const WIDTH_ABOVE: [u8; 93] = [
    1, 147, 0, 11, 0, 0, 0, 11, 0, 108, 0, 89, 0, 108, 0, 28, 0, 1, 0, 37, 1, 1, 0, 94, 1, 108, 0, 115, 0, 29, 0, 97, 0,
    59, 0, 124, 0, 59, 0, 63, 0, 208, 0, 72, 0, 208, 0, 129, 0, 59, 0, 150, 0, 203, 0, 8, 0, 200, 0, 52, 1, 1, 0, 56, 1,
    1, 0, 113, 0, 159, 0, 0, 115, 115, 0, 135, 2, 91, 0, 0, 6, 91, 0, 0, 4, 66, 0, 0, 11, 66, 0, 0, 7,
];

/// The width of byte `b` in TextWidth 0x35880. The function does not test the range: a byte outside 0x20..0xA2 reads
/// the table of the font before or after, or the data around the three tables. The byte is signed.
/// The width of the text `s` in font `font` (the sum of TextWidth 0x35880 over its bytes).
pub fn text_width(fonts: &[Font; 3], font: usize, s: &[u8]) -> i32 {
    s.iter().map(|&b| text_width_of(fonts, font, b)).sum()
}

fn text_width_of(fonts: &[Font; 3], font: usize, b: u8) -> i32 {
    let i = font as i32 * 131 + b as i32 - 0x20;
    let w = match i {
        i if i < 0 => WIDTH_BELOW[(i + 32) as usize],
        i if i >= 393 => WIDTH_ABOVE[(i - 393) as usize],
        i => fonts[i as usize / 131].widths[i as usize % 131],
    };
    w as i8 as i32
}

const W0: [u8; 131] = {
    let mut w = [4u8; 131];
    let mut i = 91;
    while i < 111 {
        w[i] = 0;
        i += 1;
    }
    w
};
const W1: [u8; 131] = [
    5, 6, 6, 6, 6, 6, 7, 5, 4, 4, 6, 6, 6, 5, 5, 6, 6, 6, 6, 6, 6, 6, 6, 6, 6, 6, 5, 6, 6, 6, 6, 6, 6, //
    6, 6, 6, 6, 6, 6, 6, 6, 6, 6, 6, 6, 6, 6, 6, 6, 6, 6, 6, 6, 6, 6, 6, 6, 6, 6, 6, 6, 6, 6, 6, 6, 6, //
    6, 6, 6, 6, 6, 6, 6, 6, 6, 6, 6, 6, 6, 6, 6, 6, 6, 6, 6, 6, 6, 6, 6, 6, 6, 0, 0, 0, 0, 0, 0, 0, 0, //
    0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 6, 6, 6, 6, 6, 6, 6, 6, 6, 6, 6, 6, 6, 6, 6, 6, 6, 6, 6, 6,
];
const W2: [u8; 131] = [
    3, 3, 5, 8, 6, 7, 6, 3, 4, 4, 7, 6, 3, 5, 3, 6, 6, 6, 6, 6, 6, 6, 6, 6, 6, 6, 3, 3, 5, 6, 5, 6, 6, //
    6, 6, 6, 6, 5, 5, 6, 6, 3, 6, 6, 5, 8, 6, 6, 6, 6, 6, 6, 7, 6, 6, 9, 6, 7, 6, 9, 6, 5, 6, 6, 3, 6, //
    6, 6, 6, 6, 5, 6, 6, 3, 5, 6, 3, 9, 6, 6, 6, 6, 5, 6, 5, 6, 6, 9, 6, 6, 5, 9, 7, 6, 6, 7, 7, 9, 8, //
    7, 9, 10, 6, 9, 8, 7, 10, 7, 7, 9, 6, 6, 6, 6, 6, 6, 6, 7, 6, 6, 6, 6, 4, 6, 6, 6, 6, 6, 4, 6, 5,
];

impl Font {
    pub fn x_of(&self, c: usize) -> usize {
        self.widths[..c].iter().map(|&w| w as usize).sum()
    }
}

/// Everything the panel loads from the hard disk and the CD (cached per (iso, exe)).
pub struct PanelData {
    /// PANELS.PCC, 320 x 160, pitch 320.
    pub sheet: Vec<u8>,
    pub fonts: [Font; 3],
    /// FACES.PCX, 320 x 200.
    pub faces: Vec<u8>,
    /// DEADFACE.PCC, 28 x 25.
    pub deadface: Vec<u8>,
    /// POPINV.PCC, 100 x 42 (the stat popup picture).
    pub popinv: Vec<u8>,
    /// CD DAT/JAGGED.DAT (the eye rows are its first bytes).
    pub jagged: Vec<u8>,
}

impl PanelData {
    pub fn load(iso: &mut Iso, exe_path: &str) -> io::Result<PanelData> {
        let dir = game_dir(exe_path);
        let main = std::fs::read(dir.join("DAT").join("MAIN.DAT"))?;
        let (w, h, sheet) = pcx(&archive_member(&main, "PANELS.PCC")?)?;
        if w != 320 || h < SHEET_H {
            return Err(bad(format!("PANELS.PCC is {} x {}", w, h)));
        }
        let sheet = sheet[..320 * SHEET_H].to_vec();
        let (w, h, faces) = pcx(&archive_member(&main, "FACES.PCX")?)?;
        if w != 320 || h < 200 {
            return Err(bad(format!("FACES.PCX is {} x {}", w, h)));
        }
        let (w, h, dead) = pcx(&archive_member(&main, "DEADFACE.PCC")?)?;
        if w < 28 || h < 25 {
            return Err(bad(format!("DEADFACE.PCC is {} x {}", w, h)));
        }
        let mut deadface = vec![0u8; 28 * 25];
        for y in 0..25 {
            deadface[y * 28..y * 28 + 28].copy_from_slice(&dead[y * w..y * w + 28]);
        }
        let (w, h, pop) = pcx(&archive_member(&main, "POPINV.PCC")?)?;
        if w < POP_W as usize || h < POP_H as usize {
            return Err(bad(format!("POPINV.PCC is {} x {}", w, h)));
        }
        let mut popinv = vec![0u8; (POP_W * POP_H) as usize];
        for y in 0..POP_H as usize {
            popinv[y * POP_W as usize..(y + 1) * POP_W as usize].copy_from_slice(&pop[y * w..y * w + POP_W as usize]);
        }
        let font = |name: &str, pitch: usize, height: usize, widths: &'static [u8; 131]| -> io::Result<Font> {
            let sheet = std::fs::read(dir.join(name))?;
            if sheet.len() < pitch * height {
                return Err(bad(format!("{} is {} bytes", name, sheet.len())));
            }
            Ok(Font { sheet, pitch, height, widths })
        };
        let fonts = [font("JA_SMALL.FNT", 442, 7, &W0)?, font("JA_M_ENG.FNT", 659, 6, &W1)?, font("JA_BIG.FNT", 789, 11, &W2)?];
        let jagged = iso.read("DAT/JAGGED.DAT")?;
        Ok(PanelData { sheet, fonts, faces: faces[..320 * 200].to_vec(), deadface, popinv, jagged })
    }
}

fn panel_cache() -> &'static Mutex<HashMap<(String, String), Arc<PanelData>>> {
    static C: OnceLock<Mutex<HashMap<(String, String), Arc<PanelData>>>> = OnceLock::new();
    C.get_or_init(|| Mutex::new(HashMap::new()))
}

pub fn panel_data(iso_path: &str, exe_path: &str) -> Result<Arc<PanelData>, i32> {
    let key = (iso_path.to_string(), exe_path.to_string());
    if let Some(d) = panel_cache().lock().unwrap_or_else(|e| e.into_inner()).get(&key) {
        return Ok(d.clone());
    }
    let mut iso = Iso::open(iso_path).map_err(|_| crate::E_FILE)?;
    let d = Arc::new(PanelData::load(&mut iso, exe_path).map_err(|_| crate::E_FILE)?);
    panel_cache().lock().unwrap_or_else(|e| e.into_inner()).insert(key, d.clone());
    Ok(d)
}

/// The two frame pictures of a face buffer: `5` no light, `0xC` the selected merc (table 0x93732).
pub const LIGHT_OFF: u8 = 5;
pub const LIGHT_ON: u8 = 0xC;

thread_local! {
    /// The frame changes since the last `take_face_lights` (0 = no change). The library's selection code runs in
    /// host callbacks, and the face buffers belong to `Screen`.
    static FACE_LIGHTS: std::cell::Cell<[u8; 8]> = const { std::cell::Cell::new([0; 8]) };
}

/// Note that slot k gets frame picture `e` (callers of 0x46A4C in the host).
pub fn note_face_light(k: usize, e: u8) {
    if k < 8 {
        FACE_LIGHTS.with(|c| {
            let mut v = c.get();
            v[k] = e;
            c.set(v);
        });
    }
}

/// The frame changes since the last call, per slot (0 = no change).
pub fn take_face_lights() -> [u8; 8] {
    FACE_LIGHTS.with(|c| c.replace([0; 8]))
}

/// 0x46A4C: the 32 x 28 sheet area of frame picture `e` over a face buffer. BlitRemap 0x522E4 with table 0 and key 0,
/// and table byte 0x1B = 0 during the call (colour 0x1B in the selected frame's middle leaves the face).
pub fn face_light(d: &PanelData, table0: &[u8; 256], face: &mut [u8], e: u8) {
    let (sx, sy) = if e == LIGHT_ON { (0xF0, 0) } else { (0xC0, 0) };
    let mut t = *table0;
    t[0x1B] = 0;
    for r in 0..28usize {
        for c in 0..32usize {
            let v = t[d.sheet[(sy + r) * 320 + sx + c] as usize];
            if v != 0 {
                face[r * 32 + c] = v;
            }
        }
    }
}

/// LoadMercFace 0x286C4: fill a face buffer (32 x 32) for face number n: kind 0 alive (returns the eye row), 1 dead,
/// 2 after the death morph (frame 9). Only the bytes the loading writes change.
pub fn load_face(iso_path: &str, d: &PanelData, n: i32, kind: i32, out: &mut [u8]) -> Result<Option<u8>, i32> {
    if n < 0 || out.len() < 1024 {
        return Err(crate::E_ARG);
    }
    let n = n as usize;
    match kind {
        0 => {
            let (cx, cy) = (28 * (n % 11), 29 * (n / 11));
            if cy + 29 > 200 {
                return Err(crate::E_ARG);
            }
            for r in 0..29 {
                let src = &d.faces[(cy + r) * 320 + cx..(cy + r) * 320 + cx + 28];
                let at = if r < 25 { (2 + r) * 32 + 2 } else { (28 + r - 25) * 32 };
                out[at..at + 28].copy_from_slice(src);
            }
            let eye = *d.jagged.get(n).ok_or(crate::E_FILE)?;
            Ok(Some(eye))
        }
        1 => {
            for r in 0..25 {
                out[(2 + r) * 32 + 2..(2 + r) * 32 + 30].copy_from_slice(&d.deadface[r * 28..r * 28 + 28]);
            }
            Ok(None)
        }
        2 => {
            // The 10 frames play in order into (2, 2); only frame 9 stays.
            load_morph(iso_path, n as i32, 9, out)?;
            Ok(None)
        }
        _ => Err(crate::E_ARG),
    }
}

/// Frame i (0..9) of face n's death morph `MORF<n>.PCC` (CD DAT/MF1.DAT for n < 30, else DAT/MF2.DAT; 10 frames of
/// 28 x 25 side by side) into a face buffer at (2, 2).
pub fn load_morph(iso_path: &str, n: i32, i: usize, out: &mut [u8]) -> Result<(), i32> {
    if n < 0 || i > 9 || out.len() < 1024 {
        return Err(crate::E_ARG);
    }
    let mut iso = Iso::open(iso_path).map_err(|_| crate::E_FILE)?;
    let arc = iso.read(if n < 30 { "DAT/MF1.DAT" } else { "DAT/MF2.DAT" }).map_err(|_| crate::E_FILE)?;
    let m = archive_member(&arc, &format!("MORF{}.PCC", n)).map_err(|_| crate::E_FILE)?;
    let (w, h, px) = pcx(&m).map_err(|_| crate::E_FILE)?;
    if w < 280 || h < 25 {
        return Err(crate::E_FILE);
    }
    for r in 0..25 {
        let s = r * w + 28 * i;
        out[(2 + r) * 32 + 2..(2 + r) * 32 + 30].copy_from_slice(&px[s..s + 28]);
    }
    Ok(())
}

// ---------------------------------------------------------------------------------------------------------------
// Drawing

/// Elements of the panel sheet: (x, y, w, h).
pub(crate) fn element(e: usize) -> (i32, i32, i32, i32) {
    match e {
        0 => (0, 0, 48, 42),
        7 => (0, 42, 48, 42),
        // panel.md "The switch view": open / close frames and the switch faces
        1 => (48, 0, 48, 42),
        2 => (96, 0, 48, 42),
        3 => (144, 0, 48, 42),
        8 => (48, 42, 48, 42),
        9 => (96, 42, 48, 42),
        10 => (144, 42, 48, 42),
        0x45 => (155, 84, 9, 8),
        0x46 => (164, 84, 9, 8),
        0x47 => (173, 84, 9, 8),
        0x48 => (182, 84, 9, 8),
        0x49 => (172, 110, 9, 8),
        0x4A => (181, 110, 9, 8),
        0x4B => (190, 110, 9, 8),
        0x4C => (199, 110, 9, 8),
        4 => (229, 0, 11, 27),
        11 => (209, 29, 11, 27),
        17 => (242, 2, 28, 25),
        // 0x21198: the name strip under a face on the A.I.M. screen
        0x18 => (272, 34, 28, 6),
        80 => (105, 117, 3, 1),
        81 => (105, 116, 3, 1),
        82 => (108, 117, 3, 1),
        83 => (108, 116, 3, 1),
        84 => (111, 117, 3, 1),
        85 => (111, 116, 3, 1),
        86 => (155, 95, 3, 1),
        87 => (155, 96, 3, 1),
        88 => (230, 1, 3, 25),
        89 => (233, 1, 3, 25),
        90 => (236, 1, 3, 25),
        91 => (210, 30, 3, 25),
        92 => (213, 30, 3, 25),
        93 => (216, 30, 3, 25),
        104 => (242, 2, 24, 22),
        0x12 => (192, 77, 38, 16),
        0x3C => (272, 83, 48, 49),
        0x28 => (115, 98, 57, 20),
        0x29 => (161, 118, 57, 20),
        0x2A => (104, 118, 57, 21),
        0x3B => (0, 84, 42, 62),
        0x2D => (240, 49, 20, 16),
        0x38 => (219, 65, 13, 9),
        0x39 => (232, 65, 12, 8),
        0x2C => (220, 49, 20, 16),
        0x19 => (192, 67, 14, 10),
        0x1A => (206, 67, 12, 10),
        0x1B => (192, 56, 10, 11),
        0x1C => (202, 56, 9, 11),
        0x35 => (91, 119, 13, 13),
        0x32 => (272, 0, 44, 17),
        0x34 => (78, 119, 13, 13),
        0x36 => (78, 132, 13, 7),
        0x37 => (91, 132, 13, 7),
        0x41 => (78, 107, 37, 9),
        0x13 => (230, 77, 38, 16),
        0x16 => (268, 65, 25, 18),
        0x17 => (293, 65, 25, 18),
        0x75 => (300, 133, 16, 16),
        0x78 => (198, 147, 37, 12),
        0x79 => (235, 147, 37, 12),
        34 => (42, 114, 36, 10),
        35 => (42, 104, 36, 10),
        36 => (42, 94, 36, 10),
        39 => (42, 84, 36, 10),
        43 => (42, 134, 36, 10),
        37 => (162, 140, 36, 10),
        38 => (42, 124, 36, 10),
        // strategic.md "The day-end screen": the OK buttons' faces (element table 0x93732)
        0x65 => (52, 144, 26, 16),
        0x66 => (134, 139, 26, 16),
        20 => (260, 49, 26, 16),
        21 => (286, 49, 26, 16),
        58 => (245, 65, 16, 9),
        114 => (238, 131, 14, 7),
        115 => (238, 138, 14, 7),
        0x2E => (172, 93, 47, 17),
        0x2F => (276, 17, 43, 17),
        0x30 => (221, 93, 51, 18),
        0x31 => (220, 29, 52, 19),
        0x43 => (220, 111, 52, 20),
        0x42 => (312, 49, 5, 13),
        0x69 => (272, 17, 47, 17),
        103 => (272, 132, 28, 25),
        95 => (131, 87, 20, 8),
        99 => (78, 149, 20, 8),
        // strategic.md "Salary raise": the dollar field of the salary box
        0x7B => (240, 77, 28, 16),
        100 => (98, 149, 20, 8),
        // panel.md "The mouse pointer"
        61 => (218, 131, 18, 16),
        62 => (38, 146, 11, 11),
        63 => (192, 29, 12, 15),
        78 => (27, 146, 11, 11),
        97 => (0, 146, 11, 11),
        98 => (11, 146, 11, 11),
        _ => unreachable!("element {}", e),
    }
}

/// Side offsets (face, pair 1, health, energy, AP) per column.
const SIDE: [[(i32, i32); 5]; 2] = [
    [(14, 3), (16, 5), (3, 4), (7, 5), (10, 5)],
    [(3, 3), (5, 5), (35, 4), (39, 5), (42, 5)],
];

/// The offsets of a slot's parts for column byte +0x180: [0] the face, [2] the bar area.
pub fn slot_side(column: u8) -> &'static [(i32, i32); 5] {
    &SIDE[if column == 0 { 0 } else { 1 }]
}
const DYING_TINT: [u8; 4] = [112, 118, 117, 116];
const ASSIGN: [&[u8]; 8] = [b"ON DUTY", b"REST", b"TRAIN", b"DOCTOR", b"PATIENT", b"REPAIR", b"FIRE", b"ESCORT"];

pub struct Painter<'a> {
    pub d: &'a PanelData,
    pub buf: &'a mut [u8],
    pub table0: &'a mut [u8; 256],
    pub table29: &'a [u8; 256],
}

impl<'a> Painter<'a> {
    #[inline]
    pub(crate) fn put(&mut self, x: i32, y: i32, v: u8) {
        // SPEC GAP: the spec does not clip; a pixel outside the 320 x 200 buffer is dropped here.
        let off = y as i64 * PW as i64 + x as i64;
        if off >= 0 && (off as usize) < self.buf.len() {
            self.buf[off as usize] = v;
        }
    }
    #[inline]
    pub(crate) fn get(&self, x: i32, y: i32) -> Option<u8> {
        let off = y as i64 * PW as i64 + x as i64;
        if off >= 0 && (off as usize) < self.buf.len() {
            Some(self.buf[off as usize])
        } else {
            None
        }
    }
    #[inline]
    pub(crate) fn sheet(&self, x: i32, y: i32) -> u8 {
        // SPEC GAP: reads past the 320 x 160 sheet are not described (an AP bar with apLeft far below 0 has an empty
        // part of up to 153 rows and reaches past row 159); they give 0 here, the original reads other bytes.
        let off = y as i64 * 320 + x as i64;
        if off >= 0 && (off as usize) < self.d.sheet.len() {
            self.d.sheet[off as usize]
        } else {
            0
        }
    }

    /// Element e at (x, y), optional row count; sheet pixel 0 is not written (DrawPanelElem 0x469F0; with a row count
    /// DrawPanelElemRows 0x46990).
    pub fn element(&mut self, e: usize, x: i32, y: i32, rows: Option<i32>) {
        let (sx, sy, w, h) = element(e);
        for r in 0..rows.unwrap_or(h) {
            for c in 0..w {
                let p = self.sheet(sx + c, sy + r);
                if p != 0 {
                    self.put(x + c, y + r, p);
                }
            }
        }
    }

    /// The 3 pixels at the element's sheet position to rows y .. y + n - 1 (all written): PanelBarRows? 0x1F660.
    pub fn bar_rows(&mut self, e: usize, x: i32, y: i32, n: i32) {
        let (sx, sy, _, _) = element(e);
        for r in 0..n {
            for c in 0..3 {
                let p = self.sheet(sx + c, sy);
                self.put(x + c, y + r, p);
            }
        }
    }

    /// Element e remapped with table 0 (BlitRemap 0x522E4, key 0).
    pub(crate) fn remapped0(&mut self, e: usize, x: i32, y: i32) {
        let (sx, sy, w, h) = element(e);
        for r in 0..h {
            for c in 0..w {
                let v = self.table0[self.sheet(sx + c, sy + r) as usize];
                if v != 0 {
                    self.put(x + c, y + r, v);
                }
            }
        }
    }

    pub fn text(&mut self, s: &[u8], x: i32, y: i32, centred: bool, fg: u8, shadow: u8, font: usize) {
        self.text_mode(s, x, y, centred, fg, shadow, font, 300);
    }

    /// Text with mode `mode`: 300 = normal; else "Text with a background" (glyph pixel 0 -> colour `mode`). The glyphs go
    /// through BlitRemapKey 0x5232D (DrawText 0x358D0).
    #[allow(clippy::too_many_arguments)]
    pub fn text_mode(&mut self, s: &[u8], mut x: i32, mut y: i32, centred: bool, fg: u8, shadow: u8, font: usize,
                     mode: i32) {
        let f = &self.d.fonts[font];
        if centred {
            let sum: i32 = s
                .iter()
                .map(|&b| text_width_of(&self.d.fonts, font, b))
                .sum();
            x -= sum / 2;
        }
        if font == 0 {
            y -= 1;
        }
        let (s_ff, s_fe) = (self.table0[0xFF], self.table0[0xFE]);
        self.table0[0xFF] = fg;
        self.table0[0xFE] = shadow;
        for &b in s {
            let c = b as i32 - 0x20;
            if !(0..131).contains(&c) {
                return; // the two table bytes stay changed
            }
            let mut w = f.widths[c as usize] as i32;
            if (x as u32).wrapping_add(w as u32) > 320 {
                w = 320 - x;
            }
            if w == 0 {
                // A copy of 0 columns runs 2^32 columns in the original (strategic.md, glyph 0x7F).
                eprintln!("stop point: the original game crashes here (text glyph of width 0)");
            }
            let gx = f.x_of(c as usize) as i32;
            if mode != 300 {
                self.table0[0] = mode as u8;
            }
            for j in 0..f.height as i32 {
                for i in 0..w {
                    let off = j as i64 * f.pitch as i64 + (gx + i) as i64;
                    let p = font_byte(&self.d.fonts, font, off);
                    if shadow != 0 && mode == 300 {
                        if p != 0 {
                            let v = self.table0[p as usize];
                            self.put(x + i, y + j, v);
                        }
                    } else {
                        let v = self.table0[p as usize];
                        if v != 0 {
                            self.put(x + i, y + j, v);
                        }
                    }
                }
            }
            if mode != 300 {
                self.table0[0] = 0;
            }
            x += w;
            // Signed here, unsigned in the original (`ja`): the same, because the clip above keeps x + w at 0 or more.
            if x > 319 {
                break;
            }
        }
        self.table0[0xFF] = s_ff;
        self.table0[0xFE] = s_fe;
    }

    pub fn number2(&mut self, v: i32, x: i32, y: i32, fg: u8, shadow: u8, font: usize) {
        let mut s = v.to_string().into_bytes();
        while s.len() < 2 {
            s.insert(0, b'^');
        }
        self.text(&s, x, y, false, fg, shadow, font);
    }

    pub fn frame(&mut self, x0: i32, y0: i32, mut x1: i32, mut y1: i32, colour: u8) {
        self.table0[1] = colour;
        // Frames wider than 194 or higher than 83: the rest e (8-bit signed) is drawn as a second part.
        let (mut ew, mut eh) = (0, 0);
        if x1 - x0 > 0xC2 {
            ew = (x1 - x0 - 0xC2) as i8 as i32;
            x1 = x0 + 0xC2;
        }
        if y1 - y0 > 0x53 {
            eh = (y1 - y0 - 0x53) as i8 as i32;
            y1 = y0 + 0x53;
        }
        self.frame_row(x0, y0, x1 - x0);
        self.frame_row(x0, y1 + eh, x1 - x0 + 1);
        if ew > 0 {
            self.frame_row(x1, y0, ew);
            self.frame_row(x1, y1 + eh, ew + 1);
        }
        self.frame_column(x0, y0, y1 - y0);
        self.frame_column(x1 + ew, y0, y1 - y0);
        if eh > 0 {
            self.frame_column(x0, y1, eh);
            self.frame_column(x1 + ew, y1, eh);
        }
        self.table0[1] = 1;
    }

    fn frame_row(&mut self, x: i32, y: i32, n: i32) {
        for i in 0..n {
            let v = self.table0[self.sheet(78 + i, 159) as usize];
            if v != 0 {
                self.put(x + i, y, v);
            }
        }
    }

    fn frame_column(&mut self, x: i32, y: i32, n: i32) {
        for j in 0..n {
            let v = self.table0[self.sheet(319, j) as usize];
            if v != 0 {
                self.put(x, y + j, v);
            }
        }
    }

    /// PortraitTint? 0x31C0C.
    pub fn tint(&mut self, x: i32, y: i32, c: u8) {
        let old = self.table0[0x1B];
        self.table0[0x1B] = c;
        self.remapped0(17, x + 2, y + 2);
        self.table0[0x1B] = old;
    }

    pub fn darken(&mut self, x: i32, y: i32) {
        for r in 0..25 {
            for c in 0..28 {
                if let Some(p) = self.get(x + 2 + c, y + 2 + r) {
                    let v = self.table29[p as usize];
                    self.put(x + 2 + c, y + 2 + r, v);
                }
            }
        }
    }

    pub(crate) fn fill(&mut self, x: i32, y: i32, w: i32, h: i32, v: u8) {
        for r in 0..h {
            for c in 0..w {
                self.put(x + c, y + r, v);
            }
        }
    }
}

/// World and front-end values for one slot drawing.
pub struct SlotEnv<'a> {
    pub slot: u8,
    pub selected: u8,
    pub combat: u8,
    pub opt_onscreen: u8,
    pub speaker: u8,
    pub hover: u8,
    pub ap_warned: [u8; 8],
    /// 8 x 1024 face buffers.
    pub faces: &'a [u8],
    pub tileset: &'a Tileset,
}

fn s8(r: &[u8], o: usize) -> i32 {
    r[o] as i8 as i32
}
fn u16_at(r: &[u8], o: usize) -> u16 {
    u16::from_le_bytes([r[o], r[o + 1]])
}

/// "Drawing one slot". `soldiers` = 48 x 512 records (the eye state +0x1AC is written).
/// SlotMouse 0x118F3: "cont" over the face of slot `slot` (font 0, centred at face + (15, 12), fg 0x27, shadow 1).
pub fn draw_cont(p: &mut Painter, slot: u8, soldiers: &[u8]) {
    if let Some((x, y)) = cont_at(slot, soldiers) {
        p.text(b"cont", x, y, true, 0x27, 1, 0);
    }
}

/// The centre of `draw_cont`'s "cont" (None for no slot): face corner of the slot's side + (15, 12).
pub fn cont_at(slot: u8, soldiers: &[u8]) -> Option<(i32, i32)> {
    let slot = slot as usize;
    if slot >= 8 || soldiers.len() < (slot + 1) * REC {
        return None;
    }
    let (bx, by) = (if slot < 4 { 0 } else { 272 }, 14 + 42 * (slot as i32 % 4));
    let side = &SIDE[if soldiers[slot * REC + 0x180] == 0 { 0 } else { 1 }];
    Some((bx + side[0].0 + 15, by + side[0].1 + 12))
}

pub fn draw_slot(p: &mut Painter, env: &SlotEnv, soldiers: &mut [u8]) {
    if env.opt_onscreen >= 2 || env.slot > 7 {
        return;
    }
    let slot = env.slot as usize;
    // SPEC GAP: "the merc in the slot" is taken as soldier record number `slot` (records 0..7 are the mercs).
    let s = &mut soldiers[slot * REC..(slot + 1) * REC];
    let (bx, by) = (if slot < 4 { 0 } else { 272 }, 14 + 42 * (slot as i32 % 4));
    let sel = env.slot == env.selected;
    let k = if sel { 7 } else { 0 };
    // SPEC GAP: a column byte +0x180 other than 0/1 is not described; any non-zero value is taken as right.
    let side = &SIDE[if s[0x180] == 0 { 0 } else { 1 }];
    let (fx, fy) = (bx + side[0].0, by + side[0].1);
    let (hx, hy) = (bx + side[2].0, by + side[2].1);
    let in_cur = s[0x182] != 0;
    let in_play = s[0x138] != 0;

    // 1.
    p.element(k, bx, by, None);
    p.element(k + 4, hx, hy, None);
    // 2. Name.
    slot_name(p, s, bx, by, sel);
    // 3. Bars (DrawEnergyBar? 0x32610).
    if in_cur {
        health_bar(p, s, hx, hy, if sel { 3 } else { 0 });
        let kh = if sel { 3 } else { 0 };
        if in_play {
            let energy = s8(s, 0x18A);
            let mut v = energy / 4;
            if v == 0 && energy > 0 {
                v = 1;
            }
            bar_shape(p, v, 89 + kh, 83, 82, bx + side[3].0, by + side[3].1);
            let v = s8(s, 0x1A5).min(25);
            bar_shape(p, v, 90 + kh, 85, 84, bx + side[4].0, by + side[4].1);
        }
    }
    // 4.
    let speaker = env.speaker == env.slot;
    if s[0x194] == 0 || !speaker {
        portrait(p, env, s, fx, fy, bx, by);
    }
    // 5. Shade (ShadeSlot? 0x552F8).
    if s[0x194] != 0 || s[0x1BB] != 0 {
        let old = p.table0[0x1B];
        p.table0[0x1B] = 0x70;
        for (dx, dy) in [(0, 0), (24, 0), (0, 20), (24, 20)] {
            p.remapped0(104, bx + dx, by + dy);
        }
        p.table0[0x1B] = old;
        if speaker {
            portrait(p, env, s, fx, fy, bx, by);
        }
    }
}

fn health_bar(p: &mut Painter, s: &[u8], hx: i32, hy: i32, kh: usize) {
    let (x, mut y) = (hx + 1, hy + 1);
    let (hp, hp_max, wounds) = (s8(s, 0x183), s8(s, 0x184), s8(s, 0x185));
    let gap = (100 - hp_max) / 4;
    let mut nh = if hp == 0 { 0 } else { (hp - 1) / 4 + 1 };
    let nw = if wounds == 0 { 0 } else { (wounds - 1) / 4 + 1 };
    if (100 - hp_max) % 4 < (100 - (hp_max - hp)) % 4 && nh > 1 {
        nh -= 1;
    }
    let rest = 25 - gap - nh - nw;
    if gap > 0 {
        p.element(88 + kh, x, y, Some(gap));
    }
    y += gap;
    if nw > 0 {
        p.bar_rows(87, x, y, nw);
    }
    y += nw;
    if rest > 0 {
        p.bar_rows(86, x, y, rest);
    }
    y += rest;
    if nh > 0 {
        p.element(81, x, y, None);
        p.bar_rows(80, x, y + 1, nh - 1);
    }
}

fn bar_shape(p: &mut Painter, v: i32, empty: usize, top: usize, body: usize, x: i32, mut y: i32) {
    let gap = 25 - v;
    if gap > 0 {
        p.element(empty, x, y, Some(gap));
    }
    if v > 0 {
        y += gap;
        p.element(top, x, y, None);
        if v > 1 {
            p.bar_rows(body, x, y + 1, v - 1);
        }
    }
}

/// The inputs of the slot portraits over the full view (TacticalRedraw 0x17759, DrawSlotPortrait? 0x320A0).
pub struct ViewPortraits<'a> {
    pub d: &'a PanelData,
    pub table0: [u8; 256],
    pub table29: &'a [u8; 256],
    pub tileset: &'a Tileset,
    /// 8 x 1024 face buffers.
    pub faces: &'a [u8],
    pub mode: u8,
    pub lit: u8,
    pub combat: u8,
    pub ap_warned: [u8; 8],
}

impl ViewPortraits<'_> {
    /// View modes 2 and 3: for each of the 8 mercs, the lit slot's portrait, then the merc's own when it is dying
    /// (+0x189) or its counter +0x1DC is above 0. They go over the map, before the view texts. `bb` = the back buffer
    /// (pitch 384); `soldiers`: the eye state +0x1AC is written.
    pub fn draw(&mut self, bb: &mut [u8], soldiers: &mut [u8]) {
        if self.mode < 2 || self.faces.len() < 8 * 1024 || soldiers.len() < 8 * REC {
            return;
        }
        // The panel coordinates of a face: back buffer x - 32, y - 14 (mode 2) or y - 28 (mode 3).
        let top = if self.mode == 2 { 14usize } else { 28 };
        let mut buf = vec![0u8; (PW * PH) as usize];
        let rows = |y: usize| ((y + top) * 384 + 32, y * PW as usize);
        let mut any = false;
        for k in 0..8usize {
            let r = &soldiers[k * REC..(k + 1) * REC];
            let own = r[0x189] != 0 || i32::from_le_bytes([r[0x1DC], r[0x1DD], r[0x1DE], r[0x1DF]]) > 0;
            if self.lit >= 0x40 && !own {
                continue;
            }
            if !any {
                any = true;
                for y in 0..PH as usize {
                    let (b, t) = rows(y);
                    if let Some(src) = bb.get(b..b + PW as usize) {
                        buf[t..t + PW as usize].copy_from_slice(src);
                    }
                }
            }
            if self.lit < 8 {
                self.one(&mut buf, soldiers, self.lit as usize);
            }
            if own {
                self.one(&mut buf, soldiers, k);
            }
        }
        if any {
            for y in 0..PH as usize {
                let (b, t) = rows(y);
                if let Some(dst) = bb.get_mut(b..b + PW as usize) {
                    dst.copy_from_slice(&buf[t..t + PW as usize]);
                }
            }
        }
    }

    /// DrawSlotPortrait? 0x320A0 with DL = 0. It lacks the steps of 0x31C98 for the darkening, the assignment text
    /// and the coloured frames: the frame is always white.
    fn one(&mut self, buf: &mut [u8], soldiers: &mut [u8], slot: usize) {
        let s = &mut soldiers[slot * REC..(slot + 1) * REC];
        let (bx, by) = (if slot < 4 { 0 } else { 272 }, 14 + 42 * (slot as i32 % 4));
        let side = &SIDE[if s[0x180] == 0 { 0 } else { 1 }];
        let (fx, fy) = (bx + side[0].0, by + side[0].1);
        let mut table0 = self.table0;
        let mut p = Painter { d: self.d, buf, table0: &mut table0, table29: self.table29 };
        let face = slot * 1024;
        for r in 1..28 {
            for c in 1..31 {
                p.put(fx + c, fy + r, self.faces[face + r as usize * 32 + c as usize]);
            }
        }
        let (hp, dying, ap) = (s8(s, 0x183), s8(s, 0x189), s8(s, 0x1A5));
        if dying != 0 {
            s[0x1AC] = 0;
        } else if ap == 0 && hp >= 15 {
            s[0x1AC] = 3;
        }
        let e = s8(s, 0x1AC);
        if e < 2 {
            let row = 28 + 2 * e;
            let eye_y = fy + 2 + s[0x1AA] as i32;
            for r in 0..2 {
                for c in 0..28 {
                    let off = face as i64 + (row + r) as i64 * 32 + c as i64;
                    let v = if off >= 0 { self.faces.get(off as usize).copied().unwrap_or(0) } else { 0 };
                    p.put(fx + 2 + c, eye_y + r, v);
                }
            }
        }
        if dying != 0 || hp < 15 {
            if let Some(&c) = DYING_TINT.get(dying as usize).filter(|_| dying >= 0) {
                p.tint(fx + 1, fy + 1, c);
            }
        }
        let icon = s[0x181];
        if icon != 0 {
            if let Some(px) = self.tileset.sprites.get(413 + icon as usize) {
                for q in px {
                    p.put(fx + 3 + q.col, fy + 13 + q.row, q.val);
                }
            }
        }
        if self.combat != 0 && hp >= 15 {
            let (fg, sh) = if ap < s8(s, 0xEF) {
                (0x72, 1)
            } else {
                (if self.ap_warned.get(s[0x17A] as usize).copied().unwrap_or(0) != 0 { 0x16 } else { 0xFF }, 0xFE)
            };
            if fg != 0xFF {
                if ap < 10 {
                    p.fill(fx + 25, fy + 20, 5, 7, 0);
                } else {
                    p.fill(fx + 22, fy + 20, 8, 7, 0);
                }
            }
            p.number2(ap, fx + 22, fy + 21, fg, sh, 0);
        }
        p.frame(fx + 1, fy + 1, fx + 30, fy + 27, 0xFF);
    }
}

/// DrawMercPortraitPanel? 0x31C98.
fn portrait(p: &mut Painter, env: &SlotEnv, s: &mut [u8], fx: i32, fy: i32, bx: i32, by: i32) {
    if env.opt_onscreen > 1 || env.slot > 7 {
        return;
    }
    let slot = env.slot as usize;
    let face = slot * 1024;
    // 1. Face buffer, 31 x 28, all pixels.
    for r in 0..28 {
        for c in 0..31 {
            p.put(fx + c, fy + r, env.faces[face + r as usize * 32 + c as usize]);
        }
    }
    let (hp, dying, ap) = (s8(s, 0x183), s8(s, 0x189), s8(s, 0x1A5));
    let in_cur = s[0x182] != 0;
    let in_play = s[0x138] != 0;
    // 2. Eyes.
    if dying != 0 {
        s[0x1AC] = 0;
    } else if ap == 0 && hp >= 15 {
        s[0x1AC] = 3;
    }
    let e = s8(s, 0x1AC);
    if e < 2 {
        let row = 28 + 2 * e;
        let eye_y = fy + 2 + s[0x1AA] as i32;
        for r in 0..2 {
            for c in 0..28 {
                // SPEC GAP: a negative eye state reads rows above the face buffer; taken from the 8 x 1024 face
                // array at that offset (0 outside it).
                let off = face as i64 + (row + r) as i64 * 32 + c as i64;
                let v = if off >= 0 { env.faces.get(off as usize).copied().unwrap_or(0) } else { 0 };
                p.put(fx + 2 + c, eye_y + r, v);
            }
        }
    }
    // 3. Tint.
    if dying != 0 || hp < 15 {
        // SPEC GAP: a dying state outside 0..3 has no tint colour; no tint is drawn then.
        if let Some(&c) = DYING_TINT.get(dying as usize).filter(|_| dying >= 0) {
            p.tint(fx, fy, c);
        }
    } else if in_cur && !in_play {
        p.tint(fx, fy, 2);
    }
    // 4. Icon.
    let icon = s[0x181];
    if icon != 0 {
        if let Some(px) = env.tileset.sprites.get(413 + icon as usize) {
            for q in px {
                p.put(fx + 2 + q.col, fy + 13 + q.row, q.val);
            }
        }
    }
    // 5. AP number.
    if hp >= 15 && env.combat != 0 && in_play {
        let a = ap;
        let (fg, sh) = if a < s8(s, 0xEF) {
            (0x72, 1)
        } else {
            (if env.ap_warned[slot] != 0 { 0x16 } else { 0xFF }, 0xFE)
        };
        if fg != 0xFF {
            if a < 10 {
                p.fill(fx + 25, fy + 20, 5, 7, 0);
            } else {
                p.fill(fx + 22, fy + 20, 8, 7, 0);
            }
        }
        p.number2(a, fx + 22, fy + 21, fg, sh, 0);
    }
    // 6. Darken.
    let assign = s[0x17E];
    if assign != 0 || ap == 0 {
        p.darken(fx, fy);
    }
    // 7. Assignment text.
    if (1..=7).contains(&assign) {
        p.text(ASSIGN[assign as usize], fx + 16, fy + 12, true, 0x27, 1, 0);
    }
    // 8. Frames.
    slot_frames(p, env, s, fx, fy, bx, by);
    // 9. Hover frame. ClearStripHover? 0x1CB58 (SlotMouse, TacticalKeyHandler): strip hover 0x93448 = 0x40 and
    // RedrawMercSlot 0x331A4(old, 1) at once. The window sets its strip hover from the mouse at every motion (0xFF =
    // none, main.rs) and draws the whole frame each pass, so the old slot's frame goes in the same frame.
    if env.hover == env.slot && hp > 0 {
        let side = &SIDE[if s[0x180] == 0 { 0 } else { 1 }];
        let (hx, hy) = (bx + side[2].0, by + side[2].1);
        p.frame(hx, hy, hx + 10, hy + 26, 0x27);
    }
}

/// "Drawing one slot" step 8, the frames (also the switch view's last step): DrawSlotFrames? 0x333B8, DrawSlotFrame?
/// 0x335F8, DrawFaceFrame? 0x3341C.
fn slot_frames(p: &mut Painter, env: &SlotEnv, s: &[u8], fx: i32, fy: i32, bx: i32, by: i32) {
    let in_cur = s[0x182] != 0;
    let in_play = s[0x138] != 0;
    if in_cur && in_play {
        if env.speaker == env.slot {
            p.frame(fx + 1, fy + 1, fx + 30, fy + 27, 0xFF);
        } else {
            let anim = u16_at(s, 0x1E8 + 2 * s[0x1F3] as usize);
            let pairs_differ = u16_at(s, 0x114) != u16_at(s, 0x124)
                || u16_at(s, 0x116) != u16_at(s, 0x126)
                || u16_at(s, 0x118) != u16_at(s, 0x128);
            // SPEC GAP: +0x1B9 < 5 compared as an unsigned byte (its type is u8).
            let at_waypoint = !(s[0x1B9] < 5 && anim >= 4 && pairs_differ);
            if (!at_waypoint || (u16_at(s, 0x120) as i16 as i32) < 0xC41) && env.opt_onscreen < 2 {
                let colour = if s[0x1A8] == 0 { 0xA2 } else { 0x16 };
                if s[0x17F] == 0 {
                    p.frame(fx + 1, fy + 1, fx + 30, fy + 27, colour);
                } else {
                    p.frame(bx + 1, by + 1, bx + 46, by + 40, colour);
                }
            }
        }
    }
}

/// "Drawing one slot" step 2, the name (also drawn by the switch view): DrawMercName? 0x32DB8, NameColours? 0x35830.
fn slot_name(p: &mut Painter, s: &[u8], bx: i32, by: i32, sel: bool) {
    // SPEC GAP: the nickname is taken as a C string from +0x16E up to its NUL (at most to the record's end).
    let mut name: Vec<u8> = s[0x16E..].iter().copied().take_while(|&b| b != 0).collect();
    if let Some(b) = name.first_mut() {
        *b = b.wrapping_add(0x20);
        if *b == 0 {
            name.clear();
        }
    }
    let (fg, sh) = if sel { (0x16, 0x15) } else { (0x0E, 0xFE) };
    p.text(&name, bx + 3, by + 32, false, fg, sh, 1);
}

/// The slot origin (X, Y) of slot k.
pub fn slot_origin(k: u8) -> (i32, i32) {
    (if k < 4 { 0 } else { 272 }, 14 + 42 * (k as i32 % 4))
}

/// The three switch labels of the switch view (panel.md "The switch view").
const SWITCH_LABELS: [&[u8]; 3] = [b"max aim", b"rsv pts", b"mute"];

/// panel.md "The switch view", drawing: the slot of merc `env.slot` with its three switches.
pub fn draw_switch_view(p: &mut Painter, env: &SlotEnv, soldiers: &[u8]) {
    if env.opt_onscreen >= 2 || env.slot > 7 {
        return;
    }
    let slot = env.slot as usize;
    let s = &soldiers[slot * REC..(slot + 1) * REC];
    let (bx, by) = slot_origin(env.slot);
    let sel = env.slot == env.selected;
    p.element(if sel { 7 } else { 0 }, bx, by, None);
    // The label colours: the original's 0x35858 looks +0x17A (selected slot 0x284F11) and gives 0x1E,0x15 or 0x0D,0x07.
    let (fg, sh) = if sel { (0x1E, 0x15) } else { (0x0D, 0x07) };
    for (i, label) in SWITCH_LABELS.iter().enumerate() {
        let y = by + 5 + 9 * i as i32;
        p.text(label, bx + 3, y, false, fg, sh, 0);
        switch_face(p, env.slot, env.selected, i, s[0x16A + i] != 0, false);
    }
    slot_name(p, s, bx, by, sel);
    let side = &SIDE[if s[0x180] == 0 { 0 } else { 1 }];
    slot_frames(p, env, s, bx + side[0].0, by + side[0].1, bx, by);
}

/// Switch i's face: on / off, pressed or not (panel.md "The switch view").
pub fn switch_face(p: &mut Painter, slot: u8, selected: u8, i: usize, on: bool, pressed: bool) {
    let (bx, by) = slot_origin(slot);
    let base = match (pressed, on) {
        (false, false) => 0x45,
        (false, true) => 0x48,
        (true, false) => 0x46,
        (true, true) => 0x47,
    };
    let e = base + if slot == selected { 4 } else { 0 };
    p.element(e, bx + 0x23, by + 5 + 9 * i as i32 - 1, None);
}

/// The view byte stored at +0x17F, as SetSlotView 0x332E4 keeps it: the value the slot mouse wrote (+1 per right
/// click, 0x11714) when it is 0 or 1; a larger value is the close, 0 (panel.md "The switch view").
pub fn set_slot_view(view: u8) -> u8 {
    if view > 1 { 0 } else { view }
}

/// The three frame elements SetSlotView 0x332E4 draws for view byte `view`: the open frames 1..3 for 1, else the
/// close frames 3..1; and the draw after them: true = the switch view, false = the slot again.
pub fn slot_view_frames(view: u8) -> ([u8; 3], bool) {
    if set_slot_view(view) == 1 { ([1, 2, 3], true) } else { ([3, 2, 1], false) }
}

/// Each frame of the open / close animation stays this long (BusyWait? 0x67250 in SetSlotView 0x332E4).
pub const SLOT_VIEW_MS: u32 = 20;

/// The frame element of the animation for view byte `view`, `ms` after its start; None = the frames are over.
pub fn slot_view_frame(view: u8, ms: u128) -> Option<u8> {
    slot_view_frames(view).0.get(ms as usize / SLOT_VIEW_MS as usize).copied()
}

/// SetSlotView 0x332E4 runs its animation only with g_redrawOff (`opt_onscreen`) < 2.
pub fn slot_view_runs(redraw_off: u8) -> bool {
    redraw_off < 2
}

/// One step of the animation in order (the callees of SetSlotView 0x332E4).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SlotViewCall {
    /// the slot background with this frame element
    Back(u8),
    /// the slot to the screen
    Copy,
    /// a wait of this many ms
    Wait(u32),
    /// the slot again
    Slot,
    /// the switch view
    Switch,
}

/// What SetSlotView 0x332E4 does for view byte `view`: the byte it keeps at +0x17F and the steps.
pub fn slot_view_calls(view: u8, redraw_off: u8) -> (u8, Vec<SlotViewCall>) {
    if !slot_view_runs(redraw_off) {
        return (view, Vec::new());
    }
    let mut calls = Vec::new();
    let mut ms = 0;
    while let Some(f) = slot_view_frame(view, ms) {
        calls.extend([SlotViewCall::Back(f), SlotViewCall::Copy, SlotViewCall::Wait(SLOT_VIEW_MS)]);
        ms += SLOT_VIEW_MS as u128;
    }
    calls.push(if slot_view_frames(view).1 { SlotViewCall::Switch } else { SlotViewCall::Slot });
    calls.push(SlotViewCall::Copy);
    (set_slot_view(view), calls)
}

/// One frame of the open / close animation: slot background element e (+7 for the selected slot).
pub fn switch_frame(p: &mut Painter, slot: u8, selected: u8, e: usize) {
    let (bx, by) = slot_origin(slot);
    p.element(e + if slot == selected { 7 } else { 0 }, bx, by, None);
}

/// The switch under screen pixel (x, y) in slot k's switch view: 8 x 5 at (X + 0x23, Y + 5 + 9 i) (inclusive
/// rectangle (X + 0x23, Y + 5 + 9 i) .. (X + 0x2B, Y + 10 + 9 i)).
pub fn switch_at(k: u8, x: i32, y: i32) -> Option<usize> {
    let (bx, by) = slot_origin(k);
    (0..3).find(|&i| {
        let y0 = by + 5 + 9 * i as i32;
        (bx + 0x23..=bx + 0x2B).contains(&x) && (y0..=y0 + 5).contains(&y)
    })
}

/// Table 29 as the panel uses it.
pub fn table29(c: &Common) -> &[u8; 256] {
    &c.remap[29]
}

/// Number n of the stat popup: (record offset, signed, dx, dy, colour).
const POPUP: [(usize, bool, i32, i32, u8); 10] = [
    (0x112, false, 77, 32, 237),
    (0x183, true, 28, 32, 237),
    (0x18B, true, 28, 4, 237),
    (0x18C, true, 28, 11, 41),
    (0x18D, true, 28, 18, 237),
    (0x18E, true, 28, 25, 41),
    (0x18F, true, 72, 4, 237),
    (0x190, true, 72, 11, 41),
    (0x191, true, 72, 18, 237),
    (0x192, true, 72, 25, 41),
];

/// "Stat popup" steps 1 and 2: the picture and the 10 numbers of record `k` go into the panel buffer at (173, 18).
pub fn draw_popup(p: &mut Painter, soldiers: &[u8], k: usize) {
    let r = &soldiers[k * REC..(k + 1) * REC];
    for y in 0..POP_H {
        for x in 0..POP_W {
            p.put(POP_X + x, POP_Y + y, p.d.popinv[(y * POP_W + x) as usize]);
        }
    }
    for n in 0..POPUP.len() {
        popup_number(p, r, n);
    }
}

/// Number n of the stat popup of record `r` (shadow pass, then colour pass) at the picture corner (173, 18).
pub fn popup_number(p: &mut Painter, r: &[u8], n: usize) {
    let (o, signed, dx, dy, colour) = POPUP[n];
    let v = if signed { r[o] as i8 as i32 } else { u16_at(r, o) as i32 };
    let mut s = v.to_string().into_bytes();
    while s.len() < 4 {
        s.insert(0, b'^');
    }
    let (x, y) = (POP_X + dx, POP_Y + dy);
    p.text(&s, x + 1, y + 1, false, 7, 0, 0);
    p.text(&s, x, y, false, colour, 0, 0);
}

/// Step 3: the 100 x 42 block at (173, 18) of `panel` goes to `screen` next to the slot origin (sx, sy) of slot k.
pub fn popup_to_screen(panel: &[u8], screen: &mut [u8], k: usize, sx: i32, sy: i32) {
    let x0 = if k < 4 { sx + 48 } else { sx - 100 };
    for y in 0..POP_H {
        for x in 0..POP_W {
            let off = ((sy + y) * PW + x0 + x) as i64;
            if off >= 0 && (off as usize) < screen.len() && (x0 + x) >= 0 && (x0 + x) < PW {
                screen[off as usize] = panel[((POP_Y + y) * PW + POP_X + x) as usize];
            }
        }
    }
}

#[cfg(test)]
mod cont_tests {
    use super::*;

    /// Ledger rows c2aea50:ja1front:15485/15492/15499/15500: "cont" of slot k sits on its own strip face (rows 42
    /// apart, left strip slots 0..3, right strip 4..7) at the face side of its own record +0x180.
    #[test]
    fn cont_sits_on_the_slot_face() {
        let mut sol = vec![0u8; 48 * REC];
        sol[6 * REC + 0x180] = 1;
        assert_eq!(cont_at(0, &sol), Some((14 + 15, 14 + 3 + 12)));
        assert_eq!(cont_at(2, &sol), Some((14 + 15, 14 + 84 + 3 + 12)));
        assert_eq!(cont_at(6, &sol), Some((272 + 3 + 15, 14 + 84 + 3 + 12)));
        assert_eq!(cont_at(7, &sol), Some((272 + 14 + 15, 14 + 126 + 3 + 12)));
        assert_eq!(cont_at(8, &sol), None);
        assert_eq!(cont_at(1, &sol[..REC]), None);
        // Ledger row c2aea50:ja1front:15483: slot 2 needs 3 records.
        assert_eq!(cont_at(2, &sol[..2 * REC]), None);
    }
}

#[cfg(test)]
mod morph_tests {
    /// The death morph frames differ one from the next, and frame 9 is the face that load_face kind 2 leaves.
    #[test]
    fn morph_frames() {
        let iso = concat!(env!("CARGO_MANIFEST_DIR"), "/../../rom/JA1.gog");
        if !std::path::Path::new(iso).exists() {
            return;
        }
        let mut frames = vec![];
        for i in 0..10 {
            let mut f = vec![0u8; 1024];
            super::load_morph(iso, 5, i, &mut f).unwrap();
            frames.push(f);
        }
        assert!(frames.windows(2).filter(|p| p[0] != p[1]).count() >= 5);
        let mut nine = vec![0u8; 1024];
        super::load_morph(iso, 5, 9, &mut nine).unwrap();
        assert_eq!(nine, frames[9]);
        assert!(super::load_morph(iso, 5, 10, &mut nine).is_err());
        // ReplayDeathAnim 0x65FB0 takes MF1.DAT for a face below 30 (`c < 0x1e`), MF2.DAT from face 30 on: each
        // archive holds only its own faces, so both sides of the limit must load.
        super::load_morph(iso, 29, 0, &mut nine).unwrap();
        super::load_morph(iso, 30, 0, &mut nine).unwrap();
    }
}

#[cfg(test)]
mod gap_minor_tests {
    use super::*;

    fn pcx_file(w: u16, h: u16, planes: u8, raw: &[u8]) -> Vec<u8> {
        let mut d = vec![0u8; 128];
        d[8..10].copy_from_slice(&(w - 1).to_le_bytes());
        d[10..12].copy_from_slice(&(h - 1).to_le_bytes());
        d[65] = planes;
        d[66..68].copy_from_slice(&w.to_le_bytes());
        d.extend_from_slice(raw);
        d
    }

    /// Ledger rows ja1front:14165 + c2aea50:ja1front:14842: a short PCX is an error (no panic), and row y starts at
    /// y * bytes-per-line * planes (only the first plane is kept).
    #[test]
    fn pcx_short_data_and_plane_rows() {
        assert!(pcx(&pcx_file(2, 2, 1, &[1, 2, 3])).is_err());
        let (w, h, px) = pcx(&pcx_file(2, 2, 2, &[1, 2, 9, 9, 3, 4, 9, 9])).unwrap();
        assert_eq!((w, h, px), (2, 2, vec![1, 2, 3, 4]));
    }

    /// Ledger rows ja1front:14281/14283: slots 0..7 note their frame picture, slot 8 and above are ignored.
    #[test]
    fn face_light_note_only_for_8_slots() {
        take_face_lights();
        note_face_light(3, 5);
        note_face_light(8, 9);
        assert_eq!(take_face_lights(), [0, 0, 0, 5, 0, 0, 0, 0]);
    }

    fn rom() -> Option<(Arc<Common>, Arc<PanelData>)> {
        let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
        let (iso, exe) = (root.join("rom/JA1.gog"), root.join("build/game113b/JA.EXE"));
        if !iso.exists() || !exe.exists() {
            return None;
        }
        let (iso, exe) = (iso.to_str().unwrap(), exe.to_str().unwrap());
        Some((crate::common(iso).unwrap(), panel_data(iso, exe).unwrap()))
    }

    /// Ledger rows c2aea50:ja1front:15297/15308: a read one past the end of the 320 x 200 buffer or the 320 x 160
    /// sheet gives None / 0 (no panic).
    #[test]
    fn painter_reads_past_the_end() {
        let Some((c, pd)) = rom() else { return };
        let mut buf = vec![7u8; 64000];
        let mut table0 = c.remap[0];
        let p = Painter { d: &pd, buf: &mut buf, table0: &mut table0, table29: table29(&c) };
        assert_eq!(p.get(319, 199), Some(7));
        assert_eq!(p.get(0, 200), None);
        assert_eq!(p.sheet(0, 160), 0);
    }

    /// Ledger rows ja1front:15489/15501 + c2aea50:ja1front:16168: the stat popup copy drops the rows below the
    /// screen and the columns right of x 319 (no wrap into the next row).
    #[test]
    fn popup_clips_bottom_and_right() {
        let panel = vec![0x44u8; 64000];
        let mut screen = vec![0u8; 64000];
        popup_to_screen(&panel, &mut screen, 0, 250, 170);
        // x0 = 298: columns 298..319 of rows 170..199 only.
        for y in 0..200usize {
            for x in 0..320usize {
                let want = if y >= 170 && x >= 298 { 0x44 } else { 0 };
                assert_eq!(screen[y * 320 + x], want, "({x},{y})");
            }
        }
        let mut screen = vec![0u8; 64000];
        popup_to_screen(&panel, &mut screen, 0, -48, 159);
        assert_eq!(screen[199 * 320], 0x44);
    }

    fn view<'a>(pd: &'a PanelData, c: &'a Common, ts: &'a Tileset, faces: &'a [u8], mode: u8) -> ViewPortraits<'a> {
        ViewPortraits {
            d: pd,
            table0: c.remap[0],
            table29: table29(c),
            tileset: ts,
            faces,
            mode,
            lit: 0,
            combat: 1,
            ap_warned: [0; 8],
        }
    }

    /// Ledger rows ja1front:14997/15004/15135: view modes below 2 and a face buffer short of 8 x 1024 draw nothing;
    /// DrawSlotPortrait 0x320A0 (`cmp ecx, 0xa; jl` at 0x322E8) clears 8 x 7 at face x + 22 for 10 AP and more.
    #[test]
    fn view_portraits_mode_size_and_ap_box() {
        let Some((c, pd)) = rom() else { return };
        let ts = Tileset {
            sprites: vec![],
            items: vec![],
            bank: vec![],
            offsets: vec![],
            palette: [0; 768],
            table0: [0; 256],
            table28: [0; 256],
            decode_errors: 0,
        };
        let faces = vec![0x33u8; 8 * 1024];
        let mut sol = vec![0u8; 8 * REC];
        sol[0x183] = 50; // hp
        sol[0x1A5] = 10; // ap
        sol[0xEF] = 20;
        let bb0 = vec![0x11u8; 384 * 240];
        for (mode, f) in [(0u8, &faces[..]), (1, &faces[..]), (2, &faces[..8 * 1024 - 1])] {
            let mut bb = bb0.clone();
            view(&pd, &c, &ts, f, mode).draw(&mut bb, &mut sol.clone());
            assert!(bb == bb0, "mode {mode} len {}", f.len());
        }
        // A soldier table short of 8 records draws nothing either.
        let mut bb = bb0.clone();
        view(&pd, &c, &ts, &faces, 2).draw(&mut bb, &mut sol.clone()[..7 * REC]);
        assert!(bb == bb0);
        let mut bb = bb0.clone();
        view(&pd, &c, &ts, &faces, 2).draw(&mut bb, &mut sol);
        // Slot 0 face corner (14, 17); mode 2 puts panel (x, y) at back buffer (x + 32, y + 14).
        let at = |x: i32, y: i32| bb[((y + 14) * 384 + x + 32) as usize];
        assert_eq!(at(14 + 10, 17 + 10), 0x33);
        for x in 22..30 {
            assert_eq!(at(14 + x, 17 + 20), 0, "x {x}");
        }
    }
}
