//! The inventory screen body (docs/spec/inventory.md "Drawing the screen (body)"): steps 1..14 of `body(S)`.
use crate::iso::Iso;
use crate::panel::{archive_member, pcx, popup_number, Painter, SlotEnv, PH, PW};
use crate::data::Tileset;
use crate::soldier::REC;
use std::collections::HashMap;
use std::io;
use std::path::Path;
use std::sync::{Arc, Mutex, OnceLock};

/// Files and tables the screen needs besides the panel data (cached per (iso, exe)).
pub struct InvData {
    /// DAT/MAIN.DAT in the game directory.
    pub main: Vec<u8>,
    /// The item type table (136 x 12 bytes) from JA.EXE.
    pub item_types: Vec<u8>,
    /// CD DAT/INV.DAT as loaded (offsetbank::load) and the place of every picture in it.
    pub inv: Vec<u8>,
    pub inv_off: Vec<usize>,
    /// EDT/INVQUOTE.EDT beside JA.EXE (the item texts, panel.md "Item info box"); empty when missing.
    pub quotes: Vec<u8>,
    /// The short item names (136 x 15 bytes, space-padded, NUL at byte 14) from JA.EXE (strategic.md "The A.I.M.
    /// screen", gear view).
    pub item_names: Vec<u8>,
}

pub(crate) const ITEM_TYPES_AT: usize = 0xDBE9C;
const ITEM_TYPES_LEN: usize = 1632;
const ITEM_NAMES_AT: usize = 0xDDB93;
const ITEM_NAMES_LEN: usize = 136 * 15;

fn bad(m: &str) -> io::Error {
    io::Error::new(io::ErrorKind::InvalidData, m.to_string())
}

impl InvData {
    fn load(iso: &mut Iso, exe_path: &str) -> io::Result<InvData> {
        let dir = Path::new(exe_path).parent().map(|p| p.to_path_buf()).unwrap_or_default();
        let main = std::fs::read(dir.join("DAT").join("MAIN.DAT"))?;
        let exe = std::fs::read(exe_path)?;
        let item_types = exe.get(ITEM_TYPES_AT..ITEM_TYPES_AT + ITEM_TYPES_LEN).ok_or_else(|| bad("JA.EXE too short"))?.to_vec();
        let item_names = exe.get(ITEM_NAMES_AT..ITEM_NAMES_AT + ITEM_NAMES_LEN).ok_or_else(|| bad("JA.EXE too short"))?.to_vec();
        let file = iso.read("DAT/INV.DAT")?;
        let loaded = crate::offsetbank::load(Some(&file), crate::offsetbank::INV_MAX)
            .map_err(|_| bad("INV.DAT does not load"))?;
        let (inv, inv_off) = (loaded.image, loaded.entries);
        // SPEC GAP: a missing INVQUOTE.EDT gives empty item texts. The name is tried as given and in lower case.
        let quotes = ["EDT/INVQUOTE.EDT", "edt/invquote.edt", "EDT/invquote.edt", "edt/INVQUOTE.EDT"]
            .iter().find_map(|n| std::fs::read(dir.join(n)).ok()).unwrap_or_default();
        Ok(InvData { main, item_types, inv, inv_off, quotes, item_names })
    }
}

fn inv_cache() -> &'static Mutex<HashMap<(String, String), Arc<InvData>>> {
    static C: OnceLock<Mutex<HashMap<(String, String), Arc<InvData>>>> = OnceLock::new();
    C.get_or_init(|| Mutex::new(HashMap::new()))
}

pub fn inv_data(iso_path: &str, exe_path: &str) -> Result<Arc<InvData>, i32> {
    let key = (iso_path.to_string(), exe_path.to_string());
    if let Some(d) = inv_cache().lock().unwrap_or_else(|e| e.into_inner()).get(&key) {
        return Ok(d.clone());
    }
    let mut iso = Iso::open(iso_path).map_err(|_| crate::E_FILE)?;
    let d = Arc::new(InvData::load(&mut iso, exe_path).map_err(|_| crate::E_FILE)?);
    inv_cache().lock().unwrap_or_else(|e| e.into_inner()).insert(key, d.clone());
    Ok(d)
}

/// World and front-end values of one drawing.
pub struct InvEnv<'a> {
    /// Record index of S.
    pub rec: usize,
    pub selected: u8,
    pub combat: u8,
    pub opt: u8,
    pub speaker: u8,
    pub hover: u8,
    pub ap_warned: [u8; 8],
    pub faces: &'a [u8],
    pub tileset: &'a Tileset,
    pub sector: u8,
    pub enemies: u8,
    pub blink: u8,
    pub sight: [u8; 8],
    pub small_panel: i32,
    pub time_w: i32,
    pub tiles: &'a [u8],
    pub ground: &'a [u8],
}

pub struct InvOut {
    pub armour: i32,
    pub load_result: i32,
    pub blink: u8,
    pub drawn: u8,
    pub item_rects: [u8; 56],
}

/// The inventory table of the item rects (x, y) (SetItemRects? 0x4AAB0 copies it to 0x949F2).
const RECTS: [(i32, i32); 14] = [
    (59, 108),
    (154, 124),
    (112, 43),
    (59, 66),
    (151, 63),
    (59, 87),
    (208, 53),
    (208, 72),
    (208, 91),
    (208, 110),
    (208, 129),
    (208, 148),
    (59, 128),
    (110, 150),
];

const BOX_X: [i32; 7] = [0, 49, 94, 139, 184, 229, 272];
const PIC_X: [i32; 7] = [1, 49, 94, 139, 184, 229, 273];
const SHOWN: [usize; 7] = [0, 7, 8, 9, 10, 11, 1];
pub(crate) const BUTTONS: [(i32, &[u8]); 6] = [(49, b"done"), (86, b"trav"), (123, b"inv"), (160, b"map"), (197, b"opt"), (234, b"abort")];

fn s8(v: u8) -> i32 {
    v as i8 as i32
}

fn rd16(r: &[u8], o: usize) -> i32 {
    i16::from_le_bytes([r[o], r[o + 1]]) as i32
}

impl InvData {
    pub(crate) fn ib(&self, t: u8, i: usize) -> u8 {
        // SPEC GAP: an item type past the 136-entry table reads 0.
        self.item_types.get(12 * t as usize + i).copied().unwrap_or(0)
    }
}

/// Load a MAIN.DAT member into the panel buffer at (x, y); 0 = loaded, -1 = no such member.
pub(crate) fn load_pcx(p: &mut Painter, d: &InvData, name: &str, x: i32, y: i32) -> i32 {
    // SPEC GAP: a member that is present but not a readable PCX counts as missing (-1).
    let Ok(m) = archive_member(&d.main, name) else { return -1 };
    let Ok((w, h, px)) = pcx(&m) else { return -1 };
    for r in 0..h as i32 {
        for c in 0..w as i32 {
            p.put(x + c, y + r, px[r as usize * w + c as usize]);
        }
    }
    0
}

/// Copy a w x h block of the sheet to (x, y); all pixels written.
fn copy_sheet(p: &mut Painter, sx: i32, sy: i32, w: i32, h: i32, x: i32, y: i32) {
    for r in 0..h {
        for c in 0..w {
            let v = p.sheet(sx + c, sy + r);
            p.put(x + c, y + r, v);
        }
    }
}

/// A w x h block of the sheet remapped with table 0 (written unless the result is 0).
fn remap_sheet(p: &mut Painter, sx: i32, sy: i32, w: i32, h: i32, x: i32, y: i32) {
    for r in 0..h {
        for c in 0..w {
            let v = p.table0[p.sheet(sx + c, sy + r) as usize];
            if v != 0 {
                p.put(x + c, y + r, v);
            }
        }
    }
}

fn darken_rect(p: &mut Painter, x: i32, y: i32, w: i32, h: i32) {
    for r in 0..h {
        for c in 0..w {
            if let Some(v) = p.get(x + c, y + r) {
                let n = p.table29[v as usize];
                p.put(x + c, y + r, n);
            }
        }
    }
}

pub(crate) fn number(p: &mut Painter, v: i32, digits: usize, x: i32, y: i32, fg: u8, sh: u8, font: usize) {
    let mut s = v.to_string().into_bytes();
    while s.len() < digits {
        s.insert(0, b'^');
    }
    p.text(&s, x, y, false, fg, sh, font);
}

/// Item picture `pic` size class: width and x offset in the 42-column cursor buffer (PictureSize? 0x46AE4: id <= 20
/// -> 42/0, <= 90 -> 30/6, else 16/13).
pub fn picture_size(pic: u8) -> (usize, i32) {
    let w = if pic <= 20 { 42 } else if pic <= 90 { 30 } else { 16 };
    let dx = match w {
        42 => 0,
        30 => 6,
        _ => 13,
    };
    (w, dx)
}

/// Item picture `pic` at (x, y) as the A.I.M. gear view draws it (strategic.md "The A.I.M. screen"): colour 0 is
/// clear, colour 0x17 (the shadow) darkens the pixel under it with `dark`, every other colour goes through table 0.
pub(crate) fn picture_shadow(p: &mut Painter, d: &InvData, pic: u8, x: i32, y: i32, dark: &[u8; 256]) {
    let (w, dx) = picture_size(pic);
    let Some(&at) = d.inv_off.get(pic as usize) else { return };
    let Some(px) = d.inv.get(at..at + 16 * w) else { return };
    for r in 0..16 {
        for col in 0..w {
            let (cx, cy) = (x + dx + col as i32, y + r as i32);
            match px[r * w + col] {
                0 => {}
                0x17 => {
                    if let Some(v) = p.get(cx, cy) {
                        p.put(cx, cy, dark[v as usize]);
                    }
                }
                c => {
                    let v = p.table0[c as usize];
                    p.put(cx, cy, v);
                }
            }
        }
    }
}

/// Condition bar (value v) at (x, y).
pub(crate) fn cond_bar(p: &mut Painter, v: i32, x: i32, y: i32) {
    let h = 10 - v / 10;
    // h < 0 (v >= 110): a stop point (the original's copy runs off memory); nothing is drawn then.
    if h > 0 {
        copy_sheet(p, 266, 65, 2, h, x, y);
    }
}

/// Item picture `pic` at (x, y) with colour c.
pub(crate) fn picture(p: &mut Painter, d: &InvData, pic: u8, x: i32, y: i32, c: u8) {
    let (w, dx) = picture_size(pic);
    // SPEC GAP: a picture past the bank, or one cut short by the file end, is not drawn.
    let Some(&at) = d.inv_off.get(pic as usize) else { return };
    let Some(px) = d.inv.get(at..at + 16 * w) else { return };
    let old = p.table0[0x17];
    p.table0[0x17] = c;
    for r in 0..16 {
        for col in 0..w {
            let v = p.table0[px[r * w + col] as usize];
            if v != 0 {
                p.put(x + dx + col as i32, y + r as i32, v);
            }
        }
    }
    p.table0[0x17] = old;
}

pub(crate) fn pocket_missing(s: &[u8], e: usize) -> bool {
    let need = match e {
        7 | 8 => 0x32,
        9 => 0x33,
        10 => 0x34,
        11 => 0x35,
        _ => return false,
    };
    s[0x17A] < 8 && s[0x48] < need
}

/// Armour rating (ai.md "Armour").
pub(crate) fn armour(s: &[u8]) -> i32 {
    let cdiv = |a: i32, b: i32| a / b;
    let mut sum = 0;
    for (o, tab) in [
        (0x3Cusize, &[(0x2Du8, 20), (0x2E, 30), (0x2F, 40), (0x30, 60)][..]),
        (0x18, &[(0x23, 5), (0x24, 7), (0x25, 10), (0x26, 15)][..]),
        (0x48, &[(0x31, 20)][..]),
    ] {
        if let Some(&(_, val)) = tab.iter().find(|&&(t, _)| t == s[o]) {
            sum += cdiv(s8(s[o + 2]) * val, 100);
        }
    }
    sum
}

/// The item place k of the soldier whose record starts at `sb`.
/// `rects` = the current item places table; `tiles` / `ground` are read for place 13 only.
pub(crate) fn item_place(p: &mut Painter, d: &InvData, rects: &[(i32, i32); 14], tiles: &[u8], ground: &[u8],
                         so: &mut [u8], sb: usize, k: usize) {
    let (x, y) = rects[k];
    let (c0, bg, bar_y, dy) = if k < 7 { (10u8, 0x30usize, y + 4, 1) } else { (0xE2u8, 0x31usize, y + 5, 2) };
    let bar_x = x + 46;
    let mut buried = false;
    let (t, count, mut cond);
    if k == 13 {
        let tile = u16::from_le_bytes([so[sb + 0x114], so[sb + 0x115]]) as usize;
        // SPEC GAP: a tile past the map, or a ground item index past the table, counts as 255 (nothing).
        let mut g = tiles.get(tile * 8 + 6).copied().unwrap_or(255);
        if g != 255 {
            match ground.get(g as usize * 12..g as usize * 12 + 12) {
                Some(it) => {
                    if it[6] == 255 && s8(it[2]) < 0 {
                        g = 255;
                    }
                }
                None => g = 255,
            }
        }
        if g == 255 {
            (t, count, cond) = (0u8, 0u8, 0u8);
        } else {
            let it = &ground[g as usize * 12..g as usize * 12 + 12];
            (t, count, cond) = (it[0], it[1], it[2]);
            buried = it[6] == 255;
        }
    } else if k == 12 {
        if s8(so[sb + 5]) <= 0 {
            so[sb + 3] = 0;
            so[sb + 4] = 0;
            so[sb + 5] = 0;
        }
        (t, count, cond) = (so[sb + 3], so[sb + 4], so[sb + 5]);
    } else {
        if s8(so[sb + 2]) <= 0 {
            so[sb] = 0;
            so[sb + 1] = 0;
            so[sb + 2] = 0;
        }
        let e = sb + 12 * k;
        (t, count, cond) = (so[e], so[e + 1], so[e + 2]);
        if t != 0 && d.ib(t, 0xA) == 0 {
            cond = 100;
        }
        let (ax, ay) = rects[12];
        if k == 0 {
            if t == 0 {
                p.element(0x43, ax, ay, None);
            } else {
                p.element(0x31, ax, ay, None);
                cond_bar(p, s8(so[sb + 5]), ax + 46, ay + 5);
            }
        }
        p.element(bg, x, y, None);
    }
    if t == 0 {
        if k <= 11 {
            cond_bar(p, s8(cond), bar_x, bar_y);
        }
        return;
    }
    if k < 13 {
        cond_bar(p, s8(cond), bar_x, bar_y);
        picture(p, d, d.ib(t, 4), x + 1, y + dy, c0);
    } else {
        picture(p, d, d.ib(t, 4), x + 5, y + dy, 0);
    }
    let n = s8(count);
    if n < 99 && k != 12 {
        if d.ib(t, 6) < 2 || t == 0x6F {
            if k != 13 && t != 0x6F {
                p.text(format!("{:02}", n.abs()).as_bytes(), x + 2, y + dy + 10, false, 0x29, c0, 0);
            }
        } else if k != 3 {
            p.text(format!("{:2}", n.abs()).as_bytes(), x + 35, y + dy + 10, false, 0xED, c0, 0);
        }
    }
    if n < 0 {
        p.text(b"jammed", x + 22, y + dy + 6, true, 0xED, 1, 0);
    }
    if buried {
        p.text(b"buried", x + 22, y + dy + 6, true, 0xED, 1, 0);
    } else if cond < 10 && (d.ib(t, 9) != 0 || d.ib(t, 7) != 0) {
        p.text(b"useless", x + 22, y + dy + 6, true, 0xED, 1, 0);
    }
}

/// Step 2, the time meter (width w).
pub(crate) fn time_meter(p: &mut Painter, time_w: i32, opt: u8) {
    if time_w > 0 && opt < 3 {
        copy_sheet(p, 131, 87, time_w, 8, 286, 3);
    }
}

/// Step 3, the sector box (the blink byte flips while enemies are in the sector).
pub(crate) fn sector_box(p: &mut Painter, opt: u8, sector: u8, enemies: u8, blink: &mut u8) {
    if opt <= 2 {
        let mut e = 100;
        if enemies != 0 {
            *blink ^= 1;
            e = if *blink == 1 { 100 } else { 99 };
        }
        p.element(e, 15, 3, None);
        number(p, sector as i32 + 1, 2, 21, 4, 0xFF, 1, 0);
    }
}

/// Step 4, first half: the label of button i.
pub(crate) fn button_label(p: &mut Painter, i: usize) {
    let (x, label) = BUTTONS[i];
    p.text(label, x + 19, 4, true, 0x2A, 3, 1);
}

/// Step 4, second half: the shade of button i (two remapped pieces).
pub(crate) fn button_shade(p: &mut Painter, i: usize) {
    let x = BUTTONS[i].0;
    let old = p.table0[0x1B];
    p.table0[0x1B] = 0x70;
    remap_sheet(p, 242, 2, 19, 12, x, 1);
    remap_sheet(p, 242, 2, 18, 12, x + 19, 1);
    p.table0[0x1B] = old;
}

/// Strip box k of the soldier whose record starts at `sb`; `dark` = with the last darken step.
pub(crate) fn strip_box(p: &mut Painter, d: &InvData, so: &mut [u8], sb: usize, k: usize, dark: bool) {
    let (x, px, y, e) = (BOX_X[k], PIC_X[k], 183, SHOWN[k]);
    if so[sb + 0x17A] > 7 {
        return;
    }
    let eb = sb + 12 * e;
    if s8(so[eb + 2]) <= 0 {
        for i in 0..3 {
            so[eb + i] = so[eb + 3 + i];
            so[eb + 3 + i] = 0;
        }
    }
    let t = so[eb];
    let missing = pocket_missing(&so[sb..sb + REC], e);
    if k == 0 && t == 0 {
        p.element(0x2E, x, y, None);
    } else {
        p.element(if k == 0 || k == 6 { 0x69 } else { 0x2F }, x, y, None);
        if missing {
            darken_rect(p, x, y, 42, 16);
        }
    }
    if t != 0 && !missing {
        picture(p, d, d.ib(t, 4), px, y, 8);
        let n = s8(so[eb + 1]);
        if n < 99 && t != 0x6F {
            if d.ib(t, 6) > 1 {
                p.text(format!("{:2}", n.abs()).as_bytes(), px + 34, y + 11, false, 0xED, 0x0A, 0);
            } else {
                p.text(format!("{:02}", n.abs()).as_bytes(), px + 1, y + 11, false, 0x29, 0x0A, 0);
            }
            if n < 0 {
                p.text(b"jammed", px + 21, y + 6, true, 0xED, 1, 0);
            }
        }
        if s8(so[eb + 2]) < 10 && (d.ib(t, 9) != 0 || d.ib(t, 7) != 0) {
            p.text(b"useless", px + 21, y + 6, true, 0xED, 1, 0);
        }
    }
    if dark && !missing {
        darken_rect(p, x, y, if k == 0 || k == 6 { 46 } else { 42 }, 16);
    }
}

fn tile_dist(a: i32, b: i32) -> i32 {
    let (ra, rb) = (a / 56, b / 56);
    let (ca, cb) = (a - 56 * ra, b - 56 * rb);
    (ra - rb).abs().max((ca - cb).abs())
}

/// Steps 1..14. `soldiers` = 48 x 512 records (in/out), `screen` = 320 x 200.
pub fn draw_inventory(p: &mut Painter, d: &InvData, env: &InvEnv, so: &mut [u8], screen: &mut [u8]) -> InvOut {
    let sb = env.rec * REC;
    let slot = so[sb + 0x17A];
    let opt = env.opt;
    let mut blink = env.blink;
    // 1.
    load_pcx(p, d, "TSTRIP.PCC", 0, 0);
    // 2.
    time_meter(p, env.time_w, opt);
    // 3.
    sector_box(p, opt, env.sector, env.enemies, &mut blink);
    // 4.
    for i in 0..BUTTONS.len() {
        button_label(p, i);
        button_shade(p, i);
    }
    // 5.
    let mut item_rects = [0u8; 56];
    for (i, &(x, y)) in RECTS.iter().enumerate() {
        item_rects[4 * i..4 * i + 2].copy_from_slice(&(x as i16).to_le_bytes());
        item_rects[4 * i + 2..4 * i + 4].copy_from_slice(&(y as i16).to_le_bytes());
    }
    // 6.
    load_pcx(p, d, "INVAMB.PCC", 48, 14);
    load_pcx(p, d, &format!("BODY{}.PCC", so[sb + 0x17C]), 110, 62);
    if so[sb + 0x1B1] != 0 {
        load_pcx(p, d, &format!("CAMO{}.PCC", so[sb + 0x17C]), 110, 77);
    }
    // Each load overwrites the loader result, so the last one (BSTRIP) is what stays.
    let load = load_pcx(p, d, "BSTRIP.PCC", 0, 182);
    // 7.
    let name: Vec<u8> = so[sb + 0x16E..sb + REC].iter().copied().take_while(|&b| b != 0).collect();
    p.text(&name, 112, 27, false, 0x0E, 5, 1);
    // 8.
    if slot < 8 {
        for r in 0..25 {
            for c in 0..28 {
                let v = env.faces[slot as usize * 1024 + (2 + r) * 32 + 2 + c];
                p.put(72 + c as i32, 31 + r as i32, v);
            }
        }
        p.text(b"SALARY:", 112, 36, false, 0xB5, 5, 0);
        number(p, u16::from_le_bytes([so[sb + 0x112], so[sb + 0x113]]) as i32, 5, 140, 36, 0x28, 1, 0);
    }
    // 9.
    let arm = armour(&so[sb..sb + REC]);
    number(p, arm, 3, 125, 88, 0xFF, 1, 0);
    p.text(b"%", 137, 88, false, 0xFF, 1, 1);
    // 10.
    for k in 0..14 {
        if !pocket_missing(&so[sb..sb + REC], k) {
            item_place(p, d, &RECTS, env.tiles, env.ground, so, sb, k);
        }
    }
    // 11. (PopupStatNumbers? 0x3FF0C)
    let rec: Vec<u8> = so[sb..sb + REC].to_vec();
    for n in 2..10 {
        popup_number(p, &rec, n);
    }
    // 12.
    let s_tile = rd16(&rec, 0x114);
    for k in 0..8usize {
        let ob = k * REC;
        if s8(so[ob + 0x183]) < 15 {
            so[ob + 0x1AC] = 3;
        }
        so[ob + 0x1BB] = 0;
        let o_tile = rd16(&so[ob..ob + REC], 0x114);
        let mut out = so[ob + 0x182] == 0 || so[ob + 0x17E] != 0 || s8(so[ob + 0x183]) < 1 || tile_dist(s_tile, o_tile) > 2;
        if !out && slot as usize != k {
            out = env.sight[k] == 0;
        }
        if out {
            so[ob + 0x1BB] = 1;
        }
        // Slot redraw: +0x17F = 0 the slot, 1 the switch view (panel.md), other values nothing.
        if opt < 2 && so[ob + 0x17F] <= 1 {
            let se = SlotEnv {
                slot: so[ob + 0x17A],
                selected: env.selected,
                combat: env.combat,
                opt_onscreen: env.opt,
                speaker: env.speaker,
                hover: env.hover,
                ap_warned: env.ap_warned,
                faces: env.faces,
                tileset: env.tileset,
            };
            if so[ob + 0x17F] == 0 {
                crate::panel::draw_slot(p, &se, so);
            } else {
                crate::panel::draw_switch_view(p, &se, so);
            }
        }
    }
    // 13.
    if slot < 8 && (opt < 3 || env.small_panel != 0) {
        for k in 0..7 {
            strip_box(p, d, so, sb, k, true);
        }
    }
    // 14.
    let n = (PW * PH) as usize;
    screen[..n].copy_from_slice(&p.buf[..n]);
    InvOut { armour: arm, load_result: load, blink, drawn: 1, item_rects }
}
