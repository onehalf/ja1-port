//! The island map picture (docs/spec/strategic.md section "The island map picture"): 60 sector cells of the
//! dark or the bright map, with the markers of trees, tappers, guards and mercs.
use crate::iso::Iso;
use crate::panel;
use std::collections::HashMap;
use std::sync::{Arc, Mutex, OnceLock};

/// 0x229D8C: the guards sent per sector at the last guard screen. DrawSectorMarkers? 0x419BC adds them on every map picture. Only
/// the opening of the next guard screen clears them (after its first picture): a load or a new game keeps them, so this
/// is program state and not world state. A quick save holds them (#2).
thread_local! {
    static GUARD_MARKS: std::cell::Cell<[i8; 60]> = const { std::cell::Cell::new([0; 60]) };
}

pub fn guard_marks() -> [i8; 60] {
    GUARD_MARKS.with(|v| v.get())
}

pub fn set_guard_marks(m: [i8; 60]) {
    GUARD_MARKS.with(|v| v.set(m));
}

pub const SW: usize = 320;
pub const SH: usize = 200;
const SCREEN: usize = SW * SH;

/// The loaded pictures: the 640 x 200 map buffer (MAPDARK.PCC left, MAP.PCC right) and the MAP.PCC palette.
pub struct MapData {
    pub buf: Vec<u8>,
    pub palette: [u8; 768],
}

fn pic(main: &[u8], name: &str) -> Result<Vec<u8>, i32> {
    let m = panel::archive_member(main, name).map_err(|_| crate::E_FILE)?;
    let (w, h, pix) = panel::pcx(&m).map_err(|_| crate::E_FILE)?;
    // The pictures are 320 x 146 (strategic.md); rows 146..199 of the 640 x 200 map buffer stay 0.
    if w != 320 || h == 0 {
        return Err(crate::E_FILE);
    }
    let mut v = vec![0u8; SCREEN];
    let n = (h.min(SH)) * 320;
    v[..n].copy_from_slice(&pix[..n]);
    Ok(v)
}

fn load(exe: &str) -> Result<MapData, i32> {
    let dir = std::path::Path::new(exe).parent().map(|p| p.to_path_buf()).unwrap_or_default();
    let main = std::fs::read(dir.join("DAT").join("MAIN.DAT")).map_err(|_| crate::E_FILE)?;
    let dark = pic(&main, "MAPDARK.PCC")?;
    let bright = pic(&main, "MAP.PCC")?;
    let mut buf = vec![0u8; 640 * 200];
    for y in 0..200 {
        buf[y * 640..y * 640 + 320].copy_from_slice(&dark[y * 320..y * 320 + 320]);
        buf[y * 640 + 320..y * 640 + 640].copy_from_slice(&bright[y * 320..y * 320 + 320]);
    }
    // Palette: as SUNSET.PCX (world_load.md End-of-day screen): the last 769 bytes are 0x0C and 256 x RGB, >> 2.
    let f = panel::archive_member(&main, "MAP.PCC").map_err(|_| crate::E_FILE)?;
    if f.len() < 769 || f[f.len() - 769] != 0x0C {
        return Err(crate::E_FILE);
    }
    let mut palette = [0u8; 768];
    for (i, v) in f[f.len() - 768..].iter().enumerate() {
        palette[i] = v >> 2;
    }
    Ok(MapData { buf, palette })
}

type MapCache = Mutex<HashMap<(String, String), Arc<MapData>>>;

/// The map pictures, cached per (iso, exe). The CD is not read (`iso` only keys the cache and is checked).
pub fn map_data(iso_path: &str, exe_path: &str) -> Result<Arc<MapData>, i32> {
    static C: OnceLock<MapCache> = OnceLock::new();
    let c = C.get_or_init(|| Mutex::new(HashMap::new()));
    let key = (iso_path.to_string(), exe_path.to_string());
    if let Some(d) = c.lock().unwrap_or_else(|e| e.into_inner()).get(&key) {
        return Ok(d.clone());
    }
    Iso::open(iso_path).map_err(|_| crate::E_FILE)?;
    let d = Arc::new(load(exe_path)?);
    c.lock().unwrap_or_else(|e| e.into_inner()).insert(key, d.clone());
    Ok(d)
}

/// Marker element (sheet x, y); all 3 x 3.
fn elem(e: u32) -> (usize, usize) {
    match e {
        29 => (251, 74),
        30 => (254, 74),
        31 => (257, 74),
        32 => (260, 74),
        33 => (263, 74),
        112 => (245, 74),
        113 => (248, 74),
        118 => (236, 74),
        _ => (239, 74), // 119
    }
}

/// A marker at screen offset y * 320 + x: no clipping, a row goes on in the next screen row; pixels past the end
/// of the screen and sheet pixel 0 are not written.
fn marker(sheet: &[u8], scr: &mut [u8], pitch: usize, e: u32, x: usize, y: usize) {
    let (sx, sy) = elem(e);
    let base = y * pitch + x;
    let end = if pitch == SW { SCREEN.min(scr.len()) } else { scr.len() };
    for r in 0..3 {
        for c in 0..3 {
            let p = sheet[(sy + r) * 320 + sx + c];
            let off = base + r * pitch + c;
            if p != 0 && off < end {
                scr[off] = p;
            }
        }
    }
}

/// The cell of sector s (0..59) with its `flags` byte: the 30 x 22 block of the map buffer, every pixel.
pub fn cell(m: &MapData, flags: u8, s: usize, scr: &mut [u8]) {
    let (cx, cy) = ((s % 10) * 31 + 6, (s / 10) * 23 + 4);
    let src_x = if flags & 1 == 0 { cx } else { cx + 320 };
    for y in 0..22 {
        let a = (cy + y) * 640 + src_x;
        let d = (cy + y) * SW + cx;
        scr[d..d + 30].copy_from_slice(&m.buf[a..a + 30]);
    }
}

/// Draw the map into `scr` (320 x 200). `sectors` = 60 x 9 bytes, `marks` = 60 signed guard marks.
pub fn draw_map(m: &MapData, sheet: &[u8], sectors: &[u8], marks: &[i8], scr: &mut [u8]) {
    scr[..SCREEN].fill(0);
    for s in 0..60usize {
        sector(m, sheet, &sectors[s * 9..s * 9 + 9], marks[s], s, scr);
    }
}

/// The cell and the markers of sector s (record `r`, 9 bytes; `mark` = its signed guard mark).
/// The original's wrapper for this pair is 0x41994 (called by AssignScreen? 0x41C60 callers): CleanMapCell
/// 0x416E4 then DrawMarkersTo 0x41788 into the map buffer; both halves are diff-tested by replaypic_diff
/// (battle shows).
pub fn sector(m: &MapData, sheet: &[u8], r: &[u8], mark: i8, s: usize, scr: &mut [u8]) {
    let flags = r[0];
    cell(m, flags, s, scr);
    markers(sheet, r, mark, s, scr, SW);
}

/// The markers of sector s (record `r`) at their screen places in `buf`, rows of `pitch` bytes.
pub fn markers(sheet: &[u8], r: &[u8], mark: i8, s: usize, scr: &mut [u8], pitch: usize) {
    let (mercs, tappers, guards, b5, trees, workable, unhealthy) = (r[2], r[3], r[4], r[5], r[6], r[7], r[8]);
    let (x0, y0) = ((s % 10) * 31 + 9, (s / 10) * 23);
    let h = trees as i32 - unhealthy as i32;
    for i in 0..trees as i32 {
        let w = i < workable as i32;
        let ok = i < h;
        let e = match (w, ok) {
            (true, true) => 30,
            (true, false) => 113,
            (false, true) => 29,
            _ => 112,
        };
        marker(sheet, scr, pitch, e, x0 + 3 * i as usize, y0 + 8);
    }
    for i in 0..(tappers / 3) as usize {
        marker(sheet, scr, pitch, 31, x0 + 3 * i, y0 + 12);
    }
    let n = guards as i32 + b5 as i32 + mark as i32;
    for i in 0..n.max(0) {
        let e = if i < guards as i32 {
            32
        } else if i < guards as i32 + b5 as i32 {
            118
        } else {
            119
        };
        marker(sheet, scr, pitch, e, x0 + 3 * i as usize, y0 + 16);
    }
    for i in 0..mercs as usize {
        marker(sheet, scr, pitch, 33, x0 + 3 * i, y0 + 20);
    }
}

/// The cells and the markers on a clear screen: (screen 320 x 200, palette 6-bit).
pub fn island_cells(iso: &str, exe: &str, sectors: &[u8], marks: &[i8]) -> Result<(Vec<u8>, [u8; 768]), i32> {
    if sectors.len() < 540 || marks.len() < 60 {
        return Err(crate::E_ARG);
    }
    let (m, d) = (map_data(iso, exe)?, panel::panel_data(iso, exe)?);
    let mut scr = vec![0u8; SCREEN];
    draw_map(&m, &d.sheet, sectors, marks, &mut scr);
    Ok((scr, m.palette))
}

/// The whole opening picture, InitStrategicMap 0x43ECC: the screen clear, rows 0..145 of the bright map (0x46C1C: its
/// frame and the lines between the cells stay on the screen), then the cells and the markers.
pub fn island_map(iso: &str, exe: &str, sectors: &[u8], marks: &[i8]) -> Result<(Vec<u8>, [u8; 768]), i32> {
    if sectors.len() < 540 || marks.len() < 60 {
        return Err(crate::E_ARG);
    }
    let (m, d) = (map_data(iso, exe)?, panel::panel_data(iso, exe)?);
    let mut scr = vec![0u8; SCREEN];
    for y in 0..146 {
        scr[y * SW..(y + 1) * SW].copy_from_slice(&m.buf[y * 640 + 320..y * 640 + 640]);
    }
    for s in 0..60usize {
        sector(&m, &d.sheet, &sectors[s * 9..s * 9 + 9], marks[s], s, &mut scr);
    }
    Ok((scr, m.palette))
}

/// strategic.md "Mouse over the island map": the sector under the mouse at (x, y), -1 when y >= 141. Division
/// rounds toward zero, the sum is a signed byte, more than 59 gives 59 (the x = 313..319 quirk is kept, P04 fixes it).
pub fn sector_at(x: i32, y: i32) -> i32 {
    if y >= 141 {
        return -1;
    }
    // Port fix P04 (#71, #2 row 16): the column is capped at 9 (x = 313..319 gave 10 = the next row's first sector).
    let col = if ja1logic::fanfix::port_on(4) { ((x - 3) / 31).min(9) } else { (x - 3) / 31 };
    let s = (((y - 3) / 23) * 10 + col) as i8 as i32;
    if s > 59 {
        59
    } else {
        s
    }
}

/// The frame of `sector` (0..59) of `colour` on `screen` (320 x 200, in/out): panel.md Frame with x0 = (s mod 10)
/// 31 + 5, y0 = (s div 10) 23 + 3, x1 = x0 + 31, y1 = y0 + 23.
pub fn sector_frame(iso: &str, exe: &str, sector: i32, colour: u8, screen: &mut [u8]) -> Result<(), i32> {
    if !(0..60).contains(&sector) || screen.len() < SCREEN {
        return Err(crate::E_ARG);
    }
    let (c, d) = (crate::common(iso)?, panel::panel_data(iso, exe)?);
    let mut table0 = c.remap[0];
    let mut p = panel::Painter { d: &d, buf: &mut screen[..SCREEN], table0: &mut table0, table29: panel::table29(&c) };
    let (x0, y0) = ((sector % 10) * 31 + 5, (sector / 10) * 23 + 3);
    p.frame(x0, y0, x0 + 31, y0 + 23, colour);
    Ok(())
}

#[cfg(test)]
mod arg_tests {
    use super::*;

    /// The argument checks come before any file is read: short sector or marker tables and a short screen are E_ARG.
    #[test]
    fn short_tables_are_e_arg() {
        assert_eq!(island_cells("", "", &[0; 539], &[0; 60]).err(), Some(crate::E_ARG));
        assert_eq!(island_cells("", "", &[0; 540], &[0; 59]).err(), Some(crate::E_ARG));
        assert_eq!(island_map("", "", &[0; 539], &[0; 60]).err(), Some(crate::E_ARG));
        assert_eq!(island_map("", "", &[0; 540], &[0; 59]).err(), Some(crate::E_ARG));
        assert_eq!(sector_frame("", "", 0, 1, &mut [0; 100]), Err(crate::E_ARG));
        assert_eq!(sector_frame("", "", 60, 1, &mut vec![0; SCREEN]), Err(crate::E_ARG));
    }

    /// A marker on the 320-wide screen writes nothing past the screen's 64000 bytes, also into a longer buffer.
    #[test]
    fn a_marker_stops_at_the_screen_end() {
        let mut sheet = vec![0u8; 320 * 200];
        let (sx, sy) = elem(33);
        for r in 0..3 {
            for c in 0..3 {
                sheet[(sy + r) * 320 + sx + c] = 9;
            }
        }
        let mut scr = vec![0u8; SCREEN + 2000];
        marker(&sheet, &mut scr, SW, 33, 10, 199);
        assert_eq!(&scr[199 * 320 + 10..199 * 320 + 13], &[9, 9, 9]);
        assert!(scr[SCREEN..].iter().all(|&b| b == 0));
    }

    /// The island pictures must be 320 wide: a picture of another width (EXPLODE.PCX, 1596 x 60) is E_FILE.
    #[test]
    fn a_picture_of_another_width_is_refused() {
        let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
        let Ok(main) = std::fs::read(root.join("build/game113b/DAT/MAIN.DAT")) else { return };
        assert_eq!(pic(&main, "EXPLODE.PCX").err(), Some(crate::E_FILE));
    }
}
