//! ja1front: clean-room front-end, step 3a. Loads one tactical sector from the original files (CD image + JA.EXE)
//! and draws its static scene as docs/spec/tactical_map.md says. Library part with a C interface; the `ja1front`
//! binary (feature `gui`) shows it in an SDL2 window.
pub mod aim;
pub mod aimgrid;
pub mod tactkeys;
pub mod guardopen;
pub mod invextra;
pub mod slotmouse;
pub mod assign;
pub mod balloon;
pub mod debugkeys;
pub mod debugpage;
pub mod battlebox;
pub mod assign_items;
pub mod ctlpanel;
pub mod quicksave;
pub mod replaypic;
pub mod cursor;
pub mod data;
pub mod dayend;
pub mod confirm;
pub mod config;
pub mod setup;
pub mod drag;
pub mod dayloop;
pub mod dostime;
pub mod enter;
pub mod events;
pub mod record;
pub mod ending;
pub mod eod;
pub mod faceanim;
pub mod fm;
pub mod sunrise;
pub mod switchbox;
pub mod flc;
pub mod focus;
pub mod host;
pub mod invdraw;
pub mod invscreen;
pub mod island;
pub mod mappanel;
pub mod mapui;
pub mod menu;
pub mod mouselimit;
pub mod msgbox;
pub mod office;
pub mod iso;
pub mod iteminfo;
pub mod sanim;
pub mod tilepopup;
pub mod panel;
pub mod pause;
pub mod payroll;
pub mod popup;
pub mod question;
pub mod salary;
pub mod sab;
pub mod play;
pub mod pointer;
pub mod dosmouse;
pub mod startup;
pub mod render;
pub mod saveslots;
pub mod screen;
pub mod sighting;
pub mod wipe;
pub mod soldier;
pub mod music;
pub mod sound;
pub mod strips;
pub mod viewmode;
pub mod overview;
pub mod sprite;
pub mod offsetbank;
pub mod travprompt;
pub mod world;
pub mod xpcount;

use data::{load_palette, Common, ExeTables, Tileset};
use iso::Iso;
use render::{Scene, SceneSoldiers, MAP, MAX_COLS};
use soldier::{SoldierBank, SoldierEnv, REC};
use std::collections::HashMap;
use std::ffi::{c_char, CStr};
use std::panic::catch_unwind;
use std::sync::{Arc, Mutex, OnceLock};

// The library's C entry points (ja1_enter_sector) come from this crate.
use ja1logic as _;

pub const E_ARG: i32 = -1;
pub const E_SECTOR: i32 = -2;
pub const E_VIEW: i32 = -3;
pub const E_FILE: i32 = -4;
pub const E_PANIC: i32 = -5;

#[derive(Default)]
struct Cache {
    common: HashMap<String, Arc<Common>>,
    exe: HashMap<String, Arc<ExeTables>>,
    tilesets: HashMap<(String, u8), Arc<Tileset>>,
    soldier_bank: HashMap<String, Arc<SoldierBank>>,
    fx_sheets: HashMap<String, Arc<render::FxSheets>>,
    worlds: HashMap<String, Arc<world::World>>,
    world_files: HashMap<String, Arc<(Vec<u8>, Vec<u8>)>>,
}

fn cache() -> &'static Mutex<Cache> {
    static C: OnceLock<Mutex<Cache>> = OnceLock::new();
    C.get_or_init(|| Mutex::new(Cache::default()))
}

fn lock() -> std::sync::MutexGuard<'static, Cache> {
    cache().lock().unwrap_or_else(|e| e.into_inner())
}

pub fn common(iso_path: &str) -> Result<Arc<Common>, i32> {
    if let Some(c) = lock().common.get(iso_path) {
        return Ok(c.clone());
    }
    let mut iso = Iso::open(iso_path).map_err(|_| E_FILE)?;
    let c = Arc::new(Common::load(&mut iso).map_err(|_| E_FILE)?);
    lock().common.insert(iso_path.to_string(), c.clone());
    Ok(c)
}

/// A world loaded from a save (the `--world` path), cached: the diff tests call it once per case.
pub fn world(iso_path: &str, exe_path: &str, save_path: &str) -> Result<Arc<world::World>, i32> {
    let key = format!("{}\u{0}{}\u{0}{}", iso_path, exe_path, save_path);
    if let Some(w) = lock().worlds.get(&key) {
        return Ok(w.clone());
    }
    let w = Arc::new(world::World::load(iso_path, exe_path, save_path).map_err(|_| E_FILE)?);
    lock().worlds.insert(key, w.clone());
    Ok(w)
}

/// JA.EXE and DAT/JAGGED.DAT, cached: the day-save load test feeds many saves through the same files.
fn world_files(iso_path: &str, exe_path: &str) -> Result<Arc<(Vec<u8>, Vec<u8>)>, i32> {
    let key = format!("{}\u{0}{}", iso_path, exe_path);
    if let Some(f) = lock().world_files.get(&key) {
        return Ok(f.clone());
    }
    let exe = std::fs::read(exe_path).map_err(|_| E_FILE)?;
    let jagged = Iso::open(iso_path).and_then(|mut i| i.read("DAT/JAGGED.DAT")).map_err(|_| E_FILE)?;
    let f = Arc::new((exe, jagged));
    lock().world_files.insert(key, f.clone());
    Ok(f)
}

pub fn soldier_bank(iso_path: &str) -> Result<Arc<SoldierBank>, i32> {
    if let Some(c) = lock().soldier_bank.get(iso_path) {
        return Ok(c.clone());
    }
    let mut iso = Iso::open(iso_path).map_err(|_| E_FILE)?;
    let b = Arc::new(SoldierBank::load(&mut iso).map_err(|_| E_FILE)?);
    lock().soldier_bank.insert(iso_path.to_string(), b.clone());
    Ok(b)
}

/// The effect pictures (combat.md "Drawing the effect records") of the MAIN.DAT beside JA.EXE.
pub fn fx_sheets(exe_path: &str) -> Result<Arc<render::FxSheets>, i32> {
    if let Some(c) = lock().fx_sheets.get(exe_path) {
        return Ok(c.clone());
    }
    let f = Arc::new(render::FxSheets::load(exe_path).map_err(|_| E_FILE)?);
    lock().fx_sheets.insert(exe_path.to_string(), f.clone());
    Ok(f)
}

pub fn exe_tables(exe_path: &str) -> Result<Arc<ExeTables>, i32> {
    if let Some(c) = lock().exe.get(exe_path) {
        return Ok(c.clone());
    }
    let e = Arc::new(ExeTables::load(exe_path).map_err(|_| E_FILE)?);
    lock().exe.insert(exe_path.to_string(), e.clone());
    Ok(e)
}

pub fn tileset_data(iso_path: &str, tileset: u8) -> Result<Arc<Tileset>, i32> {
    let key = (iso_path.to_string(), tileset);
    if let Some(t) = lock().tilesets.get(&key) {
        return Ok(t.clone());
    }
    let c = common(iso_path)?;
    let mut iso = Iso::open(iso_path).map_err(|_| E_FILE)?;
    let t = Arc::new(Tileset::load(&mut iso, &c, tileset).map_err(|_| E_FILE)?);
    lock().tilesets.insert(key, t.clone());
    Ok(t)
}

pub fn sector_tileset(iso_path: &str, sector: i32) -> Result<u8, i32> {
    if !(1..=60).contains(&sector) {
        return Err(E_SECTOR);
    }
    Ok(common(iso_path)?.tileset(sector as usize))
}

/// The 8-bit RGB palette of a tileset (0..9).
pub fn palette(iso_path: &str, tileset: i32) -> Result<[u8; 768], i32> {
    if !(0..=9).contains(&tileset) {
        return Err(E_ARG);
    }
    if let Some(t) = lock().tilesets.get(&(iso_path.to_string(), tileset as u8)) {
        return Ok(t.palette);
    }
    let mut iso = Iso::open(iso_path).map_err(|_| E_FILE)?;
    load_palette(&mut iso, tileset as u8).map_err(|_| E_FILE)
}

/// The view with top-left tile (c0, r0): 16*cols x 14*rows palette indexes, row-major.
pub fn render_view(iso_path: &str, exe_path: &str, sector: i32, c0: i32, r0: i32, cols: i32, rows: i32) -> Result<Vec<u8>, i32> {
    if !(1..=60).contains(&sector) {
        return Err(E_SECTOR);
    }
    // SPEC GAP: the spec only describes a 14 x 12 window inside the map. Accepted here: any window that lies on the
    // map and fits the 384 x 854 back buffer (cols 1..22, rows 1..56).
    if cols < 1 || cols > MAX_COLS || rows < 1 || rows > MAP || c0 < 0 || r0 < 0 || c0 + cols > MAP || r0 + rows > MAP {
        return Err(E_VIEW);
    }
    let c = common(iso_path)?;
    let e = exe_tables(exe_path)?;
    let t = tileset_data(iso_path, c.tileset(sector as usize))?;
    let scene = Scene { common: &c, exe: &e, ts: &t, sector: sector as usize, c0, r0, cols, rows, tilemap: None, soldiers: None, cursor: None, lines: Vec::new(), portraits: None, fx: None, shadows: true, roofs: crate::debugkeys::roofs() != 0, dark: None, ground: None };
    Ok(scene.window())
}

fn check_view(c0: i32, r0: i32, cols: i32, rows: i32) -> Result<(), i32> {
    if cols < 1 || cols > MAX_COLS || rows < 1 || rows > MAP || c0 < 0 || r0 < 0 || c0 + cols > MAP || r0 + rows > MAP {
        return Err(E_VIEW);
    }
    Ok(())
}

/// C layout of the test interface scene (tactical_map.md "Front-end test interface").
#[repr(C)]
pub struct Ja1fScene {
    pub tilemap: *const u8,
    pub soldiers: *mut u8,
    pub script_words: *const u16,
    pub seen: *const u8,
    pub show_all: u8,
    pub viewer: u8,
    pub selected: u8,
    pub shade: u8,
    /// 200 x 12 bytes or null (no ground items drawn).
    pub ground_items: *const u8,
}

/// # Safety
/// The scene pointers must be valid: tilemap 3136 * 8 bytes, soldiers 48 * 512 bytes, script_words 48 u16,
/// seen 64 * 512 bytes, ground_items null or 200 * 12 bytes.
unsafe fn render_scene(iso_path: &str, exe_path: &str, tileset: i32, sc: &Ja1fScene, c0: i32, r0: i32, cols: i32, rows: i32) -> Result<Vec<u8>, i32> {
    if !(0..=9).contains(&tileset) {
        return Err(E_SECTOR);
    }
    check_view(c0, r0, cols, rows)?;
    if sc.tilemap.is_null() || sc.soldiers.is_null() || sc.script_words.is_null() || sc.seen.is_null() {
        return Err(E_ARG);
    }
    let c = common(iso_path)?;
    let e = exe_tables(exe_path)?;
    let t = tileset_data(iso_path, tileset as u8)?;
    let bank = soldier_bank(iso_path)?;
    let tilemap = std::slice::from_raw_parts(sc.tilemap, MAP as usize * MAP as usize * 8);
    let records = std::slice::from_raw_parts_mut(sc.soldiers, 48 * REC);
    let words = std::slice::from_raw_parts(sc.script_words, 48);
    let seen = std::slice::from_raw_parts(sc.seen, 64 * 512);
    let ground = if sc.ground_items.is_null() { None } else { Some(std::slice::from_raw_parts(sc.ground_items, 2400)) };
    // Tables as the static scene uses them: table 28 is the tileset's (LST overrides); 29 already has its overrides.
    let mut remap = c.remap.clone();
    remap[28] = t.table28;
    let env = SoldierEnv { bank: &bank, remap: &remap, seen, show_all: sc.show_all != 0, viewer: sc.viewer, selected: sc.selected, shade: sc.shade };
    let soldiers = SceneSoldiers { env, records: std::cell::RefCell::new(records), script_words: words, texts: None };
    let scene = Scene { common: &c, exe: &e, ts: &t, sector: 1, c0, r0, cols, rows, tilemap: Some(tilemap), soldiers: Some(soldiers), cursor: None, lines: Vec::new(), portraits: None, fx: None, shadows: true, roofs: crate::debugkeys::roofs() != 0, dark: None, ground };
    Ok(scene.window())
}

/// # Safety
/// `iso_path`, `exe_path`: NUL-terminated paths; `scene`: valid Ja1fScene; `out`: room for 16*cols * 14*rows bytes.
#[no_mangle]
pub unsafe extern "C" fn ja1f_render_scene(
    iso_path: *const c_char,
    exe_path: *const c_char,
    tileset: i32,
    scene: *const Ja1fScene,
    c0: i32,
    r0: i32,
    cols: i32,
    rows: i32,
    out: *mut u8,
) -> i32 {
    let (Some(iso), Some(exe)) = (cstr(iso_path), cstr(exe_path)) else { return E_ARG };
    if out.is_null() || scene.is_null() {
        return E_ARG;
    }
    let (out, scene) = (out as usize, scene as usize);
    guard(move || match render_scene(iso, exe, tileset, &*(scene as *const Ja1fScene), c0, r0, cols, rows) {
        Ok(v) => {
            std::ptr::copy_nonoverlapping(v.as_ptr(), out as *mut u8, v.len());
            0
        }
        Err(e) => e,
    })
}

/// Test interface of cursor.md "Hover": `n` hover steps at mouse tile `tile` on the library world `w` (host `h`),
/// with gv_t3 from JA.EXE. `st` and `hs` are read and written back. The original is UseItemAt 0x1D9B8.
///
/// # Safety
/// `w` / `h` valid library world and host; `st`, `hs` valid; `exe_path` NUL-terminated.
#[no_mangle]
#[allow(clippy::too_many_arguments)]
pub unsafe extern "C" fn ja1f_hover(
    w: *mut host::Ja1World,
    h: *const host::Ja1Host,
    exe_path: *const c_char,
    st: *mut cursor::CursorState,
    hs: *mut cursor::HoverState,
    tile: i32,
    shift: i32,
    alt: i32,
    n: i32,
) -> i32 {
    let Some(exe) = cstr(exe_path) else { return E_ARG };
    if w.is_null() || h.is_null() || st.is_null() || hs.is_null() {
        return E_ARG;
    }
    let (wp, hp, sp, hsp) = (w as usize, h as usize, st as usize, hs as usize);
    guard(move || {
        let e = match exe_tables(exe) {
            Ok(e) => e,
            Err(c) => return c,
        };
        let mut hv = cursor::Hover {
            w: wp as *mut host::Ja1World,
            h: hp as *const host::Ja1Host,
            st: &mut *(sp as *mut cursor::CursorState),
            hs: &mut *(hsp as *mut cursor::HoverState),
            gv_t3: &e.gv_t3,
            shift: shift != 0,
            alt: alt != 0,
            confirm_move: false,
            fast_fire: false,
            drag: 0,
            squad: 0,
        };
        for _ in 0..n {
            hv.step(tile);
        }
        0
    })
}

/// Test entry for validate/diff/click_diff.py: `Hover::press_walks` (1 = this left press reaches AllMoveTraverse?).
#[no_mangle]
pub unsafe extern "C" fn ja1f_press_walks(
    w: *mut host::Ja1World,
    h: *const host::Ja1Host,
    exe_path: *const c_char,
    st: *mut cursor::CursorState,
    hs: *mut cursor::HoverState,
    tile: i32,
    shift: i32,
) -> i32 {
    let Some(exe) = cstr(exe_path) else { return E_ARG };
    if w.is_null() || h.is_null() || st.is_null() || hs.is_null() {
        return E_ARG;
    }
    let (wp, hp, sp, hsp) = (w as usize, h as usize, st as usize, hs as usize);
    guard(move || {
        let e = match exe_tables(exe) {
            Ok(e) => e,
            Err(c) => return c,
        };
        let hv = cursor::Hover {
            w: wp as *mut host::Ja1World,
            h: hp as *const host::Ja1Host,
            st: &mut *(sp as *mut cursor::CursorState),
            hs: &mut *(hsp as *mut cursor::HoverState),
            gv_t3: &e.gv_t3,
            shift: shift != 0,
            alt: false,
            confirm_move: false,
            fast_fire: false,
            drag: 0,
            squad: 0,
        };
        hv.press_walks(tile) as i32
    })
}

/// Test interface of cursor.md "Clicks": one click of `button` (1 left, 2 right) at `tile`; an open wait (aim or
/// confirm) gets a left click on `tile` at once. The library calls are not run: each one is written to `log` (8
/// int32 per call: kind 1 click / 2 reload / 3 select / 4 move, then its arguments; at most 8 calls, count in
/// `log_n`) and answered from `rets` (click, reload, select; a move gives 0). `status` gets the status line text
/// (80 bytes, NUL-ended, "" for none), `wait` the wait kind (0 none, 1 aim, 2 confirm, 3 walk, 4 posture).
/// A right press is a hold (cursor.md "Clicks", right step 3): `polls` = count n, then n pairs (buttons: 1 left,
/// 2 right; mouse y), the press at y 100 (null: one poll with both buttons up); a posture confirm gets the scripted
/// answer; then kinds 5 turn (slot, d, ticks; answered from rets[5]), 6 set anim (slot, code, keep, reset), 7 look, 8 merc message (slot, quote, near).
///
/// # Safety
/// As ja1f_hover (the original is UseItemAt 0x1D9B8, the click path); `rets` 5 int32 (7 with `polls`: rets[6] = the Fast Firing switch), `log` 64 int32, `log_n` 1 int32, `status` 80 bytes, `wait` 1
/// int32, `polls` null or 1 + 2n int32.
#[no_mangle]
#[allow(clippy::too_many_arguments)]
pub unsafe extern "C" fn ja1f_click(
    w: *mut host::Ja1World,
    h: *const host::Ja1Host,
    exe_path: *const c_char,
    st: *mut cursor::CursorState,
    hs: *mut cursor::HoverState,
    tile: i32,
    button: i32,
    shift: i32,
    alt: i32,
    rets: *const i32,
    log: *mut i32,
    log_n: *mut i32,
    status: *mut u8,
    wait: *mut i32,
    polls: *const i32,
    aim: *const i32,
) -> i32 {
    let Some(exe) = cstr(exe_path) else { return E_ARG };
    if w.is_null() || h.is_null() || st.is_null() || hs.is_null() || rets.is_null() || log.is_null()
        || log_n.is_null() || status.is_null() || wait.is_null() {
        return E_ARG;
    }
    let (wp, hp, sp, hsp) = (w as usize, h as usize, st as usize, hs as usize);
    let r = [*rets, *rets.add(1), *rets.add(2), if polls.is_null() { 0 } else { *rets.add(5) }];
    let (answer, confirm_move) = (*rets.add(3), *rets.add(4));
    let fast_fire = if polls.is_null() { 0 } else { *rets.add(6) };
    let pl: Vec<(i32, i32)> = if polls.is_null() {
        vec![(0, 100)]
    } else {
        (0..*polls as usize).map(|k| (*polls.add(1 + 2 * k), *polls.add(2 + 2 * k))).collect()
    };
    // AimLoop 0x1D744: `aim` = count n, then n pairs (buttons, key) per poll; a key is read before the buttons.
    let aim_script: Vec<cursor::Answer> = if aim.is_null() {
        Vec::new()
    } else {
        (0..*aim as usize).filter_map(|k| match (*aim.add(1 + 2 * k), *aim.add(2 + 2 * k)) {
            (_, 27) => Some(cursor::Answer::Escape),
            (b, _) if b & 1 != 0 => Some(cursor::Answer::Left(tile)),
            (b, _) if b & 2 != 0 => Some(cursor::Answer::Right),
            _ => None,
        }).collect()
    };
    let (lp, np, tp, wtp) = (log as usize, log_n as usize, status as usize, wait as usize);
    guard(move || {
        let e = match exe_tables(exe) {
            Ok(e) => e,
            Err(c) => return c,
        };
        let calls = std::rc::Rc::new(std::cell::RefCell::new(Vec::new()));
        let c2 = calls.clone();
        cursor::LIB_HOOK.with(|k| {
            *k.borrow_mut() = Some(Box::new(move |c: cursor::LibCall| {
                use cursor::LibCall::*;
                let (row, ret) = match c {
                    Click { slot, tile, item } => ([1, slot, tile, item, 0, 0, 0, 0], r[0]),
                    ClickDetector { slot, tile } => ([1, slot, tile, 0x4D, 0, 0, 0, 0], r[0]),
                    Reload { slot } => ([2, slot, 0, 0, 0, 0, 0, 0], r[1]),
                    Select { slot, force, quote } => {
                        if r[2] != 0 {
                            // As the library: a selection that works makes the merc the selected one.
                            (*(wp as *mut host::Ja1World)).selected = slot as u8;
                        }
                        ([3, slot, force, quote, 0, 0, 0, 0], r[2])
                    }
                    // As the original test's record: a move order counts when its gates pass.
                    Move { slot, tile, .. } if cursor::move_gate(wp as *mut host::Ja1World, hp as *const host::Ja1Host,
                                                                 slot, tile) == 0 => return 0,
                    Move { slot, tile, marker, sneak, b177, alt, walk, .. } =>
                        ([4, slot, tile, marker, sneak, alt, walk.unwrap_or(-1), b177], 0),
                    // The gates run in the library (they change nothing).
                    Gate { slot, tile } => return cursor::move_gate(wp as *mut host::Ja1World, hp as *const host::Ja1Host, slot, tile),
                    Turn { slot, d, ticks } => ([5, slot, d, ticks, 0, 0, 0, 0], r[3]),
                    Anim { slot, code, keep, reset } => ([6, slot, code, keep, reset, 0, 0, 0], 0),
                    Message { slot, quote, near } => ([8, slot, quote, near, 0, 0, 0, 0], 0),
                    Effect { slot, kind } => ([10, slot, kind, 0, 0, 0, 0, 0], 0),
                    CursorBack => ([11, 0, 0, 0, 0, 0, 0, 0], 0),
                };
                c2.borrow_mut().push(row);
                ret
            }))
        });
        let mut hv = cursor::Hover {
            w: wp as *mut host::Ja1World,
            h: hp as *const host::Ja1Host,
            st: &mut *(sp as *mut cursor::CursorState),
            hs: &mut *(hsp as *mut cursor::HoverState),
            gv_t3: &e.gv_t3,
            shift: shift != 0,
            alt: alt != 0,
            confirm_move: confirm_move != 0,
            fast_fire: fast_fire != 0,
            drag: 0,
            squad: 0,
        };
        let scripted = |a: i32| match a {
            1 => cursor::Answer::Escape,
            2 => cursor::Answer::Right,
            3 => cursor::Answer::Left(tile + 1),
            _ => cursor::Answer::Left(tile),
        };
        let mut out = if button == 2 { hv.right(tile, 100) } else { hv.left(tile) };
        let mut kind = 0;
        if button == 2 {
            let mut k = 0;
            let mut last = (0, 100);
            let mut look = false;
            while let Some(wt) = out.wait {
                let a = match wt {
                    cursor::ClickWait::Hold(h) if h.check => cursor::Answer::Poll { left: false, right: last.0 & 2 != 0, y: last.1 },
                    cursor::ClickWait::Hold(_) => {
                        // (past the list: both buttons up)
                        last = pl.get(k).copied().unwrap_or((0, last.1));
                        k += 1;
                        cursor::Answer::Poll { left: last.0 & 1 != 0, right: last.0 & 2 != 0, y: last.1 }
                    }
                    cursor::ClickWait::Posture(_) => {
                        kind = 4;
                        scripted(answer)
                    }
                    _ => break,
                };
                let o2 = hv.answer(wt, a);
                look |= o2.look;
                out = cursor::ClickOut { status: out.status.or(o2.status), ..o2 };
            }
            if look {
                calls.borrow_mut().push([7, 0, 0, 0, 0, 0, 0, 0]);
            }
        }
        if kind == 0 {
            kind = match out.wait {
                Some(cursor::ClickWait::Aim { .. }) => 1,
                Some(cursor::ClickWait::Confirm { .. }) => 2,
                Some(cursor::ClickWait::Walk { .. }) => 3,
                _ => 0,
            };
        }
        if let Some(wt) = out.wait {
            // The aim always fires (its button is still down); a confirm gets the scripted answer.
            let a = if kind == 1 { cursor::Answer::Left(tile) } else { scripted(answer) };
            let mut wt = wt;
            let mut last = a;
            if kind == 1 {
                for &step in &aim_script {
                    if step != cursor::Answer::Right {
                        last = step;
                        break;
                    }
                    let o2 = hv.answer(wt, step);
                    wt = o2.wait.unwrap_or(wt);
                    calls.borrow_mut().push([9, hv.st.ap, hv.st.ap_fg as i32, hv.st.ap_shadow as i32, 0, 0, 0, 0]);
                }
            }
            let o2 = hv.answer(wt, last);
            out.status = out.status.or(o2.status);
        }
        cursor::LIB_HOOK.with(|k| *k.borrow_mut() = None);
        let calls = calls.borrow();
        for (i, row) in calls.iter().take(8).enumerate() {
            for (j, v) in row.iter().enumerate() {
                *(lp as *mut i32).add(8 * i + j) = *v;
            }
        }
        *(np as *mut i32) = calls.len() as i32;
        *(wtp as *mut i32) = kind;
        let text = out.status.unwrap_or_default();
        let b = text.as_bytes();
        let n = b.len().min(79);
        std::ptr::copy_nonoverlapping(b.as_ptr(), tp as *mut u8, n);
        *(tp as *mut u8).add(n) = 0;
        0
    })
}

/// Test interface of cursor.md "Drawing": the target mark, then the cursor pass, into `back` (384 x 854, in and
/// out) with a tile map (3136 x 8 bytes, for the occupant), remap table 0 (256 bytes, in and out) and the view in
/// pixels (x, y, w, h).
/// `st` is read and written back.
///
/// # Safety
/// All pointers valid with the sizes above; paths NUL-terminated.
#[no_mangle]
pub unsafe extern "C" fn ja1f_cursor_draw(
    iso_path: *const c_char,
    exe_path: *const c_char,
    tileset: i32,
    tilemap: *const u8,
    soldiers: *const u8,
    table0: *mut u8,
    view: *const i32,
    st: *mut cursor::CursorState,
    back: *mut u8,
) -> i32 {
    let (Some(iso), Some(exe)) = (cstr(iso_path), cstr(exe_path)) else { return E_ARG };
    if tilemap.is_null() || soldiers.is_null() || table0.is_null() || view.is_null() || st.is_null() || back.is_null() || !(0..=9).contains(&tileset) {
        return E_ARG;
    }
    let (tilemap, soldiers, table0, view, st, back) =
        (tilemap as usize, soldiers as usize, table0 as usize, view as usize, st as usize, back as usize);
    guard(move || {
        let run = || -> Result<(), i32> {
            let e = exe_tables(exe)?;
            let t = tileset_data(iso, tileset as u8)?;
            let pd = panel::panel_data(iso, exe)?;
            let tm = std::slice::from_raw_parts(tilemap as *const u8, 3136 * 8);
            let v = std::slice::from_raw_parts(view as *const i32, 4);
            let st = &mut *(st as *mut cursor::CursorState);
            let buf = std::slice::from_raw_parts_mut(back as *mut u8, (render::PITCH * render::HEIGHT) as usize);
            let mut t0 = [0u8; 256];
            t0.copy_from_slice(std::slice::from_raw_parts(table0 as *const u8, 256));
            let mut env = cursor::CursorEnv { table: &e.cursor, font0: &pd.fonts[0], table0: t0, view_x: v[0], view_y: v[1], view_w: v[2], view_h: v[3] };
            // SPEC GAP: a mouse tile off the map has no occupant; taken as 0x40 (none).
            let occ = if (0..3136).contains(&st.tile) { tm[st.tile as usize * 8 + 5] } else { 0x40 };
            let recs = std::slice::from_raw_parts(soldiers as *const u8, 48 * 512);
            let cc = if ((occ & 0x7F) as usize) < 48 { recs[(occ & 0x7F) as usize * 512 + 0x9C] } else { 0 };
            cursor::target_mark(buf, &t.sprites, &mut env, st, occ, cc);
            cursor::cursor_pass(buf, &t.sprites, &mut env, st, Some((recs, tm)));
            std::slice::from_raw_parts_mut(table0 as *mut u8, 256).copy_from_slice(&env.table0);
            Ok(())
        };
        match run() {
            Ok(()) => 0,
            Err(e) => e,
        }
    })
}

unsafe fn cstr<'a>(p: *const c_char) -> Option<&'a str> {
    if p.is_null() {
        return None;
    }
    CStr::from_ptr(p).to_str().ok()
}

fn guard(f: impl FnOnce() -> i32 + std::panic::UnwindSafe) -> i32 {
    catch_unwind(f).unwrap_or(E_PANIC)
}

/// cursor.md "Debug keys (level 4)" debug pages: writes one line per text, "P X Y TEXT\n" (P = page 1..4, after
/// a line "P redraw map|screen"), for record `rec` (512 bytes) into `out` (NUL-terminated). Returns the length or
/// -1 when `out` is too small.
///
/// # Safety
/// `rec`: 512 bytes; `out`: room for `cap` bytes.
#[no_mangle]
pub unsafe extern "C" fn ja1f_debug_pages(rec: *const u8, call_m_tile: i32, call_m_level: i32, call_e_tile: i32,
                                          call_e_level: i32, enemies: i32, script_word: i32, out: *mut u8, cap: i32) -> i32 {
    let r = std::slice::from_raw_parts(rec, 512);
    let o = debugpage::Outside { call_m: (call_m_tile as i16, call_m_level as u8), call_e: (call_e_tile as i16, call_e_level as u8),
                                 enemies: enemies as u8, script_word: script_word as u16 };
    let mut t = String::new();
    for (k, p) in debugpage::pages(r, &o).iter().enumerate() {
        t += &format!("{} redraw {}\n", k + 1, if p.map_view { "map" } else { "screen" });
        for (x, y, s) in debugpage::texts(p) {
            t += &format!("{} {} {} {}\n", k + 1, x, y, String::from_utf8_lossy(&s));
        }
    }
    write_out(&t, out, cap)
}

/// cursor.md "Opponent lists (Ctrl+O)": the messages, one per line.
///
/// # Safety
/// `rec`: 512 bytes; `knowledge`: 48; `rem_m`: 128; `rem_e`: 320; `out`: room for `cap` bytes.
#[no_mangle]
pub unsafe extern "C" fn ja1f_opp_lists(rec: *const u8, knowledge: *const u8, rem_m: *const u8, rem_e: *const u8,
                                        out: *mut u8, cap: i32) -> i32 {
    let l = debugpage::opp_lists(std::slice::from_raw_parts(rec, 512), std::slice::from_raw_parts(knowledge, 48),
                                 std::slice::from_raw_parts(rem_m, 128), std::slice::from_raw_parts(rem_e, 320));
    write_out(&(l.join("\n") + "\n"), out, cap)
}

unsafe fn write_out(t: &str, out: *mut u8, cap: i32) -> i32 {
    if t.len() + 1 > cap.max(0) as usize {
        return -1;
    }
    std::ptr::copy_nonoverlapping(t.as_ptr(), out, t.len());
    *out.add(t.len()) = 0;
    t.len() as i32
}

/// The tmp file of op 219 save_tmp (SaveSectorTmp 0x2F800) from `tiles` (3136 x 8 bytes) and `ground` (0x960 bytes)
/// into `out` (room for 18,080 bytes); returns the length.
///
/// # Safety
/// The pointers must hold the sizes above.
#[no_mangle]
pub unsafe extern "C" fn ja1f_tmp_file(tiles: *const u8, ground: *const u8, out: *mut u8) -> i32 {
    let f = host::make_tmp_file(std::slice::from_raw_parts(tiles, 3136 * 8), std::slice::from_raw_parts(ground, 0x960));
    std::ptr::copy_nonoverlapping(f.as_ptr(), out, f.len());
    f.len() as i32
}

/// ClearItemStash? 0x515DC on a store of `n` records (type, quantity, condition) in `records` with its 136 `counts`;
/// returns the records left.
///
/// # Safety
/// `counts`: 136 bytes, `records`: 3 x `n` bytes.
#[no_mangle]
pub unsafe extern "C" fn ja1f_stash_clear(counts: *mut u8, records: *mut u8, n: i32) -> i32 {
    let counts = std::slice::from_raw_parts_mut(counts, 136);
    let flat = std::slice::from_raw_parts_mut(records, 3 * n.max(0) as usize);
    let mut recs: Vec<[u8; 3]> = flat.chunks(3).map(|c| [c[0], c[1], c[2]]).collect();
    world::clear_stash(counts, &mut recs);
    for (k, r) in recs.iter().enumerate() {
        flat[3 * k..3 * k + 3].copy_from_slice(r);
    }
    recs.len() as i32
}

/// LoadItemStash 0x51508 on a save's stash tail (`tail` = 136 counts, then `counts[i]` 3-byte records for each
/// type i): writes the counts after the load to `counts_out` (136 bytes) and the store's records to
/// `records_out` (3 x `cap` bytes); returns the record count, or -1 when `cap` is too small.
///
/// # Safety
/// `tail`: `len` bytes; `counts_out`: 136 bytes; `records_out`: 3 x `cap` bytes.
#[no_mangle]
pub unsafe extern "C" fn ja1f_stash_load(tail: *const u8, len: usize, counts_out: *mut u8,
                                         records_out: *mut u8, cap: i32) -> i32 {
    let tail = std::slice::from_raw_parts(tail, len);
    let counts = std::slice::from_raw_parts_mut(counts_out, 136);
    let mut recs: Vec<[u8; 3]> = Vec::new();
    world::read_stash(tail, counts, &mut recs);
    if recs.len() > cap.max(0) as usize {
        return -1;
    }
    for (k, r) in recs.iter().enumerate() {
        std::ptr::copy_nonoverlapping(r.as_ptr(), records_out.add(3 * k), 3);
    }
    recs.len() as i32
}

/// SaveItemStash 0x51590: writes the 136 `counts` and `n` 3-byte `records` to `out` (136 + 3 x `n` bytes);
/// returns the byte count, or -1 when `cap` is too small.
///
/// # Safety
/// `counts`: 136 bytes; `records`: 3 x `n` bytes; `out`: `cap` bytes.
#[no_mangle]
pub unsafe extern "C" fn ja1f_stash_save(counts: *const u8, records: *const u8, n: i32,
                                         out: *mut u8, cap: usize) -> i32 {
    let counts = std::slice::from_raw_parts(counts, 136);
    let flat = std::slice::from_raw_parts(records, 3 * n.max(0) as usize);
    let recs: Vec<[u8; 3]> = flat.chunks(3).map(|c| [c[0], c[1], c[2]]).collect();
    let b = world::write_stash(counts, &recs);
    if b.len() > cap {
        return -1;
    }
    std::ptr::copy_nonoverlapping(b.as_ptr(), out, b.len());
    b.len() as i32
}

/// ResetSectorsFromDat 0x3712C: rewrites `sectors` (60 records of 177 bytes) from `jagged` (JAGGED.DAT);
/// returns 0, or -1 when JAGGED.DAT is too short.
///
/// # Safety
/// `jagged`: `len` bytes; `sectors`: 60 x 177 bytes.
#[no_mangle]
pub unsafe extern "C" fn ja1f_sector_reset(jagged: *const u8, len: usize, sectors: *mut u8) -> i32 {
    let jagged = std::slice::from_raw_parts(jagged, len);
    let sectors = std::slice::from_raw_parts_mut(sectors, 60 * 177);
    match world::reset_sectors(jagged, sectors) {
        Ok(()) => 0,
        Err(_) => -1,
    }
}

/// Random 0x53538: `draws` calls of Random(n) from `seed` over the front-end's Watcom rand (host::Rng);
/// returns the last value and writes the final seed to `final_seed`.
///
/// # Safety
/// `final_seed` must point to a u32.
#[no_mangle]
pub unsafe extern "C" fn ja1f_random(seed: u32, n: i32, draws: u32, final_seed: *mut u32) -> i32 {
    let mut r = host::Rng::new(seed);
    let mut v = 0;
    for _ in 0..draws {
        v = r.random(n);
    }
    if !final_seed.is_null() {
        *final_seed = r.seed;
    }
    v
}

/// The header of a film (FLC `data`, `len` bytes) as FlcOpen 0x5BB30 reads it: `out` = ms per frame, the place of the
/// first frame, the frames. Returns 0, or -5 = not a film.
///
/// # Safety
/// `data` holds `len` bytes, `out` 3 numbers.
#[no_mangle]
pub unsafe extern "C" fn ja1f_flc_head(data: *const u8, len: i32, out: *mut i32) -> i32 {
    let data = std::slice::from_raw_parts(data, len as usize);
    let Some(f) = flc::Flc::new(data) else { return -5 };
    let out = std::slice::from_raw_parts_mut(out, 3);
    out[0] = f.speed_ms as i32;
    out[1] = f.pos() as i32;
    out[2] = f.frames as i32;
    0
}

/// A film (FLC `data`, `len` bytes) played at (x, y) on `screen` (320 x 200) with the DAC copy `dac` (4 bytes per
/// colour; changed only with `flag` != 0), every frame and then the ring frame: after each frame its screen + DAC
/// (65,024 bytes) go to `out` (room for `max` frames). Returns the frames written; -1 = not a film.
///
/// # Safety
/// The pointers must hold the sizes above.
#[no_mangle]
pub unsafe extern "C" fn ja1f_flc(data: *const u8, len: i32, x: i32, y: i32, flag: i32, screen: *mut u8, dac: *mut u8,
                                  out: *mut u8, max: i32) -> i32 {
    let data = std::slice::from_raw_parts(data, len as usize);
    let (screen, dac) = (std::slice::from_raw_parts_mut(screen, 64000), std::slice::from_raw_parts_mut(dac, 1024));
    let Some(mut f) = flc::Flc::new(data) else { return -1 };
    f.frames += 1;
    let mut n = 0;
    while n < max && f.next_frame(screen, x, y) {
        if flag != 0 {
            f.set_dac(dac);
        }
        let o = out.add(n as usize * 65024);
        std::ptr::copy_nonoverlapping(screen.as_ptr(), o, 64000);
        std::ptr::copy_nonoverlapping(dac.as_ptr(), o.add(64000), 1024);
        n += 1;
    }
    n
}

/// The test log of `ja1f_film_trace`.
struct FilmTrace<'a> {
    keys: &'a [u8],
    at: usize,
    lines: String,
    subtitles: bool,
}

impl FilmTrace<'_> {
    fn crc(s: &sab::Sab) -> (u32, u32) {
        (crc32(&s.screen), crc32(&s.dac))
    }
}

fn crc32(d: &[u8]) -> u32 {
    let mut c = !0u32;
    for &b in d {
        c ^= b as u32;
        for _ in 0..8 {
            c = if c & 1 != 0 { (c >> 1) ^ 0xEDB8_8320 } else { c >> 1 };
        }
    }
    !c
}

impl sab::SabIo for FilmTrace<'_> {
    fn file(&mut self, _name: &str) -> Option<Vec<u8>> {
        None
    }
    fn elapsed_ms(&mut self) -> i64 {
        0
    }
    fn timer_start(&mut self) {}
    fn fade(&mut self, _s: &mut sab::Sab, _pal: [u8; 1024], _steps: i32, _from: i32, _to: i32) {}
    fn film(&mut self, _s: &mut sab::Sab, _name: &str, _x: i32, _y: i32, _flag: i32, _loops: i32) {}
    fn film_wait(&mut self, _start: std::time::Instant, after_ms: u64) {
        self.lines += &format!("tick {}\n", after_ms);
    }
    fn film_key(&mut self, s: &sab::Sab, j: i32) -> Option<i8> {
        let (a, b) = Self::crc(s);
        self.lines += &format!("frame {} {:x} {:x}\n", j, a, b);
        let k = self.keys.get(self.at).copied().unwrap_or(0);
        self.at += 1;
        // 0xFF = an extended key: the first GetKey byte is 0.
        (k != 0).then_some(if k == 0xFF { 0 } else { k as i8 })
    }
    fn subtitles(&self) -> bool {
        self.subtitles
    }
    fn int_quote(&mut self, q: i32) -> Vec<u8> {
        self.lines += &format!("quote {}\n", q);
        Vec::new()
    }
    fn draw_subtitle(&mut self, _screen: &mut [u8], _text: &[u8], fg: u8, shadow: u8) {
        self.lines += &format!("text {} {}\n", fg, shadow);
    }
    fn speech(&mut self, rate: i32, vol: i32, data: &[u8]) {
        self.lines += &format!("speech {} {} 0 {:x}\n", rate, vol, crc32(data));
    }
    fn film_show(&mut self, s: &sab::Sab) {
        let (a, b) = Self::crc(s);
        self.lines += &format!("show {:x} {:x} {} {}\n", a, b, s.cue_i, s.abort as i32);
    }
    fn sfx_play(&mut self, slot: i32, rate: i32, vol: u8, loops: i32) {
        self.lines += &format!("sfx {} {} {} {}\n", slot, rate, vol, loops);
    }
    fn music(&mut self, theme: i32, stop_first: i32) {
        self.lines += &format!("music {} {}\n", theme, stop_first);
    }
}

/// flcplay_diff.py: `Sab::play_film` on film `data` with the cue table `cues` (320 bytes), the key per callback `keys`
/// (0 = none), and the start `screen` + `dac`. Writes the text log to `out` and returns its length.
///
/// # Safety
/// The pointers must hold the sizes given and the sizes above (screen 64000, dac 1024).
#[no_mangle]
#[allow(clippy::too_many_arguments)]
pub unsafe extern "C" fn ja1f_film_trace(data: *const u8, len: i32, x: i32, y: i32, flag: i32, loops: i32, film_flag: i32,
                                         rate: i32, cues: *const u8, keys: *const u8, nkeys: i32, screen: *const u8,
                                         dac: *const u8, out: *mut u8, cap: i32, voices: *const u8, nvoices: i32,
                                         subtitles: i32, vol: i32) -> i32 {
    let data = std::slice::from_raw_parts(data, len as usize);
    let mut s = sab::Sab::new();
    s.set_speech_archive(std::slice::from_raw_parts(voices, nvoices as usize).to_vec());
    s.cue_vol = vol;
    s.cues.copy_from_slice(std::slice::from_raw_parts(cues, 320));
    s.cue_rate = rate;
    s.film_flag = film_flag != 0;
    s.screen.copy_from_slice(std::slice::from_raw_parts(screen, 64000));
    s.dac.copy_from_slice(std::slice::from_raw_parts(dac, 1024));
    let mut io = FilmTrace { keys: std::slice::from_raw_parts(keys, nkeys as usize), at: 0, lines: String::new(),
                            subtitles: subtitles != 0 };
    s.play_film(&mut io, data, x, y, flag, loops);
    let n = io.lines.len().min(cap as usize);
    std::ptr::copy_nonoverlapping(io.lines.as_ptr(), out, n);
    n as i32
}

/// A volume key (sound::volume_key, TacticalKeyHandler 0x13490) on the numbers `music` / `sound` (changed in
/// place). Returns 0 = no AIL call, 1 = XMIDI master volume set, 2 = digital master volume set; `sent` = the value.
///
/// # Safety
/// The pointers must be valid.
#[no_mangle]
pub unsafe extern "C" fn ja1f_volume_key(key: u8, music: *mut i32, sound: *mut i32, sent: *mut i32) -> i32 {
    let mut v = sound::Volumes { music: *music, sound: *sound };
    let r = sound::volume_key(key, &mut v);
    *music = v.music;
    *sound = v.sound;
    match r {
        None => 0,
        Some(sound::MasterSet::Music(x)) => {
            *sent = x;
            1
        }
        Some(sound::MasterSet::Digital(x)) => {
            *sent = x;
            2
        }
    }
}

/// Pause? 0x1002C (panel.md "Pause (key P) and the clock"): pause::Pause::set on the same fields plus `paused`.
/// Result: 1 when the screen must be drawn again.
///
/// # Safety
/// The pointers must be valid.
#[no_mangle]
pub unsafe extern "C" fn ja1f_pause_set(on: i32, paused: *mut u8, round_lock: *mut u8, clock_on: *mut u8,
                                        lock_owned: *mut u8, blink: *mut u8, blink_timer: *mut i32) -> i32 {
    let mut p = pause::Pause {
        paused: *paused != 0,
        clock_on: *clock_on != 0,
        lock_owned: *lock_owned != 0,
        blink: *blink != 0,
        blink_timer: *blink_timer,
        steps: 0,
    };
    let redraw = p.set(on != 0, &mut *round_lock);
    *paused = p.paused as u8;
    *clock_on = p.clock_on as u8;
    *lock_owned = p.lock_owned as u8;
    *blink = p.blink as u8;
    *blink_timer = p.blink_timer;
    redraw as i32
}

/// puff_diff: host op 178 on the 16 effect records (`fx`, 16 x 0x12 bytes), `a` = 7 i32. Result as `play::puff`.
///
/// # Safety
/// The pointers must be valid.
#[no_mangle]
pub unsafe extern "C" fn ja1f_puff(fx: *mut u8, a: *const i32) -> i32 {
    play::puff(std::slice::from_raw_parts_mut(fx, 16 * play::FX_REC), std::slice::from_raw_parts(a, 7))
}

/// QueueQuote? 0x1B2A4 on the queue fields `q` = slot, event, focus, delay, then tile (2 bytes). `a` = the 5 arguments.
/// Returns -1, or the slot of the pending quote that played first; `q_play` gets the delay at that moment.
///
/// # Safety
/// The pointers must be valid.
#[no_mangle]
pub unsafe extern "C" fn ja1f_queue_quote(q: *mut u8, a: *const i32, q_play: *mut i32) -> i32 {
    let mut w: Box<host::Ja1World> = Box::new(std::mem::zeroed());
    w.quote_slot = *q;
    w.quote_event = *q.add(1);
    w.quote_focus = *q.add(2);
    w.quote_delay = *q.add(3) as i8;
    w.quote_tile = u16::from_le_bytes([*q.add(4), *q.add(5)]);
    let mut played = -1;
    let wp: *mut host::Ja1World = &mut *w;
    play::queue_quote(wp, std::slice::from_raw_parts(a, 5), |w, slot| {
        played = slot;
        *q_play = (*w).quote_delay as i32;
    });
    *q = w.quote_slot;
    *q.add(1) = w.quote_event;
    *q.add(2) = w.quote_focus;
    *q.add(3) = w.quote_delay as u8;
    *q.add(4) = w.quote_tile as u8;
    *q.add(5) = (w.quote_tile >> 8) as u8;
    played
}

/// AnyGameSaveExists 0x4FDB8: 1 = a file GAMEn.SAV exists in the directory `dir` (a C string).
///
/// # Safety
/// `dir` must be a valid C string.
#[no_mangle]
pub unsafe extern "C" fn ja1f_any_game_save(dir: *const std::os::raw::c_char) -> i32 {
    let d = std::ffi::CStr::from_ptr(dir).to_string_lossy().into_owned();
    menu::any_game_save(Some(std::path::Path::new(&d))) as i32
}

/// 0x11DC0 with the answers `answers` (bit 0 first question, bit 1 second). `log` (at least 200 bytes) gets for each
/// question its text, a zero, its picture, a zero. Returns 0 = go on, 1 = abort, 3 = abort + restart.
///
/// # Safety
/// `log` must be valid.
#[no_mangle]
pub unsafe extern "C" fn ja1f_ask_abort(answers: i32, log: *mut u8) -> i32 {
    let mut out = Vec::new();
    let mut k = 0;
    let r = question::ask_abort(|(text, pic)| {
        out.extend_from_slice(text);
        out.push(0);
        out.extend_from_slice(pic.as_bytes());
        out.push(0);
        k += 1;
        answers >> (k - 1) & 1 != 0
    });
    std::ptr::copy_nonoverlapping(out.as_ptr(), log, out.len().min(200));
    match r {
        None => 0,
        Some(false) => 1,
        Some(true) => 3,
    }
}

/// The table of 0x381C0: see soldier::bitmap_number.
/// Test export of ParseCmdArg 0x1008C (cmdarg_diff): the result 0 NOCRITICAL, 1 RESET, 2 ALLOWSAVE, 3 unknown; `out` gets the line.
#[no_mangle]
pub unsafe extern "C" fn ja1f_cmd_word(word: *const std::os::raw::c_char, out: *mut u8, cap: i32) -> i32 {
    let w = world::cmd_word(std::ffi::CStr::from_ptr(word).to_bytes());
    let line = world::cmd_word_line(&w);
    let n = line.len().min(cap.max(1) as usize - 1);
    std::ptr::copy_nonoverlapping(line.as_ptr(), out, n);
    *out.add(n) = 0;
    match w {
        world::CmdWord::NoCritical => 0,
        world::CmdWord::Reset => 1,
        world::CmdWord::AllowSave => 2,
        world::CmdWord::Unknown(_) => 3,
    }
}

#[no_mangle]
pub extern "C" fn ja1f_bitmap_number(f: i32, frame: i32) -> i32 {
    soldier::bitmap_number(f as u16, frame as u16) as i32
}

/// 0x50B04 on the 8 team records `soldiers`.
///
/// # Safety
/// `soldiers` must hold 8 records.
#[no_mangle]
pub unsafe extern "C" fn ja1f_hired_this_visit(soldiers: *const u8, p: i32) -> i32 {
    aim::hired_this_visit(std::slice::from_raw_parts(soldiers, 8 * 0x200), p as u8) as i32
}

/// FormatSectorDatName 0x2F7C8: the name goes to `out` (at least 32 bytes, zero end).
///
/// # Safety
/// `out` must be valid.
#[no_mangle]
pub unsafe extern "C" fn ja1f_sector_dat_name(tileset: i32, out: *mut u8) {
    let n = sound::sector_dat_name(tileset as u8);
    std::ptr::copy_nonoverlapping(n.as_ptr(), out, n.len().min(31));
    *out.add(n.len().min(31)) = 0;
}

/// 0x4C26C on the palette `pal` (768 bytes): 1 = it rotated.
///
/// # Safety
/// `pal` must be valid.
#[no_mangle]
pub unsafe extern "C" fn ja1f_office_cycle(pal: *mut u8, elapsed_ms: i32) -> i32 {
    office::cycle_palette(&mut *(pal as *mut [u8; 768]), elapsed_ms as i64) as i32
}

/// OfficeHighlight 0x4C0C8 + OfficeHighlightFrame 0x4BFFC (office::highlight_on / highlight_off / play_frames) on a
/// picture `base`. `seq` holds n triples (k, on, gap in ms before the call). Log entries of 6 words: [0, picture index,
/// 0, 0, 0, clock] a picture on the screen, [1, fx, rate, vol, loops, clock] a sound, [2, 0, 0, 0, 0, clock] the
/// palette rotated. counts = [log entries, pictures].
///
/// # Safety
/// `base` holds 64000 bytes, `pal` 768, `seq` 3n words, `log` 6 * log_cap words, `pics` 64000 * pic_cap, `counts` 2.
#[no_mangle]
pub unsafe extern "C" fn ja1f_office_highlight(exe: *const c_char, base: *mut u8, seq: *const i32, n: i32, pal: *mut u8,
                                               log: *mut i32, log_cap: i32, pics: *mut u8, pic_cap: i32,
                                               counts: *mut i32) -> i32 {
    let Some(exe) = cstr(exe) else { return E_ARG };
    if base.is_null() || seq.is_null() || n < 0 || pal.is_null() || log.is_null() || pics.is_null() || counts.is_null() {
        return E_ARG;
    }
    let (ba, sq, pa, lg, pc, ct) = (base as usize, seq as usize, pal as usize, log as usize, pics as usize, counts as usize);
    guard(move || {
        let base = std::slice::from_raw_parts_mut(ba as *mut u8, 64000);
        let seq = std::slice::from_raw_parts(sq as *const i32, 3 * n as usize);
        let pal = &mut *(pa as *mut [u8; 768]);
        let lg = std::slice::from_raw_parts_mut(lg as *mut i32, 6 * log_cap.max(0) as usize);
        let pc = std::slice::from_raw_parts_mut(pc as *mut u8, 64000 * pic_cap.max(0) as usize);
        let ct = std::slice::from_raw_parts_mut(ct as *mut i32, 2);
        let d = match office::OfficeData::load(exe) {
            Ok(d) => d,
            Err(e) => return e,
        };
        let (mut saved, mut ck) = (vec![], office::HighlightClock::default());
        let clock = std::cell::Cell::new(1000i64);
        ck.cycled = clock.get();
        let (mut nl, mut np) = (0usize, 0usize);
        let mut put = |e: [i32; 5], nl: &mut usize, t: i64| {
            if let Some(w) = lg.get_mut(6 * *nl..6 * *nl + 6) {
                w[..5].copy_from_slice(&e);
                w[5] = t as i32;
            }
            *nl += 1;
        };
        for t in seq.chunks(3) {
            clock.set(clock.get() + t[2] as i64);
            let mut p = office::Poll::default();
            let (k, on) = (t[0] as usize, t[1] != 0);
            if !(1..=7).contains(&k) {
                return E_ARG;
            }
            if on {
                office::highlight_on(&d, base, &mut saved, k, &mut p);
            } else {
                office::highlight_off(&d, base, &saved, k, &mut p);
            }
            office::play_frames(&p, pal, &mut ck, &mut || clock.get(), &mut |st, _| match st {
                office::HighlightStep::Sfx { fx, rate, vol, loops } => put([1, fx, rate, vol, loops], &mut nl, clock.get()),
                office::HighlightStep::Show(f) => {
                    if let Some(w) = pc.get_mut(64000 * np..64000 * np + 64000) {
                        w.copy_from_slice(f);
                    }
                    put([0, np as i32, 0, 0, 0], &mut nl, clock.get());
                    np += 1;
                }
                office::HighlightStep::Tick => clock.set(clock.get() + 1),
                office::HighlightStep::Cycled(_) => put([2, 0, 0, 0, 0], &mut nl, clock.get()),
            });
            if !on {
                if let Some(w) = pc.get_mut(64000 * np..64000 * np + 64000) {
                    w.copy_from_slice(base);
                }
                put([0, np as i32, 0, 0, 0], &mut nl, clock.get());
                np += 1;
            }
        }
        if nl > log_cap.max(0) as usize || np > pic_cap.max(0) as usize {
            return E_ARG;
        }
        ct[0] = nl as i32;
        ct[1] = np as i32;
        0
    })
}

/// Lock? 0x4B730 / Unlock? 0x4B764 (panel.md "Other users of Lock") on the fields they read and write. `unlock` != 0
/// runs Unlock, else Lock with `clock` (the original's AL, 0 or 1). `paused` is not used by either call.
///
/// # Safety
/// The pointers must be valid.
#[no_mangle]
pub unsafe extern "C" fn ja1f_pause_op(unlock: i32, clock: i32, round_lock: *mut u8, clock_on: *mut u8,
                                       lock_owned: *mut u8, blink: *mut u8, blink_timer: *mut i32) -> i32 {
    let mut p = pause::Pause {
        paused: false,
        clock_on: *clock_on != 0,
        lock_owned: *lock_owned != 0,
        blink: *blink != 0,
        blink_timer: *blink_timer,
        steps: 0,
    };
    if unlock != 0 {
        p.unlock(&mut *round_lock);
    } else {
        p.lock(clock != 0, &mut *round_lock);
    }
    *clock_on = p.clock_on as u8;
    *lock_owned = p.lock_owned as u8;
    *blink = p.blink as u8;
    *blink_timer = p.blink_timer;
    0
}

/// InvGradeName 0x25BDC: the item voice's grade name for `base` text, prefix `code` (2 C / 3 L / 4 U) and signed
/// count `v`; the text goes to `out` (NUL ended, at most `max` bytes). Returns the text length.
///
/// # Safety
/// `base`: NUL-terminated bytes; `out`: room for `max` bytes.
#[no_mangle]
pub unsafe extern "C" fn ja1f_inv_grade_name(base: *const u8, code: u32, v: i32, out: *mut u8, max: i32) -> i32 {
    let b = std::ffi::CStr::from_ptr(base as *const std::ffi::c_char).to_string_lossy().into_owned();
    let s = iteminfo::inv_grade_name(&b, code as u8, v as i8);
    let bytes = s.as_bytes();
    let n = bytes.len().min((max.max(1) - 1) as usize);
    std::ptr::copy_nonoverlapping(bytes.as_ptr(), out, n);
    *out.add(n) = 0;
    n as i32
}

/// ToggleMessage 0x3F994: the switch byte at `switch` flips and `word` gets " on" / " off"; the text goes to `out`
/// (NUL ended, at most `max` bytes). Returns the text length.
///
/// # Safety
/// `word`: NUL-terminated bytes; `out`: room for `max` bytes.
#[no_mangle]
pub unsafe extern "C" fn ja1f_toggle_text(word: *const u8, switch: *mut u8, out: *mut u8, max: i32) -> i32 {
    let w = std::ffi::CStr::from_ptr(word as *const std::ffi::c_char).to_string_lossy().into_owned();
    let s = play::toggle_text(&w, &mut *switch);
    let b = s.as_bytes();
    let n = b.len().min((max.max(1) - 1) as usize);
    std::ptr::copy_nonoverlapping(b.as_ptr(), out, n);
    *out.add(n) = 0;
    n as i32
}

/// Test interface of InitProgramWorld? 0x377B0 (startworld_diff): the port's program-start state. `tiles`: the map
/// (25088 B); `window`: the 8 view values of 0x229CD4, 0x229CCC, 0x229CC8, 0x229CD0, 0x229CB8, 0x229CC4, 0x229CD8,
/// 0x229CDC; `table`: 48 soldier offsets; the result = the seed for the clock `secs`.
/// # Safety
/// `iso`, `exe`: NUL-terminated paths; the buffers have the sizes above. Returns -1 and no seed when the world fails.
#[no_mangle]
pub unsafe extern "C" fn ja1f_program_start(iso: *const std::ffi::c_char, exe: *const std::ffi::c_char, secs: u64,
                                            tiles: *mut u8, window: *mut i32, table: *mut u32, seed: *mut u32) -> i32 {
    let iso = std::ffi::CStr::from_ptr(iso).to_string_lossy().into_owned();
    let exe = std::ffi::CStr::from_ptr(exe).to_string_lossy().into_owned();
    let Ok(w) = world::World::program_start(&iso, &exe) else { return -1 };
    std::ptr::copy_nonoverlapping(w.tiles.as_ptr(), tiles, w.tiles.len());
    let mut f = vec![0u8; 0x10000];
    quicksave::fixed_state(&w, &mut f);
    for k in 0..8 {
        *window.add(k) = i32::from_le_bytes(f[0xE0D2 + 4 * k..0xE0D6 + 4 * k].try_into().unwrap());
    }
    let rec = world::size::SOLDIERS / 48;
    for k in 0..48 {
        *table.add(k) = (k * rec) as u32;
    }
    *seed = world::clock_seed(secs);
    0
}

/// 0x4E93C (QuickRestore): the allow-save mode on the day save's base bytes 21285..21286; `base` holds the first
/// 21648 bytes of the save. Returns -1 when `len` is too small.
///
/// # Safety
/// `base`: room for `len` bytes.
#[no_mangle]
pub unsafe extern "C" fn ja1f_allow_save_mode(base: *mut u8, len: usize, allow: i32) -> i32 {
    if len < 21287 {
        return -1;
    }
    world::set_allow_save_mode(std::slice::from_raw_parts_mut(base, len), allow != 0);
    0
}

/// SetViewPose? 0x2B6A0: the soldier's view pose from its level byte (+0x192) and item 3 type (+0x24).
#[no_mangle]
pub extern "C" fn ja1f_view_pose(level: u8, item3: u8) -> i32 {
    ja1logic::sector::view_pose(level, item3) as i32
}

/// SetSlotView 0x332E4: the view byte stored at +0x17F and the three open / close frame elements. `out[0]` = the
/// stored byte, `out[1..4]` = the frames, `out[4]` = the draw after them (0 = the slot, 1 = the switch view).
/// Returns -1 for a null `out`.
///
/// # Safety
/// `out`: room for 5 bytes.
#[no_mangle]
pub unsafe extern "C" fn ja1f_slot_view(view: u32, redraw_off: u32, stored: *mut u8, out: *mut u32, max: i32) -> i32 {
    if out.is_null() || stored.is_null() {
        return E_ARG;
    }
    let (v, calls) = panel::slot_view_calls(view as u8, redraw_off as u8);
    if calls.len() > max as usize {
        return E_ARG;
    }
    *stored = v;
    for (i, c) in calls.iter().enumerate() {
        let (code, arg) = match *c {
            panel::SlotViewCall::Back(f) => (1, f as u32),
            panel::SlotViewCall::Copy => (2, 0),
            panel::SlotViewCall::Wait(ms) => (3, ms),
            panel::SlotViewCall::Slot => (4, 0),
            panel::SlotViewCall::Switch => (5, 0),
        };
        *out.add(i * 2) = code;
        *out.add(i * 2 + 1) = arg;
    }
    calls.len() as i32
}

fn put_fields(w: &world::World, out: *mut u8, cap: usize) -> i32 {
    let o = quicksave::field_bytes(w);
    if o.len() > cap {
        return E_ARG;
    }
    // SAFETY: the caller gives `cap` bytes at `out`.
    unsafe { std::ptr::copy_nonoverlapping(o.as_ptr(), out, o.len()) };
    o.len() as i32
}

/// quickrestore_diff: the world's fields by name after `quicksave::restore` of `q` over the day save `day`
/// (`quicksave::field_table`: u8 name length, name, u32 length, bytes). Result: the bytes written.
///
/// # Safety
/// The paths are C strings; `q`: `q_len` bytes; `out`: `cap` bytes.
#[no_mangle]
pub unsafe extern "C" fn ja1f_quick_fields(iso_path: *const c_char, exe_path: *const c_char, day_path: *const c_char,
                                           q: *const u8, q_len: usize, out: *mut u8, cap: usize) -> i32 {
    let (Some(iso), Some(exe), Some(day)) = (cstr(iso_path), cstr(exe_path), cstr(day_path)) else { return E_ARG };
    let (q, out) = (q as usize, out as usize);
    guard(move || {
        let Ok(jagged) = Iso::open(iso).and_then(|mut i| i.read("DAT/JAGGED.DAT")) else { return E_FILE };
        let Ok(mut w) = world::World::load(iso, exe, day) else { return E_FILE };
        let q = std::slice::from_raw_parts(q as *const u8, q_len);
        if quicksave::restore(&mut w, &jagged, q).is_err() {
            return E_FILE;
        }
        put_fields(&w, out as *mut u8, cap)
    })
}

/// quickrestore_diff: the volume numbers of sound.rs (music, sound), the two numbers that Quick Restore reads back.
///
/// # Safety
/// `out`: 2 i32.
#[no_mangle]
pub unsafe extern "C" fn ja1f_sound_volumes(out: *mut i32) {
    let v = sound::volumes();
    *out = v.music;
    *out.add(1) = v.sound;
}

/// loadgame_diff: the world's fields by name after `World::from_parts` on the day save `save`.
///
/// # Safety
/// The paths are C strings; `save`: `save_len` bytes; `out`: `cap` bytes.
#[no_mangle]
pub unsafe extern "C" fn ja1f_day_fields(iso_path: *const c_char, exe_path: *const c_char, save: *const u8,
                                         save_len: usize, out: *mut u8, cap: usize) -> i32 {
    let (Some(iso), Some(exe)) = (cstr(iso_path), cstr(exe_path)) else { return E_ARG };
    let (save, out) = (save as usize, out as usize);
    guard(move || {
        let files = match world_files(iso, exe) {
            Ok(f) => f,
            Err(e) => return e,
        };
        let save = std::slice::from_raw_parts(save as *const u8, save_len);
        let Ok(w) = world::World::from_parts(exe, &files.0, &files.1, save) else { return E_FILE };
        put_fields(&w, out as *mut u8, cap)
    })
}

/// loadsector_diff: the world's fields by name after the day save `save` and a sector entry
/// (`enter::enter_world`: the job of LoadTacticalSector? 0x2AD20 with EAX 0) from the random seed `seed`.
/// `*end_seed` = the seed after the entry. Result: the byte count, or an error below 0.
///
/// # Safety
/// The paths are C strings; `save`: `save_len` bytes; `out`: `cap` bytes; `end_seed`: one u32.
#[no_mangle]
pub unsafe extern "C" fn ja1f_enter_fields(iso_path: *const c_char, exe_path: *const c_char, save: *const u8,
                                           save_len: usize, seed: u32, out: *mut u8, cap: usize,
                                           end_seed: *mut u32) -> i32 {
    let (Some(iso), Some(exe)) = (cstr(iso_path), cstr(exe_path)) else { return E_ARG };
    let (save, out, end_seed) = (save as usize, out as usize, end_seed as usize);
    guard(move || {
        let files = match world_files(iso, exe) {
            Ok(f) => f,
            Err(e) => return e,
        };
        let save = std::slice::from_raw_parts(save as *const u8, save_len);
        let Ok(mut w) = world::World::from_parts(exe, &files.0, &files.1, save) else { return E_FILE };
        // Noise in every field the entry starts again: a reset the port lacks shows up as a mismatch.
        {
            let u = &mut w.update;
            u.first_hit = 0x55;
            u.turn_flag = 0x55;
            u.turn_clock = 0x5555;
            u.redraw_pending = 0x5555;
            u.squad_lost = 0x55;
            u.enemies_warned = 0x55;
            u.quit_tactical = 0x55;
            u.dead_mercs = 0x55;
            u.dead_allies = 0x55;
            u.dead_npc = 0x55;
            u.enemy_deaths = 0x55;
            u.redraw_next = 0x55;
            u.weather_tire = 0x55;
        }
        w.small_panel = 0x55;
        w.strip_flag = 0x55;
        w.inv.strip_box = 0x55;
        w.inv.face_left = 0x55;
        let Ok((_, end, _)) = enter::enter_world(iso, exe, &mut w, seed) else { return E_FILE };
        *(end_seed as *mut u32) = end;
        let mut p = play::Play::new(end);
        let c = &mut p.cursor;
        (c.marker_kind, c.marker_tile, c.marker_blink, c.blink_timer, c.mark_timer) = (0x55, 0x55, 0x55, 0x55, 0x55);
        (c.over_map, c.mode, c.value, c.tile, c.ap) = (0x55, 0x55, 0x55, 0x55, 0x55);
        p.scroll = viewmode::Scroll { dy: 0x55, dx: 0x55, hover: 0x55, delay: 0x55 };
        p.hover.lit = 0x55;
        p.slots.hover = 0x55;
        p.sector_start(&mut w);
        put_enter_fields(&w, &p, out as *mut u8, cap)
    })
}

/// The world's fields and the front-end state of the sector entry (see `ja1f_enter_fields`).
///
/// # Safety
/// `out`: `cap` bytes.
unsafe fn put_enter_fields(w: &world::World, p: &play::Play, out: *mut u8, cap: usize) -> i32 {
    let n = put_fields(w, out, cap);
    if n < 0 {
        return n;
    }
    let (c, sc) = (&p.cursor, &p.scroll);
    let mut o = Vec::new();
    let i4 = |v: i32| v.to_le_bytes().to_vec();
    for (name, b) in [("c.marker_kind", i4(c.marker_kind)), ("c.marker_tile", i4(c.marker_tile)),
                      ("c.marker_blink", i4(c.marker_blink)), ("c.blink_timer", i4(c.blink_timer)),
                      ("c.mark_timer", i4(c.mark_timer)), ("c.over_map", vec![c.over_map]), ("c.mode", vec![c.mode]),
                      ("c.value", vec![c.value]), ("c.tile", i4(c.tile)), ("c.ap", i4(c.ap)),
                      ("c.scroll_dy", vec![sc.dy as u8]), ("c.scroll_dx", vec![sc.dx as u8]),
                      ("c.scroll_hover", i4(sc.hover)), ("c.scroll_delay", i4(sc.delay)),
                      ("c.lit", vec![p.hover.lit]), ("c.slot_hover", vec![p.slots.hover]),
                      ("c.strip_box", vec![w.inv.strip_box]), ("c.quit_tactical", vec![w.update.quit_tactical]),
                      ("c.strip_flag", vec![w.strip_flag])] {
        o.push(name.len() as u8);
        o.extend_from_slice(name.as_bytes());
        o.extend_from_slice(&(b.len() as u32).to_le_bytes());
        o.extend_from_slice(&b);
    }
    if n as usize + o.len() > cap {
        return E_ARG;
    }
    std::ptr::copy_nonoverlapping(o.as_ptr(), (out as *mut u8).add(n as usize), o.len());
    n + o.len() as i32
}

/// squadlost_diff: enter the sector of the day save (as `ja1f_enter_fields`), noise in the state that SquadLost?
/// 0x3A32C sets (the original gets the same noise), `sel` selected, then `Play::squad_lost_move`.
///
/// # Safety
/// As `ja1f_enter_fields`.
#[no_mangle]
pub unsafe extern "C" fn ja1f_squadlost_fields(iso_path: *const c_char, exe_path: *const c_char, save: *const u8,
                                               save_len: usize, seed: u32, sel: u8, out: *mut u8, cap: usize,
                                               end_seed: *mut u32) -> i32 {
    let (Some(iso), Some(exe)) = (cstr(iso_path), cstr(exe_path)) else { return E_ARG };
    let (save, out, end_seed) = (save as usize, out as usize, end_seed as usize);
    guard(move || {
        let files = match world_files(iso, exe) {
            Ok(f) => f,
            Err(e) => return e,
        };
        let save = std::slice::from_raw_parts(save as *const u8, save_len);
        let Ok(mut w) = world::World::from_parts(exe, &files.0, &files.1, save) else { return E_FILE };
        let Ok((_, end, _)) = enter::enter_world(iso, exe, &mut w, seed) else { return E_FILE };
        let mut p = play::Play::new(end);
        p.sector_start(&mut w);
        p.files = Some((iso.to_string(), exe.to_string()));
        w.combat = 1;
        w.selected = sel;
        w.update.screen_drawn = 0;
        w.update.view_hold = 1;
        w.update.turn_shown = 1;
        p.cursor.over_map = 0;
        p.cursor.mode = 3;
        panel::take_face_lights(); // only the move's frame changes count (ja1f_take_face_lights)
        if let Err(m) = p.squad_lost_move(&mut w) {
            eprintln!("squad_lost_move: {}", m);
            return E_FILE;
        }
        *(end_seed as *mut u32) = p.host.rng.seed;
        put_enter_fields(&w, &p, out as *mut u8, cap)
    })
}

/// squadlost_diff: the portrait frame changes noted since the last call (`panel::take_face_lights`), 8 bytes per slot,
/// 0 = no change, else the FaceLight? 0x46A4C element.
///
/// # Safety
/// `out`: 8 bytes.
#[no_mangle]
pub unsafe extern "C" fn ja1f_take_face_lights(out: *mut u8) {
    std::ptr::copy_nonoverlapping(panel::take_face_lights().as_ptr(), out, 8);
}

unsafe fn put_text(t: &[u8], out: *mut u8, max: i32) -> i32 {
    if out.is_null() || max < 0 || t.len() > max as usize {
        return E_ARG;
    }
    std::ptr::copy_nonoverlapping(t.as_ptr(), out, t.len());
    t.len() as i32
}

/// aimhired_diff: `aim::hired_text` of the 90-byte profile record `pr` (AimAlreadyHiredMsg? 0x2306C).
/// `info[0]` = the box's text x (0x42 = 66 at the left, 0 = centred), `info[1]` = its time limit. Result: text length.
///
/// # Safety
/// `pr`: 90 bytes; `out`: `max` bytes; `info`: 2 values.
#[no_mangle]
pub unsafe extern "C" fn ja1f_hired_text(pr: *const u8, out: *mut u8, max: i32, info: *mut i32) -> i32 {
    let (t, left) = aim::hired_text(std::slice::from_raw_parts(pr, 90));
    *info = if left { 0x42 } else { 0 };
    *info.add(1) = aim::HIRED_MS as i32;
    put_text(&t, out, max)
}

/// msgtext_diff: `play::sees_text` of the 512-byte record `rec` (MsgSeesEnemies 0x11448). Result: the text length.
///
/// # Safety
/// `rec`: 512 bytes; `out`: `max` bytes.
#[no_mangle]
pub unsafe extern "C" fn ja1f_sees_text(rec: *const u8, out: *mut u8, max: i32) -> i32 {
    put_text(&play::sees_text(std::slice::from_raw_parts(rec, 512)), out, max)
}

/// msgtext_diff: `sighting::weapon_text` of the 512-byte record `rec` (WeaponStatusText 0x56F4C).
///
/// # Safety
/// `rec`: 512 bytes; `out`: `max` bytes.
#[no_mangle]
pub unsafe extern "C" fn ja1f_weapon_text(rec: *const u8, out: *mut u8, max: i32) -> i32 {
    put_text(sighting::weapon_text(std::slice::from_raw_parts(rec, 512)).as_bytes(), out, max)
}

/// msgtext_diff: `play::guards_text` (count, number) (0x1F724 + MsgStrInt 0x27CE0); `ms`: the box time.
///
/// # Safety
/// `out`: `max` bytes; `ms`: one u32.
#[no_mangle]
pub unsafe extern "C" fn ja1f_guards_text(count: i32, number: i32, out: *mut u8, max: i32, ms: *mut u32) -> i32 {
    *ms = play::STATUS_MS;
    put_text(play::guards_text(count, number).as_bytes(), out, max)
}

/// msgtext_diff: the text of host op 213 hears_msg (MsgHearsNoise? 0x72A60).
///
/// # Safety
/// `rec`: 512 bytes; `out`: `max` bytes.
#[no_mangle]
pub unsafe extern "C" fn ja1f_hears_text(rec: *const u8, level: i32, kind: i32, dir: i32, out: *mut u8, max: i32) -> i32 {
    let rec = std::slice::from_raw_parts(rec, 512);
    put_text(play::hears_text(rec, level, kind, dir).as_bytes(), out, max)
}

/// msgtext_diff: the text of host op 335 battle_msg.
///
/// # Safety
/// `out`: `max` bytes.
#[no_mangle]
pub unsafe extern "C" fn ja1f_battle_text(t: i32, n: i32, out: *mut u8, max: i32) -> i32 {
    put_text(play::battle_text(t, n).as_bytes(), out, max)
}

/// msgtext_diff: the text of host op 125 message; -1 for no text.
///
/// # Safety
/// `out`: `max` bytes.
#[no_mangle]
pub unsafe extern "C" fn ja1f_message_text(text: i32, out: *mut u8, max: i32) -> i32 {
    match play::message_text(text) {
        Some(t) => put_text(t.as_bytes(), out, max),
        None => -1,
    }
}

/// msgtext_diff: the turn banner of `kind`; `colour`: one i32; -1 for no banner.
///
/// # Safety
/// `out`: `max` bytes.
#[no_mangle]
pub unsafe extern "C" fn ja1f_banner(kind: i32, out: *mut u8, max: i32, colour: *mut i32) -> i32 {
    match play::Banner::from_kind(kind) {
        Some(b) => {
            *colour = b.colour as i32;
            put_text(b.text.as_bytes(), out, max)
        }
        None => -1,
    }
}

/// msgtext_diff: the box of host op 203 (MsgEnemiesNearby 0x1B0EC); `ms`: the box time.
///
/// # Safety
/// `out`: `max` bytes; `ms`: one u32.
#[no_mangle]
pub unsafe extern "C" fn ja1f_nearby_msg(out: *mut u8, max: i32, ms: *mut u32) -> i32 {
    let (text, time) = play::nearby_box();
    *ms = time;
    put_text(text.as_bytes(), out, max)
}

/// msgtext_diff: the sighting part of the loop head (`sighting::loop_head_with`, FUN_56D3C 0x56D3C) with the Merc
/// message call logged. `sight`: in/out slot, how, near, fx_id (4 bytes). `called`: 1 when the message call ran,
/// with `slot_how_near` = its slot, how, near. Result: the text length, or 0 with no call.
///
/// # Safety
/// `soldiers`: 48 x 512 bytes; `fx_anims`: 16 x 0x12 bytes; `sight`: 4 bytes; `called`: one byte;
/// `slot_how_near`: 3 bytes; `out`: `max` bytes.
#[no_mangle]
pub unsafe extern "C" fn ja1f_sight_head(soldiers: *mut u8, fx_anims: *const u8, sight: *mut u8, called: *mut u8,
                                         slot_how_near: *mut u8, out: *mut u8, max: i32) -> i32 {
    let mut w: host::Ja1World = std::mem::zeroed();
    w.soldiers = soldiers;
    w.fx_anims = fx_anims;
    w.fx_id = *sight.add(3);
    let mut s = sighting::Sight { slot: *sight, how: *sight.add(1) as i8, near: *sight.add(2) };
    let mut len = 0;
    *called = 0;
    sighting::loop_head_with(&mut s, &mut w, |slot, how, near, text| {
        *called = 1;
        *slot_how_near = slot;
        *slot_how_near.add(1) = how as u8;
        *slot_how_near.add(2) = near as u8;
        len = put_text(text.as_bytes(), out, max);
        None
    });
    *sight = s.slot;
    *sight.add(1) = s.how as u8;
    *sight.add(2) = s.near;
    *sight.add(3) = w.fx_id;
    len
}

/// balloonview_diff: `sighting::merc_message` with the balloon's end (`right` = the right button) and its view move.
/// `vals` (in / out): view x, y, w, h, opt_onscreen, follow_all, focus_slot, shown_slot, show_timer, view_tile, spotted,
/// focus_last. Result: 1 when the message ran, with a balloon 3; `slide`: the view slide's steps.
///
/// # Safety
/// `soldiers`: 48 x 512 bytes; `vals`: 12 values; `slide`: one value.
#[no_mangle]
pub unsafe extern "C" fn ja1f_balloon_view(soldiers: *mut u8, vals: *mut i32, slot: i32, how: i32, near: i32, right: i32, slide: *mut i32) -> i32 {
    let mut w: host::Ja1World = std::mem::zeroed();
    w.soldiers = soldiers;
    let v = std::slice::from_raw_parts_mut(vals, 12);
    (w.view_x, w.view_y, w.view_w, w.view_h) = (v[0], v[1], v[2], v[3]);
    w.opt_onscreen = v[4] as u8;
    w.follow_all = v[5] as u8;
    w.focus_slot = v[6] as u8;
    w.shown_slot = v[7];
    w.show_timer = v[8];
    w.view_tile = v[9];
    w.spotted = v[10];
    w.focus_last = v[11] as u8;
    let (mut msg, mut cry) = (None, None);
    let _ = focus::take_slides();
    let r = sighting::merc_message(&mut w, std::ptr::null(), slot as u8, how as i8, near, "", &mut msg, &mut cry);
    let mut res = 0;
    if let Some(r) = r {
        res = if r.balloon.is_some() { 3 } else { 1 };
        if let Some(u) = r.undo.filter(|u| right != 0 && u.right_skips) {
            let b = u.before;
            (w.view_x, w.view_y, w.show_timer, w.view_tile, w.spotted) = (b.view_x, b.view_y, b.show_timer, b.view_tile, b.spotted);
            (w.focus_slot, w.focus_last, w.shown_slot) = (b.focus_slot, b.focus_last, b.shown_slot);
            let _ = focus::take_slides();
        }
    }
    *slide = focus::take_slides().iter().map(|s| s.steps as i32).sum();
    (v[0], v[1], v[2], v[3]) = (w.view_x, w.view_y, w.view_w, w.view_h);
    v[6] = w.focus_slot as i32;
    v[7] = w.shown_slot;
    v[8] = w.show_timer;
    v[9] = w.view_tile;
    v[10] = w.spotted;
    v[11] = w.focus_last as i32;
    res
}

/// Sets the roofs flag (tactical_map.md "Drawing order"; Alt+Q in cursor.md "Debug keys (level 4)") for later renders.
#[no_mangle]
pub extern "C" fn ja1f_set_roofs(v: u8) {
    debugkeys::set_roofs(v)
}

/// # Safety
/// `iso_path`, `exe_path`: NUL-terminated paths; `out`: room for 16*cols * 14*rows bytes.
#[no_mangle]
pub unsafe extern "C" fn ja1f_render_view(
    iso_path: *const c_char,
    exe_path: *const c_char,
    sector: i32,
    c0: i32,
    r0: i32,
    cols: i32,
    rows: i32,
    out: *mut u8,
) -> i32 {
    let (Some(iso), Some(exe)) = (cstr(iso_path), cstr(exe_path)) else { return E_ARG };
    if out.is_null() {
        return E_ARG;
    }
    let out = out as usize;
    guard(move || match render_view(iso, exe, sector, c0, r0, cols, rows) {
        Ok(v) => {
            std::ptr::copy_nonoverlapping(v.as_ptr(), out as *mut u8, v.len());
            0
        }
        Err(e) => e,
    })
}

/// # Safety
/// `iso_path`: NUL-terminated path; `out768`: room for 768 bytes.
#[no_mangle]
pub unsafe extern "C" fn ja1f_palette(iso_path: *const c_char, tileset: i32, out768: *mut u8) -> i32 {
    let Some(iso) = cstr(iso_path) else { return E_ARG };
    if out768.is_null() {
        return E_ARG;
    }
    let out = out768 as usize;
    guard(move || match palette(iso, tileset) {
        Ok(p) => {
            std::ptr::copy_nonoverlapping(p.as_ptr(), out as *mut u8, 768);
            0
        }
        Err(e) => e,
    })
}

/// LoadTerrainTileset 0x2A9FC: `record` = one 15,681-byte DAT/TERRAIN.DAT sector record (the 15680 tile bytes,
/// then the tileset byte); `base28` = the base remap table 28 (256 bytes); `lst` = MISC\TILESET<n>.LST (`lst_len`
/// bytes, 0xFF keeps the table's own byte). Fills `out_map` (3136 x 8), `out_tileset` and `out_table28` (256);
/// returns 0.
///
/// # Safety
/// The pointers must hold the sizes above.
#[no_mangle]
pub unsafe extern "C" fn ja1f_terrain_load(record: *const u8, base28: *const u8, lst: *const u8, lst_len: usize,
                                           out_map: *mut u8, out_tileset: *mut u8, out_table28: *mut u8) -> i32 {
    if record.is_null() || base28.is_null() || lst.is_null() || out_map.is_null() || out_tileset.is_null()
        || out_table28.is_null() {
        return E_ARG;
    }
    let record = std::slice::from_raw_parts(record, data::RECORD);
    let mut base = [0u8; 256];
    base.copy_from_slice(std::slice::from_raw_parts(base28, 256));
    let lst = std::slice::from_raw_parts(lst, lst_len);
    let ts = record[data::TILES * 5];
    data::fill_map_record(record, std::slice::from_raw_parts_mut(out_map, data::TILES * 8));
    let t = data::table28_from_lst(&base, lst, ts);
    std::ptr::copy_nonoverlapping(t.as_ptr(), out_table28, 256);
    *out_tileset = ts;
    0
}

/// terrainload_diff: remap table `k` of data::Common (JAGGED.DAT with table 29's fixed overrides) into `out` (256 B).
///
/// # Safety
/// `terrain` / `jag` hold `tlen` / `jlen` bytes, `out` 256.
#[no_mangle]
pub unsafe extern "C" fn ja1f_common_remap(terrain: *const u8, tlen: usize, jag: *const u8, jlen: usize, k: i32, out: *mut u8) -> i32 {
    let t = std::slice::from_raw_parts(terrain, tlen).to_vec();
    let Ok(c) = data::Common::from_files(t, std::slice::from_raw_parts(jag, jlen)) else { return E_ARG };
    let Some(tab) = usize::try_from(k).ok().and_then(|k| c.remap.get(k)) else { return E_ARG };
    std::ptr::copy_nonoverlapping(tab.as_ptr(), out, 256);
    0
}

/// Tileset (0..9) of sector 1..60, or a negative error.
///
/// # Safety
/// `iso_path`: NUL-terminated path.
#[no_mangle]
pub unsafe extern "C" fn ja1f_tileset(iso_path: *const c_char, sector: i32) -> i32 {
    let Some(iso) = cstr(iso_path) else { return E_ARG };
    guard(move || match sector_tileset(iso, sector) {
        Ok(t) => t as i32,
        Err(e) => e,
    })
}

/// C layout of the panel test interface (panel.md "Front-end interface").
#[repr(C)]
pub struct Ja1fPanel {
    pub slot: u8,
    pub selected: u8,
    pub combat: u8,
    pub opt_onscreen: u8,
    pub speaker: u8,
    pub hover: u8,
    pub ap_warned: [u8; 8],
    pub faces: *const u8,
    pub soldiers: *mut u8,
    pub table0: *mut u8,
}

/// # Safety
/// Pointers of `p` valid (faces 8 x 1024, soldiers 48 x 512, table0 256); `panel` 320 x 200 bytes.
unsafe fn draw_slot(iso_path: &str, exe_path: &str, tileset: i32, p: &Ja1fPanel, panel: *mut u8, what: i32)
    -> Result<(), i32> {
    if !(0..=9).contains(&tileset) || p.faces.is_null() || p.soldiers.is_null() || p.table0.is_null() {
        return Err(E_ARG);
    }
    let c = common(iso_path)?;
    let d = panel::panel_data(iso_path, exe_path)?;
    let t = tileset_data(iso_path, tileset as u8)?;
    let faces = std::slice::from_raw_parts(p.faces, 8 * 1024);
    let soldiers = std::slice::from_raw_parts_mut(p.soldiers, 48 * REC);
    let table0 = &mut *(p.table0 as *mut [u8; 256]);
    let buf = std::slice::from_raw_parts_mut(panel, (panel::PW * panel::PH) as usize);
    let env = panel::SlotEnv {
        slot: p.slot,
        selected: p.selected,
        combat: p.combat,
        opt_onscreen: p.opt_onscreen,
        speaker: p.speaker,
        hover: p.hover,
        ap_warned: p.ap_warned,
        faces,
        tileset: &t,
    };
    let mut painter = panel::Painter { d: &d, buf, table0, table29: panel::table29(&c) };
    match what {
        -1 => panel::draw_slot(&mut painter, &env, soldiers),
        0 => panel::draw_switch_view(&mut painter, &env, soldiers),
        1..=7 => panel::switch_frame(&mut painter, p.slot, p.selected, what as usize),
        0x100..=0x105 => {
            let i = (what - 0x100) as usize / 2;
            panel::switch_face(&mut painter, p.slot, p.selected, i, what & 1 != 0, true)
        }
        _ => return Err(E_ARG),
    }
    Ok(())
}

/// Draw one merc panel slot into `panel` (320 x 200, in/out).
///
/// # Safety
/// `iso_path`, `exe_path`: NUL-terminated paths; `p`: valid Ja1fPanel; `panel`: 320 x 200 bytes.
#[no_mangle]
pub unsafe extern "C" fn ja1f_draw_slot(
    iso_path: *const c_char,
    exe_path: *const c_char,
    tileset: i32,
    p: *const Ja1fPanel,
    panel: *mut u8,
) -> i32 {
    let (Some(iso), Some(exe)) = (cstr(iso_path), cstr(exe_path)) else { return E_ARG };
    if p.is_null() || panel.is_null() {
        return E_ARG;
    }
    let (p, panel) = (p as usize, panel as usize);
    guard(move || match draw_slot(iso, exe, tileset, &*(p as *const Ja1fPanel), panel as *mut u8, -1) {
        Ok(()) => 0,
        Err(e) => e,
    })
}

/// panel.md "The switch view": what = 0 the switch view of slot p.slot, 1..7 an open / close animation frame
/// (slot background element `what`), 0x100 + 2 i + on = switch i's pressed face.
///
/// # Safety
/// As ja1f_draw_slot.
#[no_mangle]
pub unsafe extern "C" fn ja1f_draw_switch(
    iso_path: *const c_char,
    exe_path: *const c_char,
    tileset: i32,
    p: *const Ja1fPanel,
    panel: *mut u8,
    what: i32,
) -> i32 {
    let (Some(iso), Some(exe)) = (cstr(iso_path), cstr(exe_path)) else { return E_ARG };
    if p.is_null() || panel.is_null() {
        return E_ARG;
    }
    let (p, panel) = (p as usize, panel as usize);
    guard(move || match draw_slot(iso, exe, tileset, &*(p as *const Ja1fPanel), panel as *mut u8, what) {
        Ok(()) => 0,
        Err(e) => e,
    })
}

/// Fill a face buffer for face number n: kind 0 alive (also writes *eye_row), 1 dead, 2 after the death morph.
///
/// # Safety
/// `iso_path`, `exe_path`: NUL-terminated paths; `out`: 1024 bytes; `eye_row`: 1 byte (kind 0).
#[no_mangle]
pub unsafe extern "C" fn ja1f_face(
    iso_path: *const c_char,
    exe_path: *const c_char,
    n: i32,
    kind: i32,
    out: *mut u8,
    eye_row: *mut u8,
) -> i32 {
    let (Some(iso), Some(exe)) = (cstr(iso_path), cstr(exe_path)) else { return E_ARG };
    if out.is_null() || (kind == 0 && eye_row.is_null()) {
        return E_ARG;
    }
    let (out, eye_row) = (out as usize, eye_row as usize);
    guard(move || {
        let d = match panel::panel_data(iso, exe) {
            Ok(d) => d,
            Err(e) => return e,
        };
        let buf = std::slice::from_raw_parts_mut(out as *mut u8, 1024);
        match panel::load_face(iso, &d, n, kind, buf) {
            Ok(Some(eye)) => {
                *(eye_row as *mut u8) = eye;
                0
            }
            Ok(None) => 0,
            Err(e) => e,
        }
    })
}

/// Fonts loaded by LoadSmallFont 0x381F8: the three font sheets (0xC16 + 0xF72 + 0x24FC bytes, in file order) into
/// `sheets`, and the char table it computes (3 x 0x189 bytes: the running x offset as a word, then that char's width)
/// into `chars`. Returns 0, or E_FILE when a font file has not the size the loader's constants give.
///
/// # Safety
/// `iso_path`, `exe_path`: NUL-terminated paths; `sheets`: 16516 bytes; `chars`: 1183 bytes.
#[no_mangle]
pub unsafe extern "C" fn ja1f_fonts(iso_path: *const c_char, exe_path: *const c_char, sheets: *mut u8,
                                    chars: *mut u8) -> i32 {
    const SIZES: [usize; 3] = [0xC16, 0xF72, 0x24FC];
    const CHARS: usize = 3 * 0x189;
    let (Some(iso), Some(exe)) = (cstr(iso_path), cstr(exe_path)) else { return E_ARG };
    if sheets.is_null() || chars.is_null() {
        return E_ARG;
    }
    let (sheets, chars) = (sheets as usize, chars as usize);
    guard(move || {
        let d = match panel::panel_data(iso, exe) {
            Ok(d) => d,
            Err(e) => return e,
        };
        let out = std::slice::from_raw_parts_mut(sheets as *mut u8, SIZES.iter().sum());
        let table = std::slice::from_raw_parts_mut(chars as *mut u8, CHARS);
        let mut at = 0;
        for (font, &n) in d.fonts.iter().zip(SIZES.iter()) {
            if font.sheet.len() != n {
                return E_FILE;
            }
            out[at..at + n].copy_from_slice(&font.sheet);
            at += n;
        }
        for (f, font) in d.fonts.iter().enumerate() {
            let mut off: u16 = 0;
            for c in 0..131 {
                let b = f * 0x189 + c * 3;
                table[b..b + 2].copy_from_slice(&off.to_le_bytes());
                table[b + 2] = font.widths[c];
                off = off.wrapping_add(font.widths[c] as u16);
            }
        }
        0
    })
}

/// Quick Save 0x2D880 vs the port: restore the original's QUICK.SAV image `q` on a world loaded from `day_path`
/// (quick-save.md) and write it again with `fill` in the bytes no world field holds (0 = the port's file).
/// Returns the number of bytes written, or E_ARG / E_FILE.
///
/// # Safety
/// `iso_path`, `exe_path`, `day_path`: NUL-terminated paths; `q`: `q_len` bytes; `out`: `out_cap` bytes.
#[no_mangle]
pub unsafe extern "C" fn ja1f_quick_rewrite(iso_path: *const c_char, exe_path: *const c_char,
                                            day_path: *const c_char, q: *const u8, q_len: usize, fill: i32,
                                            out: *mut u8, out_cap: usize) -> i32 {
    let (Some(iso), Some(exe), Some(day)) = (cstr(iso_path), cstr(exe_path), cstr(day_path)) else { return E_ARG };
    if q.is_null() || out.is_null() || q_len > 1 << 24 {
        return E_ARG;
    }
    let (q, out) = (q as usize, out as usize);
    guard(move || {
        let jagged = match Iso::open(iso).and_then(|mut i| i.read("DAT/JAGGED.DAT")) {
            Ok(j) => j,
            Err(_) => return E_FILE,
        };
        let mut w = match world::World::load(iso, exe, day) {
            Ok(w) => w,
            Err(_) => return E_FILE,
        };
        let q = std::slice::from_raw_parts(q as *const u8, q_len);
        let theme = match quicksave::restore(&mut w, &jagged, q) {
            Ok(t) => t,
            Err(_) => return E_FILE,
        };
        let bytes = quicksave::write_fill(&w, theme, fill as u8);
        if bytes.len() > out_cap {
            return E_ARG;
        }
        std::ptr::copy_nonoverlapping(bytes.as_ptr(), out as *mut u8, bytes.len());
        bytes.len() as i32
    })
}

/// tacredraw_diff: the inputs of one tactical frame (TacticalRedraw 0x176C8) on a world restored from a quick save.
#[repr(C)]
pub struct Ja1fFrame {
    pub tileset: i32,
    pub c0: i32,
    pub r0: i32,
    pub cols: i32,
    pub rows: i32,
    pub turn_timer: i32,
    pub turn_timer2: i32,
    pub time_compress: i32,
    pub darken: i32,
    pub stop_timer: i32,
    pub screen_drawn: i32,
    pub sees_slot: i32,
    pub lit: i32,
    pub named: i32,
    pub roofs: i32,
    pub show_all: i32,
    pub view_mode: i32,
    pub banner_colour: i32,
    pub banner: *const c_char,
    pub sees_text: *const c_char,
    /// `patch_n` triples (soldier slot, record offset, byte), the 288 bytes of the effect records (null = keep), and
    /// the 240 bytes of the hit-number texts (null = keep).
    pub patch: *const i32,
    pub patch_n: i32,
    pub fx: *const u8,
    pub dmg_text: *const u8,
    /// 8 x 1024 face buffers, the combat byte, and the 8 "AP warned" bytes.
    pub faces: *const u8,
    pub combat: i32,
    pub ap_warned: *const u8,
    /// Out: the 8 eye bytes +0x1AC after the frame (null = not wanted).
    pub eyes_out: *mut u8,
}

/// tacredraw_diff: restore the fixed part + tail `q` of a quick save on the day save `day`, and draw one frame with
/// `enter::render_world_cursor`. `st` is read and written back; `out` gets 16*cols x 14*rows bytes, and
/// `reload_msg` the byte the frame sets (0x284F14 in the original is cleared by the frame: see the test).
///
/// # Safety
/// The paths and the texts are C strings; `q`: `q_len` bytes; `fr`, `st`, `reload_msg`: valid; `out`: room for the view.
#[no_mangle]
pub unsafe extern "C" fn ja1f_tactical_frame(iso_path: *const c_char, exe_path: *const c_char, day_path: *const c_char,
                                             q: *const u8, q_len: usize, fr: *const Ja1fFrame,
                                             st: *mut cursor::CursorState, out: *mut u8, reload_msg: *mut u8) -> i32 {
    let (Some(iso), Some(exe), Some(day)) = (cstr(iso_path), cstr(exe_path), cstr(day_path)) else { return E_ARG };
    if q.is_null() || out.is_null() || fr.is_null() || st.is_null() || q_len > 1 << 24 {
        return E_ARG;
    }
    let (q, fr, st, out, reload_msg) = (q as usize, fr as usize, st as usize, out as usize, reload_msg as usize);
    guard(move || {
        let fr = &*(fr as *const Ja1fFrame);
        let jagged = match Iso::open(iso).and_then(|mut i| i.read("DAT/JAGGED.DAT")) {
            Ok(j) => j,
            Err(_) => return E_FILE,
        };
        let mut w = match world::World::load(iso, exe, day) {
            Ok(w) => w,
            Err(_) => return E_FILE,
        };
        if quicksave::restore(&mut w, &jagged, std::slice::from_raw_parts(q as *const u8, q_len)).is_err() {
            return E_FILE;
        }
        w.update.turn_timer = fr.turn_timer as i16;
        w.update.turn_timer2 = fr.turn_timer2 as i16;
        w.update.time_compress = fr.time_compress as u8;
        w.update.stop_timer = fr.stop_timer;
        w.update.screen_drawn = fr.screen_drawn as u8;
        w.update.sees_slot = fr.sees_slot as u8;
        w.update.follow_all = fr.show_all as _;
        w.update.opt_reload_msg = 0;
        w.opt_onscreen = fr.view_mode as u8;
        w.view_w = fr.cols * 16;
        w.view_h = fr.rows * 14;
        if fr.patch_n > 0 {
            for t in std::slice::from_raw_parts(fr.patch, fr.patch_n as usize * 3).chunks(3) {
                w.soldiers[t[0] as usize * 512 + t[1] as usize] = t[2] as u8;
            }
        }
        if !fr.fx.is_null() {
            w.fx_anims.copy_from_slice(std::slice::from_raw_parts(fr.fx, 288));
        }
        if !fr.dmg_text.is_null() {
            w.dmg_text.copy_from_slice(std::slice::from_raw_parts(fr.dmg_text, 240));
        }
        if !fr.sees_text.is_null() {
            w.sees_text = std::ffi::CStr::from_ptr(fr.sees_text).to_bytes().to_vec();
        }
        if fr.darken != 0 {
            enter::darken_next();
        }
        w.combat = fr.combat as _;
        if !fr.ap_warned.is_null() {
            let n = w.ap_warned.len().min(8);
            w.ap_warned[..n].copy_from_slice(std::slice::from_raw_parts(fr.ap_warned, n));
        }
        if !fr.faces.is_null() {
            enter::view_faces(std::slice::from_raw_parts(fr.faces, 8 * 1024));
        }
        enter::label_slots(fr.lit as u8, fr.named as u8);
        debugkeys::set_roofs(fr.roofs as u8);
        let banner_text = if fr.banner.is_null() { None } else { cstr(fr.banner) };
        let banner = banner_text.map(|t| (t, fr.banner_colour as u8));
        let img = enter::render_world_cursor(iso, exe, &mut w, fr.tileset as u8, fr.c0, fr.r0, fr.cols, fr.rows,
                                             Some(&mut *(st as *mut cursor::CursorState)), banner, None);
        enter::label_slots(0x40, 0x40);
        debugkeys::set_roofs(1);
        match img {
            Ok(v) => {
                std::ptr::copy_nonoverlapping(v.as_ptr(), out as *mut u8, v.len());
                if reload_msg != 0 {
                    *(reload_msg as *mut u8) = w.update.opt_reload_msg;
                }
                if !fr.eyes_out.is_null() {
                    for k in 0..8 {
                        *fr.eyes_out.add(k) = w.soldiers[k * 512 + 0x1AC];
                    }
                }
                0
            }
            Err(e) => e,
        }
    })
}

/// LoadGame 0x4E964 vs the port: load the day save `save` (the original reads it at file offset 0x1C) and
/// write the fixed part back with `fill` in the bytes no world field holds (0 = the port's file). `keep` = the
/// running program's Sound FX (low byte) and Music (next byte) switches: a day save load puts them back after the
/// read like the Restore Game and "Abort this day" loads (`ctlpanel::put_sound_switches`); -1 = the file's values.
/// Returns the fixed part's length, or E_ARG / E_FILE.
///
/// # Safety
/// `iso_path`, `exe_path`: NUL-terminated paths; `save`: `save_len` bytes; `out`: `out_cap` bytes.
#[no_mangle]
pub unsafe extern "C" fn ja1f_loadgame(iso_path: *const c_char, exe_path: *const c_char,
                                       save: *const u8, save_len: usize, fill: i32, keep: i32,
                                       out: *mut u8, out_cap: usize) -> i32 {
    let (Some(iso), Some(exe)) = (cstr(iso_path), cstr(exe_path)) else { return E_ARG };
    if save.is_null() || out.is_null() || save_len > 1 << 24 {
        return E_ARG;
    }
    let (save, out) = (save as usize, out as usize);
    guard(move || {
        let files = match world_files(iso, exe) {
            Ok(f) => f,
            Err(e) => return e,
        };
        let save = std::slice::from_raw_parts(save as *const u8, save_len);
        let mut w = match world::World::from_parts(exe, &files.0, &files.1, save) {
            Ok(w) => w,
            Err(_) => return E_FILE,
        };
        if keep >= 0 {
            crate::ctlpanel::put_sound_switches(&mut w, [keep as u8, (keep >> 8) as u8]);
        }
        let bytes = w.save_bytes_fill(fill as u8);
        let n = bytes.len().min(world::SAVE_FIXED_END);
        if n > out_cap {
            return E_ARG;
        }
        std::ptr::copy_nonoverlapping(bytes.as_ptr(), out as *mut u8, n);
        n as i32
    })
}

/// The stat popup of slot k: the picture and numbers into `panel` at (173, 18), the block to `screen`.
///
/// # Safety
/// `soldiers`: 48 x 512 bytes; `panel`, `screen`: 320 x 200 bytes each.
#[no_mangle]
pub unsafe extern "C" fn ja1f_draw_popup(
    iso_path: *const c_char,
    exe_path: *const c_char,
    k: i32,
    soldiers: *const u8,
    panel: *mut u8,
    screen: *mut u8,
) -> i32 {
    let (Some(iso), Some(exe)) = (cstr(iso_path), cstr(exe_path)) else { return E_ARG };
    if !(0..8).contains(&k) || soldiers.is_null() || panel.is_null() || screen.is_null() {
        return E_ARG;
    }
    let (soldiers, panel, screen) = (soldiers as usize, panel as usize, screen as usize);
    guard(move || {
        let (c, d) = match (common(iso), panel::panel_data(iso, exe)) {
            (Ok(c), Ok(d)) => (c, d),
            (Err(e), _) | (_, Err(e)) => return e,
        };
        let soldiers = std::slice::from_raw_parts(soldiers as *const u8, 48 * REC);
        let buf = std::slice::from_raw_parts_mut(panel as *mut u8, (panel::PW * panel::PH) as usize);
        let scr = std::slice::from_raw_parts_mut(screen as *mut u8, (panel::PW * panel::PH) as usize);
        let mut table0 = c.remap[0];
        let mut painter = panel::Painter { d: &d, buf, table0: &mut table0, table29: panel::table29(&c) };
        panel::draw_popup(&mut painter, soldiers, k as usize);
        let (sx, sy) = screen::slot_origin(k as usize);
        panel::popup_to_screen(painter.buf, scr, k as usize, sx, sy);
        0
    })
}

/// The drown / injection popup (LoadPopups? 0x288DC, host op 71): writes the frames into `out` and
/// returns the byte count, or a negative error. `out` = [n i32][per frame: x i32, y i32, w i32, h i32,
/// then the 120 x 60 source block].
///
/// # Safety
/// `iso_path`, `exe_path`: NUL-terminated paths; `out`: room for `out_size` bytes.
#[no_mangle]
pub unsafe extern "C" fn ja1f_popup(
    iso_path: *const c_char,
    exe_path: *const c_char,
    slot: i32,
    selected: i32,
    flag: i32,
    out: *mut u8,
    out_size: i32,
) -> i32 {
    let (Some(_iso), Some(exe)) = (cstr(iso_path), cstr(exe_path)) else { return E_ARG };
    if out.is_null() || !(0..8).contains(&slot) {
        return E_ARG;
    }
    let (out, out_size) = (out as usize, out_size.max(0) as usize);
    if out_size < 4 {
        return E_ARG;
    }
    guard(move || {
        let f = match popup::frames(exe, slot as u8, selected as u8, flag) {
            Ok(f) => f,
            Err(e) => return e.raw_os_error().unwrap_or(E_FILE),
        };
        // The 4-byte count comes before the frames: `out_size - 4` bytes hold them.
        let n = ((out_size - 4) / (16 + popup::W as usize * popup::H as usize)).min(f.len());
        let buf = std::slice::from_raw_parts_mut(out as *mut u8, out_size);
        buf[..4].copy_from_slice(&(n as i32).to_le_bytes());
        let mut p = 4;
        for fr in &f[..n] {
            for (i, v) in [fr.x, fr.y, fr.w, fr.h].iter().enumerate() {
                buf[p + 4 * i..p + 4 * i + 4].copy_from_slice(&v.to_le_bytes());
            }
            p += 16;
            buf[p..p + (popup::W as usize * popup::H as usize)].copy_from_slice(&fr.src);
            p += popup::W as usize * popup::H as usize;
        }
        p as i32
    })
}

/// Draw frame `index` of a `ja1f_popup` result onto a 320 x 200 screen: the ScaledBlit of one popup
/// frame (LoadPopups? 0x288DC). Used by validate/diff/loadpopup_diff.py and the host play.
///
/// # Safety
/// `frames`: a `ja1f_popup` output buffer of `frames_size` bytes; `screen`: 320 x 200 bytes.
#[no_mangle]
pub unsafe extern "C" fn ja1f_popup_draw(
    frames: *const u8,
    frames_size: i32,
    index: i32,
    screen: *mut u8,
) -> i32 {
    let frame_bytes = 16 + (popup::W * popup::H) as usize;
    if frames.is_null() || screen.is_null() || frames_size < 4 || index < 0 {
        return E_ARG;
    }
    let (frames, frames_size) = (frames as usize, frames_size as usize);
    if frames_size > 64 << 20 {
        return E_ARG;
    }
    let buf = std::slice::from_raw_parts(frames as *const u8, frames_size);
    let n = i32::from_le_bytes([buf[0], buf[1], buf[2], buf[3]]);
    let p = 4 + index as usize * frame_bytes;
    if index >= n || p + frame_bytes > frames_size {
        return E_ARG;
    }
    let rd = |q: usize| i32::from_le_bytes([buf[q], buf[q + 1], buf[q + 2], buf[q + 3]]);
    let block = &buf[p + 16..p + frame_bytes];
    let fr = popup::Frame { x: rd(p), y: rd(p + 4), w: rd(p + 8), h: rd(p + 12), src: block.to_vec() };
    let scr = std::slice::from_raw_parts_mut(screen as *mut u8, (panel::PW * panel::PH) as usize);
    popup::blit(&fr, scr);
    0
}

/// C layout of the inventory screen drawing (inventory.md "Front-end interface: ja1f_draw_inventory").
#[repr(C)]
pub struct Ja1fInventory {
    pub panel: Ja1fPanel,
    pub tileset: i32,
    pub sector: u8,
    pub enemies: u8,
    pub blink: u8,
    pub sight: [u8; 8],
    pub small_panel: i32,
    pub time_w: i32,
    pub tiles: *const u8,
    pub ground: *const u8,
}

#[repr(C)]
pub struct Ja1fInventoryOut {
    pub armour: i32,
    pub load_result: i32,
    pub blink: u8,
    pub drawn: u8,
    pub item_rects: [u8; 56],
}

/// Draw the inventory screen of merc `v.panel.slot` (steps 1..14) into `panel` and `screen`.
///
/// # Safety
/// `v`: valid Ja1fInventory (faces 8 x 1024, soldiers 48 x 512, table0 256, tiles 3136 x 8, ground 200 x 12);
/// `panel`, `screen`: 320 x 200 bytes each (in/out); `out`: valid Ja1fInventoryOut.
#[no_mangle]
pub unsafe extern "C" fn ja1f_draw_inventory(
    iso_path: *const c_char,
    exe_path: *const c_char,
    v: *const Ja1fInventory,
    panel: *mut u8,
    screen: *mut u8,
    out: *mut Ja1fInventoryOut,
) -> i32 {
    let (Some(iso), Some(exe)) = (cstr(iso_path), cstr(exe_path)) else { return E_ARG };
    if v.is_null() || panel.is_null() || screen.is_null() || out.is_null() {
        return E_ARG;
    }
    let (v, panel, screen, out) = (v as usize, panel as usize, screen as usize, out as usize);
    guard(move || {
        let v = &*(v as *const Ja1fInventory);
        let p = &v.panel;
        if !(0..=9).contains(&v.tileset)
            || p.slot > 7
            || p.faces.is_null()
            || p.soldiers.is_null()
            || p.table0.is_null()
            || v.tiles.is_null()
            || v.ground.is_null()
        {
            return E_ARG;
        }
        let (c, d, id) = match (common(iso), panel::panel_data(iso, exe), invdraw::inv_data(iso, exe)) {
            (Ok(c), Ok(d), Ok(id)) => (c, d, id),
            (Err(e), _, _) | (_, Err(e), _) | (_, _, Err(e)) => return e,
        };
        let t = match tileset_data(iso, v.tileset as u8) {
            Ok(t) => t,
            Err(e) => return e,
        };
        let faces = std::slice::from_raw_parts(p.faces, 8 * 1024);
        let soldiers = std::slice::from_raw_parts_mut(p.soldiers, 48 * REC);
        let table0 = &mut *(p.table0 as *mut [u8; 256]);
        let buf = std::slice::from_raw_parts_mut(panel as *mut u8, (panel::PW * panel::PH) as usize);
        let scr = std::slice::from_raw_parts_mut(screen as *mut u8, (panel::PW * panel::PH) as usize);
        let env = invdraw::InvEnv {
            rec: p.slot as usize,
            selected: p.selected,
            combat: p.combat,
            opt: p.opt_onscreen,
            speaker: p.speaker,
            hover: p.hover,
            ap_warned: p.ap_warned,
            faces,
            tileset: &t,
            sector: v.sector,
            enemies: v.enemies,
            blink: v.blink,
            sight: v.sight,
            small_panel: v.small_panel,
            time_w: v.time_w,
            tiles: std::slice::from_raw_parts(v.tiles, 3136 * 8),
            ground: std::slice::from_raw_parts(v.ground, 200 * 12),
        };
        let mut painter = panel::Painter { d: &d, buf, table0, table29: panel::table29(&c) };
        let r = invdraw::draw_inventory(&mut painter, &id, &env, soldiers, scr);
        let o = &mut *(out as *mut Ja1fInventoryOut);
        o.armour = r.armour;
        o.load_result = r.load_result;
        o.blink = r.blink;
        o.drawn = r.drawn;
        o.item_rects = r.item_rects;
        0
    })
}

/// C layout of the interface strips drawing (panel.md "Front-end interface", ja1f_draw_strips).
#[repr(C)]
pub struct Ja1fStrips {
    pub panel: Ja1fPanel,
    pub tileset: i32,
    pub top: u8,
    pub keep: u8,
    pub sector: u8,
    pub mercs: u8,
    pub enemies: u8,
    pub strip_flag: u8,
    pub turn_slot: u8,
    pub blink: u8,
    pub combat_seen: u8,
    pub small_panel: i32,
    pub time_w: i32,
    pub sel_in_play: u8,
}

#[repr(C)]
pub struct Ja1fStripsOut {
    pub load_result: i32,
    pub blink: u8,
    pub combat_seen: u8,
    pub mouse_x: i32,
}

/// "The interface strips" into `panel` and `screen` (320 x 200 each, in/out); soldiers in/out.
///
/// # Safety
/// mouselimit_diff: the driver position of a mouse at (x, y) under the limit that 0x682AC sets.
/// pointer::poll_shape for pointer_diff.
#[no_mangle]
pub extern "C" fn ja1f_poll_shape(cur: i32, buttons: u8) -> i32 {
    pointer::poll_shape(cur, buttons)
}

/// pointer::shape for mouseshape_diff: `out` = w, h, hot x, hot y; `pix` holds w x h bytes (room for 42 x 16). 0 = no such
/// shape (an item), 1 = done, < 0 = load fault.
///
/// # Safety
/// C strings; `out` holds 4 i32, `pix` 672 bytes.
#[no_mangle]
pub unsafe extern "C" fn ja1f_pointer_shape(iso: *const c_char, exe: *const c_char, s: i32, out: *mut i32, pix: *mut u8) -> i32 {
    let (Some(iso), Some(exe)) = (cstr(iso), cstr(exe)) else { return -1 };
    let d = match panel::panel_data(iso, exe) {
        Ok(d) => d,
        Err(c) => return -c.abs().max(1),
    };
    let Some(sh) = pointer::shape(&d, s) else { return 0 };
    if sh.pix.len() > 672 {
        return -100;
    }
    for (i, v) in [sh.w, sh.h, sh.hx, sh.hy].into_iter().enumerate() {
        *out.add(i) = v;
    }
    std::ptr::copy_nonoverlapping(sh.pix.as_ptr(), pix, sh.pix.len());
    1
}

/// pointer::held_picture + pointer::held_item for mouseshape_diff: item type `item` at mouse (mx, my) over `buf` (64000 bytes).
///
/// # Safety
/// C strings; `types` holds `ntypes` bytes, `buf` 64000.
#[no_mangle]
pub unsafe extern "C" fn ja1f_pointer_held(iso: *const c_char, exe: *const c_char, types: *const u8, ntypes: usize, item: usize,
                                           mx: i32, my: i32, buf: *mut u8) -> i32 {
    let (Some(iso), Some(exe)) = (cstr(iso), cstr(exe)) else { return -1 };
    let (c, d, inv) = match (common(iso), panel::panel_data(iso, exe), invdraw::inv_data(iso, exe)) {
        (Ok(c), Ok(d), Ok(i)) => (c, d, i),
        _ => return -1,
    };
    let pic = pointer::held_picture(std::slice::from_raw_parts(types, ntypes), item);
    let mut table0 = c.remap[0];
    let buf = std::slice::from_raw_parts_mut(buf, 64000);
    let mut p = panel::Painter { d: &d, buf, table0: &mut table0, table29: panel::table29(&c) };
    pointer::held_item(&mut p, &inv, pic, mx, my);
    pic as i32
}

/// pointer::turn_mouse for mouseshape_diff: `tape` = `n` rows of (op, x, y); op 0 = the player moves the mouse to (x, y); `out` gets
/// the x, y after each row.
///
/// # Safety
/// `tape` holds 3 n i32, `out` 2 n i32.
#[no_mangle]
pub unsafe extern "C" fn ja1f_turn_mouse(x: i32, y: i32, tape: *const i32, n: i32, out: *mut i32) {
    let (mut now, mut saved) = ((x, y), (0, 0));
    for i in 0..n as usize {
        let (op, a, b) = (*tape.add(3 * i), *tape.add(3 * i + 1), *tape.add(3 * i + 2));
        if op == 0 {
            now = (a, b);
        } else if let Some(p) = pointer::turn_mouse(op, a, b, now, &mut saved) {
            now = p;
        }
        *out.add(2 * i) = now.0;
        *out.add(2 * i + 1) = now.1;
    }
}

/// pointer::top_row_hidden for pointer_diff.
#[no_mangle]
pub extern "C" fn ja1f_top_row_hidden(y: i32, hot_y: i32, flag: u8) -> i32 {
    pointer::top_row_hidden(y, hot_y, flag) as i32
}

/// pointer::hide_mouse for pointer_diff.
#[no_mangle]
pub extern "C" fn ja1f_hide_mouse(off: u8, over_map: u8) -> i32 {
    pointer::hide_mouse(off != 0, over_map) as i32
}

/// pointer::mouse_redraw for pointer_diff.
#[no_mangle]
pub extern "C" fn ja1f_mouse_redraw(off: u8, over_map: u8, ready: u8) -> i32 {
    pointer::mouse_redraw(off != 0, over_map, ready != 0) as i32
}

/// startup::START and startup::set_music_zero for startup_diff: title argument, limit (4), mouse (2), shape, off, zero.
#[no_mangle]
pub unsafe extern "C" fn ja1f_start_plan(driver: u16, music_on: u8, out: *mut i32) {
    let p = startup::START;
    let v = [p.title_arg, p.limit.0, p.limit.1, p.limit.2, p.limit.3, p.mouse.0, p.mouse.1, p.shape, p.pointer_off as i32,
             startup::set_music_zero(driver, music_on) as i32];
    std::slice::from_raw_parts_mut(out, v.len()).copy_from_slice(&v);
}

/// startup::START.music for startup_diff, a zero-ended string.
#[no_mangle]
pub extern "C" fn ja1f_start_music() -> *const std::os::raw::c_char {
    std::ffi::CString::new(startup::START.music).unwrap_or_default().into_raw()
}

#[no_mangle]
pub extern "C" fn ja1f_mouse_clamp(x0: i32, y0: i32, x1: i32, y1: i32, x: i32, y: i32) -> i64 {
    let (a, b) = mouselimit::clamp((x0, y0, x1, y1), x, y);
    ((a as i64) << 16) | (b as i64 & 0xFFFF)
}

/// ctlpanel_diff: the limit of the Control Panel, 0x4B854, 12 bits per value: x0, y0, x1, y1.
#[no_mangle]
pub extern "C" fn ja1f_ctl_mouse_box(view_w: i32, view_h: i32, onscreen: i32) -> i64 {
    let (a, b, c, d) = ctlpanel::mouse_box(view_w, view_h, onscreen);
    a as i64 | (b as i64) << 12 | (c as i64) << 24 | (d as i64) << 36
}

/// ctlshell_diff: ControlPanel 0x4B788 around its input loop. The lock state before (`st`: round_lock, lock_owned,
/// clock switch, blink byte, blink_timer) goes through the panel's Lock (clock 0) and Unlock (pause::Pause, the same
/// calls as `PlayState::locked_ticks` with no tick), and the close result `r` through `ctlpanel::after_close`.
/// `st` holds the state after; returns 1 = QuickSave, 0 = QuickRestore, -1 = neither.
///
/// # Safety
/// `st`: 5 i32.
#[no_mangle]
pub unsafe extern "C" fn ja1f_ctl_shell(st: *mut i32, r: i32) -> i32 {
    if st.is_null() {
        return E_ARG;
    }
    let v = std::slice::from_raw_parts_mut(st, 5);
    let mut rl = v[0] as u8;
    let mut p = pause::Pause { lock_owned: v[1] != 0, clock_on: v[2] != 0, blink: v[3] != 0, blink_timer: v[4],
                               ..Default::default() };
    p.lock(false, &mut rl);
    p.unlock(&mut rl);
    v.copy_from_slice(&[rl as i32, p.lock_owned as i32, p.clock_on as i32, p.blink as i32, p.blink_timer]);
    match ctlpanel::after_close(r) {
        Some(true) => 1,
        Some(false) => 0,
        None => -1,
    }
}

/// invwrap_diff: invscreen::cursor_after; result flag | moved << 8 | x << 16 (the row is always 191).
#[no_mangle]
pub extern "C" fn ja1f_cursor_after(held: u8, drop: u8, mouse_x: i32, screen_ran: i32, picked: i32) -> i64 {
    let (f, m) = invscreen::cursor_after(held, drop, mouse_x, screen_ran != 0, picked != 0);
    f as i64 | (m.is_some() as i64) << 8 | (m.map_or(0, |p| p.0) as i64) << 16
}

/// invwrap_diff --fan 66: screen_drawn 0x92DCC while a pick-up's inventory screen runs (play::pick_screen_drawn with
/// fan fix F66 from the switch mask, #70 row 90).
#[no_mangle]
pub extern "C" fn ja1f_pick_screen_drawn(drawn: u8, from_pick: i32) -> i32 {
    crate::play::pick_screen_drawn(drawn, from_pick != 0, ja1logic::fanfix::on(66)) as i32
}

/// payday_diff: the mouse place of a morning host op (mouselimit::place_of_op); 0 = no move.
///
/// # Safety
/// `out` points to 2 writable i32.
#[no_mangle]
pub unsafe extern "C" fn ja1f_mouse_op_place(op: i32, out: *mut i32) -> i32 {
    match mouselimit::place_of_op(op) {
        Some((x, y)) => {
            *out = x;
            *out.add(1) = y;
            1
        }
        None => 0,
    }
}

/// payroll_diff, payday_diff, xp_diff: the limit that a host op of the morning sets. Returns 1 and fills `out`, or 0.
///
/// # Safety
/// `out` points to 4 writable i32.
#[no_mangle]
pub unsafe extern "C" fn ja1f_mouse_op_limit(op: i32, out: *mut i32) -> i32 {
    match mouselimit::of_op(op) {
        Some(l) => {
            *out = l.0;
            *out.add(1) = l.1;
            *out.add(2) = l.2;
            *out.add(3) = l.3;
            1
        }
        None => 0,
    }
}

/// guard_diff: one pass of the map loops' mouse limit rule (`MapLimit::step`) with the mouse in row `y`. `below` is the state
/// between passes. Returns 1 and fills `out` when the pass sets a limit, or 0.
///
/// # Safety
/// `below` points to one writable byte, `out` to 4 writable i32.
#[no_mangle]
pub unsafe extern "C" fn ja1f_map_limit_step(below: *mut u8, y: i32, out: *mut i32) -> i32 {
    let mut m = mouselimit::MapLimit { below: *below != 0 };
    let r = m.step(y);
    *below = m.below as u8;
    match r {
        Some(l) => {
            *out = l.0;
            *out.add(1) = l.1;
            *out.add(2) = l.2;
            *out.add(3) = l.3;
            1
        }
        None => 0,
    }
}

/// facelight_diff: 0x46A4C, frame picture `e` over a face buffer of 1024 bytes.
///
/// # Safety
/// `iso`, `exe`: C strings; `table0`: 256 bytes; `face`: 1024 bytes.
#[no_mangle]
pub unsafe extern "C" fn ja1f_face_light(iso: *const c_char, exe: *const c_char, table0: *const u8, face: *mut u8, e: i32) -> i32 {
    let (Some(iso), Some(exe)) = (cstr(iso), cstr(exe)) else { return E_ARG };
    let (t, f) = (table0 as usize, face as usize);
    guard(move || {
        let d = match panel::panel_data(iso, exe) {
            Ok(d) => d,
            Err(c) => return c,
        };
        let t = &*(t as *const [u8; 256]);
        panel::face_light(&d, t, std::slice::from_raw_parts_mut(f as *mut u8, 1024), e as u8);
        0
    })
}

/// facelight_diff: the face buffers of a new `Screen` after a quick restore (8 x 1024 bytes).
///
/// # Safety
/// `iso`, `exe`, `day`: C strings; `q`: `q_len` bytes; `out`: 8192 bytes.
#[no_mangle]
pub unsafe extern "C" fn ja1f_screen_faces(iso: *const c_char, exe: *const c_char, day: *const c_char, q: *const u8, q_len: usize, out: *mut u8) -> i32 {
    let (Some(iso), Some(exe), Some(day)) = (cstr(iso), cstr(exe), cstr(day)) else { return E_ARG };
    let (q, out) = (q as usize, out as usize);
    guard(move || {
        let jagged = match Iso::open(iso).and_then(|mut i| i.read("DAT/JAGGED.DAT")) {
            Ok(j) => j,
            Err(_) => return E_FILE,
        };
        let mut w = match world::World::load(iso, exe, day) {
            Ok(w) => w,
            Err(_) => return E_FILE,
        };
        if quicksave::restore(&mut w, &jagged, std::slice::from_raw_parts(q as *const u8, q_len)).is_err() {
            return E_FILE;
        }
        match screen::Screen::new(iso, exe, &mut w) {
            Ok(sc) => {
                std::slice::from_raw_parts_mut(out as *mut u8, 8192).copy_from_slice(&sc.faces);
                0
            }
            Err(c) => c,
        }
    })
}

/// `v`: valid Ja1fStrips (faces 8 x 1024, soldiers 48 x 512, table0 256); `panel`, `screen`: 320 x 200 bytes each;
/// `out`: valid Ja1fStripsOut.
#[no_mangle]
pub unsafe extern "C" fn ja1f_draw_strips(
    iso_path: *const c_char,
    exe_path: *const c_char,
    v: *const Ja1fStrips,
    mouse_x_in: i32,
    panel: *mut u8,
    screen: *mut u8,
    out: *mut Ja1fStripsOut,
) -> i32 {
    let (Some(iso), Some(exe)) = (cstr(iso_path), cstr(exe_path)) else { return E_ARG };
    if v.is_null() || panel.is_null() || screen.is_null() || out.is_null() {
        return E_ARG;
    }
    let (v, panel, screen, out) = (v as usize, panel as usize, screen as usize, out as usize);
    guard(move || {
        let v = &*(v as *const Ja1fStrips);
        let p = &v.panel;
        if !(0..=9).contains(&v.tileset) || p.faces.is_null() || p.soldiers.is_null() || p.table0.is_null() {
            return E_ARG;
        }
        let (c, d, id) = match (common(iso), panel::panel_data(iso, exe), invdraw::inv_data(iso, exe)) {
            (Ok(c), Ok(d), Ok(id)) => (c, d, id),
            (Err(e), _, _) | (_, Err(e), _) | (_, _, Err(e)) => return e,
        };
        let t = match tileset_data(iso, v.tileset as u8) {
            Ok(t) => t,
            Err(e) => return e,
        };
        let faces = std::slice::from_raw_parts(p.faces, 8 * 1024);
        let soldiers = std::slice::from_raw_parts_mut(p.soldiers, 48 * REC);
        let table0 = &mut *(p.table0 as *mut [u8; 256]);
        let buf = std::slice::from_raw_parts_mut(panel as *mut u8, (panel::PW * panel::PH) as usize);
        let scr = std::slice::from_raw_parts_mut(screen as *mut u8, (panel::PW * panel::PH) as usize);
        let env = strips::StripsEnv {
            top: v.top,
            keep: v.keep,
            opt: p.opt_onscreen,
            combat: p.combat,
            sector: v.sector,
            mercs: v.mercs,
            enemies: v.enemies,
            strip_flag: v.strip_flag,
            selected: p.selected,
            turn_slot: v.turn_slot,
            sel_in_play: v.sel_in_play,
            small_panel: v.small_panel,
            time_w: v.time_w,
            speaker: p.speaker,
            hover: p.hover,
            ap_warned: p.ap_warned,
            faces,
            tileset: &t,
            switch_as_normal: false,
            shade_last: 0,
        };
        let (mut blink, mut seen) = (v.blink, v.combat_seen);
        let mut painter = panel::Painter { d: &d, buf, table0, table29: panel::table29(&c) };
        let r = strips::draw_strips(&mut painter, &id, &env, &mut blink, &mut seen, mouse_x_in, soldiers, scr);
        let o = &mut *(out as *mut Ja1fStripsOut);
        o.load_result = r.load_result;
        o.blink = blink;
        o.combat_seen = seen;
        o.mouse_x = r.mouse_x;
        0
    })
}

/// The island map picture (strategic.md "The island map picture"): `sectors` 60 x 9 bytes, `marks` 60 signed
/// bytes, `screen` 320 x 200 (out), `palette` 768 bytes (out, 6-bit values).
///
/// # Safety
/// `iso`, `exe`: NUL-terminated paths; the buffers have the sizes above.
#[no_mangle]
pub unsafe extern "C" fn ja1f_island_map(
    iso: *const c_char,
    exe: *const c_char,
    sectors: *const u8,
    marks: *const i8,
    screen: *mut u8,
    palette: *mut u8,
) -> i32 {
    let (Some(iso), Some(exe)) = (cstr(iso), cstr(exe)) else { return E_ARG };
    if sectors.is_null() || marks.is_null() || screen.is_null() || palette.is_null() {
        return E_ARG;
    }
    let (sectors, marks, screen, palette) = (sectors as usize, marks as usize, screen as usize, palette as usize);
    guard(move || {
        let sec = std::slice::from_raw_parts(sectors as *const u8, 540);
        let mk = std::slice::from_raw_parts(marks as *const i8, 60);
        match island::island_cells(iso, exe, sec, mk) {
            Ok((s, p)) => {
                std::slice::from_raw_parts_mut(screen as *mut u8, 64000).copy_from_slice(&s);
                std::slice::from_raw_parts_mut(palette as *mut u8, 768).copy_from_slice(&p);
                0
            }
            Err(e) => e,
        }
    })
}

/// InitStrategicMap 0x43ECC: the whole opening picture of the island map (mapopen_diff).
///
/// # Safety
/// `sectors` = 540 bytes, `marks` = 60 bytes, `screen` = 64000 bytes, `palette` = 768 bytes.
#[no_mangle]
pub unsafe extern "C" fn ja1f_map_open(
    iso: *const c_char,
    exe: *const c_char,
    sectors: *const u8,
    marks: *const i8,
    screen: *mut u8,
    palette: *mut u8,
) -> i32 {
    let (Some(iso), Some(exe)) = (cstr(iso), cstr(exe)) else { return E_ARG };
    if sectors.is_null() || marks.is_null() || screen.is_null() || palette.is_null() {
        return E_ARG;
    }
    let (sectors, marks, screen, palette) = (sectors as usize, marks as usize, screen as usize, palette as usize);
    guard(move || {
        let sec = std::slice::from_raw_parts(sectors as *const u8, 540);
        let mk = std::slice::from_raw_parts(marks as *const i8, 60);
        match island::island_map(iso, exe, sec, mk) {
            Ok((s, p)) => {
                std::slice::from_raw_parts_mut(screen as *mut u8, 64000).copy_from_slice(&s);
                std::slice::from_raw_parts_mut(palette as *mut u8, 768).copy_from_slice(&p);
                0
            }
            Err(e) => e,
        }
    })
}

/// strategic.md "The replay pictures": replay_draw (k, a) on R (`r` 60 x 9 bytes) with the given screen and clean
/// map (320 x 200 each, in and out); `waits` (out) = the sum of the waits of the draw (ms).
///
/// # Safety
/// `iso`, `exe`: NUL-terminated paths; the buffers have the sizes above.
#[no_mangle]
pub unsafe extern "C" fn ja1f_replay_draw(
    iso: *const c_char,
    exe: *const c_char,
    r: *const u8,
    k: i32,
    a: i32,
    screen: *mut u8,
    clean: *mut u8,
    waits: *mut i32,
) -> i32 {
    let (Some(iso), Some(exe)) = (cstr(iso), cstr(exe)) else { return E_ARG };
    if r.is_null() || screen.is_null() || clean.is_null() || waits.is_null() {
        return E_ARG;
    }
    let (r, screen, clean, waits) = (r as usize, screen as usize, clean as usize, waits as usize);
    guard(move || {
        let r = std::slice::from_raw_parts(r as *const u8, 540);
        let scr = std::slice::from_raw_parts_mut(screen as *mut u8, 64000);
        let cln = std::slice::from_raw_parts_mut(clean as *mut u8, 64000);
        let mut p = match replaypic::ReplayPic::new(iso, exe, r) {
            Ok(p) => p,
            Err(e) => return e,
        };
        p.screen.copy_from_slice(scr);
        p.set_clean(cln);
        p.draw(k, a, r);
        scr.copy_from_slice(&p.screen);
        cln.copy_from_slice(p.clean());
        *(waits as *mut i32) = p.out.iter().map(|e| if let play::ReplayEv::Wait(ms) = e { *ms as i32 } else { 0 }).sum();
        0
    })
}

/// strategic.md "The replay pictures": replay_anim k (0 flags, 1 march, 3 burn, 4 explosion) with arguments `a`
/// (5 values) and the carry on R (`r` 60 x 9 bytes) with the given screen and clean map (320 x 200 each, in and
/// out); `waits` (out) = the sum of the waits of the animation (ms).
///
/// # Safety
/// `iso`, `exe`: NUL-terminated paths; `a` holds 5 values; the buffers have the sizes above.
#[no_mangle]
pub unsafe extern "C" fn ja1f_replay_anim(
    iso: *const c_char,
    exe: *const c_char,
    r: *const u8,
    k: i32,
    a: *const i32,
    carry: i32,
    screen: *mut u8,
    clean: *mut u8,
    waits: *mut i32,
) -> i32 {
    let (Some(iso), Some(exe)) = (cstr(iso), cstr(exe)) else { return E_ARG };
    if r.is_null() || a.is_null() || screen.is_null() || clean.is_null() || waits.is_null() {
        return E_ARG;
    }
    let (r, a, screen, clean, waits) = (r as usize, a as usize, screen as usize, clean as usize, waits as usize);
    guard(move || {
        let r = std::slice::from_raw_parts(r as *const u8, 540);
        let a = std::slice::from_raw_parts(a as *const i32, 5);
        let scr = std::slice::from_raw_parts_mut(screen as *mut u8, 64000);
        let cln = std::slice::from_raw_parts_mut(clean as *mut u8, 64000);
        let mut p = match replaypic::ReplayPic::new(iso, exe, r) {
            Ok(p) => p,
            Err(e) => return e,
        };
        p.screen.copy_from_slice(scr);
        p.set_clean(cln);
        p.carry = carry as u8;
        p.anim(k, [a[0], a[1], a[2], a[3], a[4]], r);
        scr.copy_from_slice(&p.screen);
        cln.copy_from_slice(p.clean());
        *(waits as *mut i32) = p.out.iter().map(|e| if let play::ReplayEv::Wait(ms) = e { *ms as i32 } else { 0 }).sum();
        0
    })
}

/// The wrapper of the inventory screen 0x54D50 after the screen: invscreen::after_screen + wrapper_end, the code the
/// window runs. `opener` = the merc that opened the screen, `mode` = the view mode, `ends` = panel ends (in and out).
///
/// # Safety
/// `w` / `h` valid library world and host; `ends` one byte.
#[no_mangle]
pub unsafe extern "C" fn ja1f_inv_after(w: *mut host::Ja1World, h: *const host::Ja1Host, opener: i32, mode: i32, ends: *mut u8) -> i32 {
    if w.is_null() || h.is_null() || ends.is_null() || !(0..48).contains(&opener) {
        return E_ARG;
    }
    let (wp, hp, ep) = (w as usize, h as usize, ends as usize);
    guard(move || {
        invscreen::after_screen(wp as *mut host::Ja1World, hp as *const host::Ja1Host, opener as usize);
        invscreen::wrapper_end(mode as u8, &mut *(ep as *mut u8));
        0
    })
}

/// ReplayScreenInit? 0x64274: the opening picture of the day replay, the code `day_replay` runs. `r` = replay table
/// (540), `accounts` (0xA8 bytes, after the day-end accounts), `v` = 7 i32 (mappanel::replay_day). Out: `screen` and
/// `clean` 64000 bytes, `palette` 768 (8-bit), `fade` 25 x 768. Result: the number of fade levels, or < 0.
///
/// # Safety
/// `iso`, `exe`: NUL-terminated paths; the buffers have the sizes above.
#[no_mangle]
pub unsafe extern "C" fn ja1f_replay_init(iso: *const c_char, exe: *const c_char, r: *const u8, accounts: *const u8, v: *const i32,
                                          screen: *mut u8, clean: *mut u8, palette: *mut u8, fade: *mut u8) -> i32 {
    let (Some(iso), Some(exe)) = (cstr(iso), cstr(exe)) else { return E_ARG };
    if r.is_null() || accounts.is_null() || v.is_null() || screen.is_null() || clean.is_null() || palette.is_null() || fade.is_null() {
        return E_ARG;
    }
    let (r, accounts, v, screen, clean, palette, fade) = (r as usize, accounts as usize, v as usize, screen as usize, clean as usize, palette as usize, fade as usize);
    guard(move || {
        let r = std::slice::from_raw_parts(r as *const u8, 540);
        let acc = std::slice::from_raw_parts(accounts as *const u8, 0xA8);
        let mut vv = [0i32; 7];
        vv.copy_from_slice(std::slice::from_raw_parts(v as *const i32, 7));
        let mut p = match replaypic::ReplayPic::new(iso, exe, r) {
            Ok(p) => p,
            Err(e) => return e,
        };
        p.panel_numbers(acc, mappanel::replay_day(acc, vv));
        std::slice::from_raw_parts_mut(screen as *mut u8, 64000).copy_from_slice(&p.start);
        std::slice::from_raw_parts_mut(clean as *mut u8, 64000).copy_from_slice(p.clean());
        std::slice::from_raw_parts_mut(palette as *mut u8, 768).copy_from_slice(&p.palette);
        let levels = p.fade_in();
        let out = std::slice::from_raw_parts_mut(fade as *mut u8, 25 * 768);
        for (i, l) in levels.iter().take(25).enumerate() {
            out[i * 768..(i + 1) * 768].copy_from_slice(l);
        }
        levels.len() as i32
    })
}

/// The sounds of host op 355 replay_anim k with arguments `a` (5 values): "NAME VOL\n" per sound into `out` (`cap`
/// bytes). Result: the bytes written, or < 0.
///
/// # Safety
/// `iso`, `exe`: NUL-terminated paths; `r` 540 bytes; `a` holds 5 values; `out` `cap` bytes.
#[no_mangle]
pub unsafe extern "C" fn ja1f_replay_sounds(iso: *const c_char, exe: *const c_char, r: *const u8, k: i32, a: *const i32, carry: i32,
                                            out: *mut u8, cap: i32) -> i32 {
    let (Some(iso), Some(exe)) = (cstr(iso), cstr(exe)) else { return E_ARG };
    if r.is_null() || a.is_null() || out.is_null() {
        return E_ARG;
    }
    let (r, a, out) = (r as usize, a as usize, out as usize);
    guard(move || {
        let r = std::slice::from_raw_parts(r as *const u8, 540);
        let a = std::slice::from_raw_parts(a as *const i32, 5);
        let mut p = match replaypic::ReplayPic::new(iso, exe, r) {
            Ok(p) => p,
            Err(e) => return e,
        };
        p.carry = carry as u8;
        p.anim(k, [a[0], a[1], a[2], a[3], a[4]], r);
        let mut t = String::new();
        for e in &p.out {
            if let play::ReplayEv::Sound(name, vol) = e {
                t.push_str(&format!("{} {}\n", name, vol));
            }
        }
        let n = t.len().min(cap as usize);
        std::slice::from_raw_parts_mut(out as *mut u8, n).copy_from_slice(&t.as_bytes()[..n]);
        n as i32
    })
}

/// strategic.md "The replay pictures": host op 355 replay_anim (k, call i) for the `n` calls `a` (5 values each) on
/// one picture (the battle scratch keeps its pixels from call to call), R `r` (60 x 9 bytes), the screen and clean
/// map (in and out). Each shown frame: its screen into `frames` (64000 bytes each, at most `cap`) and its wait (ms,
/// 0 when none follows) into `waits`. Result: the number of frames (more than `cap`: only `cap` written), or < 0.
///
/// # Safety
/// `iso`, `exe`: NUL-terminated paths; `a` holds 5 n values; `frames` 64000 cap bytes, `waits` cap values.
#[no_mangle]
pub unsafe extern "C" fn ja1f_replay_seq(
    iso: *const c_char,
    exe: *const c_char,
    r: *const u8,
    k: i32,
    a: *const i32,
    n: i32,
    screen: *mut u8,
    clean: *mut u8,
    frames: *mut u8,
    waits: *mut i32,
    cap: i32,
) -> i32 {
    let (Some(iso), Some(exe)) = (cstr(iso), cstr(exe)) else { return E_ARG };
    if r.is_null() || a.is_null() || screen.is_null() || clean.is_null() || frames.is_null() || waits.is_null() || n < 0 || cap < 0 {
        return E_ARG;
    }
    let (r, a, screen, clean, frames, waits) = (r as usize, a as usize, screen as usize, clean as usize, frames as usize, waits as usize);
    guard(move || {
        let r = std::slice::from_raw_parts(r as *const u8, 540);
        let a = std::slice::from_raw_parts(a as *const i32, 5 * n as usize);
        let scr = std::slice::from_raw_parts_mut(screen as *mut u8, 64000);
        let cln = std::slice::from_raw_parts_mut(clean as *mut u8, 64000);
        let frs = std::slice::from_raw_parts_mut(frames as *mut u8, 64000 * cap as usize);
        let ws = std::slice::from_raw_parts_mut(waits as *mut i32, cap as usize);
        let mut p = match replaypic::ReplayPic::new(iso, exe, r) {
            Ok(p) => p,
            Err(e) => return e,
        };
        p.screen.copy_from_slice(scr);
        p.set_clean(cln);
        for c in a.chunks(5) {
            p.anim(k, [c[0], c[1], c[2], c[3], c[4]], r);
        }
        scr.copy_from_slice(&p.screen);
        cln.copy_from_slice(p.clean());
        let mut got = 0usize;
        let shown = play::replay_shown(&p.out);
        for (ev, &on) in p.out.iter().zip(&shown) {
            match ev {
                play::ReplayEv::Frame(_) if !on => {}
                play::ReplayEv::Frame(f) => {
                    if got < cap as usize {
                        frs[got * 64000..(got + 1) * 64000].copy_from_slice(f);
                        ws[got] = 0;
                    }
                    got += 1;
                }
                play::ReplayEv::Wait(ms) if got > 0 && got <= cap as usize => ws[got - 1] += *ms as i32,
                _ => {}
            }
        }
        got as i32
    })
}

/// strategic.md "The map panel": drawing the panel (MAPPAN.PCC, the raised tab of `mode`, number fields 0..14).
#[no_mangle]
pub unsafe extern "C" fn ja1f_map_panel(
    iso: *const c_char,
    exe: *const c_char,
    accounts: *const u8,
    team_on_duty: i32,
    mode: i32,
    screen: *mut u8,
) -> i32 {
    map_panel_call(iso, exe, accounts, team_on_duty, mode, None, screen)
}

/// strategic.md "The map panel": number field k (0..15), 16 = the pressed wage number, 17 = wage box step 4.
#[no_mangle]
pub unsafe extern "C" fn ja1f_map_field(
    iso: *const c_char,
    exe: *const c_char,
    accounts: *const u8,
    team_on_duty: i32,
    k: i32,
    screen: *mut u8,
) -> i32 {
    map_panel_call(iso, exe, accounts, team_on_duty, 0, Some(k), screen)
}

unsafe fn map_panel_call(
    iso: *const c_char,
    exe: *const c_char,
    accounts: *const u8,
    on_duty: i32,
    mode: i32,
    what: Option<i32>,
    screen: *mut u8,
) -> i32 {
    let (Some(iso), Some(exe)) = (cstr(iso), cstr(exe)) else { return E_ARG };
    if accounts.is_null() || screen.is_null() {
        return E_ARG;
    }
    let (accounts, screen) = (accounts as usize, screen as usize);
    guard(move || {
        let acc = std::slice::from_raw_parts(accounts as *const u8, 0xA8);
        let scr = std::slice::from_raw_parts_mut(screen as *mut u8, 64000);
        match mappanel::draw(iso, exe, acc, on_duty, mode, what, scr) {
            Ok(()) => 0,
            Err(e) => e,
        }
    })
}

/// strategic.md "The day-end accounts": the replay panel's number fields 31..42 on `screen` (320 x 200, in/out);
/// `day` = 6 i32 (fields 31, 32, 33, 40, 41, 42).
#[no_mangle]
pub unsafe extern "C" fn ja1f_replay_fields(iso: *const c_char, exe: *const c_char, accounts: *const u8, day: *const i32,
                                            screen: *mut u8) -> i32 {
    let (Some(iso), Some(exe)) = (cstr(iso), cstr(exe)) else { return E_ARG };
    if accounts.is_null() || day.is_null() || screen.is_null() {
        return E_ARG;
    }
    let (accounts, day, screen) = (accounts as usize, day as usize, screen as usize);
    guard(move || {
        let acc = std::slice::from_raw_parts(accounts as *const u8, 0xA8);
        let mut d = [0i32; 6];
        d.copy_from_slice(std::slice::from_raw_parts(day as *const i32, 6));
        let scr = std::slice::from_raw_parts_mut(screen as *mut u8, 64000);
        let (Ok(c), Ok(pd)) = (crate::common(iso), crate::panel::panel_data(iso, exe)) else { return crate::E_FILE };
        let mut table0 = c.remap[0];
        let mut p = crate::panel::Painter { d: &pd, buf: scr, table0: &mut table0, table29: crate::panel::table29(&c) };
        mappanel::replay_fields(&mut p, acc, d);
        0
    })
}

/// strategic.md "The processing plants": steps 1..3 on `screen` (320 x 200, in/out).
#[no_mangle]
pub unsafe extern "C" fn ja1f_processing(
    iso: *const c_char,
    exe: *const c_char,
    accounts: *const u8,
    plants: *const u8,
    sectors: *const u8,
    screen: *mut u8,
) -> i32 {
    let (Some(iso), Some(exe)) = (cstr(iso), cstr(exe)) else { return E_ARG };
    if accounts.is_null() || plants.is_null() || sectors.is_null() || screen.is_null() {
        return E_ARG;
    }
    let (accounts, plants, sectors, screen) = (accounts as usize, plants as usize, sectors as usize, screen as usize);
    guard(move || {
        let acc = std::slice::from_raw_parts(accounts as *const u8, 0xA8);
        let pl = std::slice::from_raw_parts(plants as *const u8, 48);
        let sec = std::slice::from_raw_parts(sectors as *const u8, 540);
        let scr = std::slice::from_raw_parts_mut(screen as *mut u8, 64000);
        match mappanel::draw_processing(iso, exe, acc, pl, sec, scr) {
            Ok(()) => 0,
            Err(e) => e,
        }
    })
}

/// strategic.md "Mouse over the island map": the sector under the mouse at (x, y), -1 when y >= 141.
#[no_mangle]
pub extern "C" fn ja1f_map_sector(x: i32, y: i32) -> i32 {
    island::sector_at(x, y)
}

/// The frame of sector (0..59) with `colour` drawn on `screen` (320 x 200, in/out). 0 = done, < 0 = error.
///
/// # Safety
/// `iso`, `exe`: NUL-terminated paths; `screen` has 64000 bytes.
#[no_mangle]
pub unsafe extern "C" fn ja1f_map_frame(iso: *const c_char, exe: *const c_char, sector: i32, colour: i32, screen: *mut u8) -> i32 {
    let (Some(iso), Some(exe)) = (cstr(iso), cstr(exe)) else { return E_ARG };
    if screen.is_null() {
        return E_ARG;
    }
    let screen = screen as usize;
    guard(move || {
        let scr = std::slice::from_raw_parts_mut(screen as *mut u8, 64000);
        match island::sector_frame(iso, exe, sector, colour as u8, scr) {
            Ok(()) => 0,
            Err(e) => e,
        }
    })
}

/// panel.md "Message box": the box of `text` (zero-ended) with darkening table `table` (not 28) drawn on `screen`
/// (320 x 200, in/out). 0 = done, < 0 = error.
///
/// # Safety
/// `iso`, `exe`, `text`: NUL-terminated; `screen` has 64000 bytes.
#[no_mangle]
pub unsafe extern "C" fn ja1f_message_box(iso: *const c_char, exe: *const c_char, text: *const c_char, table: i32,
                                          screen: *mut u8) -> i32 {
    let (Some(iso), Some(exe)) = (cstr(iso), cstr(exe)) else { return E_ARG };
    if text.is_null() || screen.is_null() || !(0..34).contains(&table) || table == 28 {
        return E_ARG;
    }
    let text = std::ffi::CStr::from_ptr(text).to_bytes().to_vec();
    let screen = screen as usize;
    guard(move || {
        let scr = std::slice::from_raw_parts_mut(screen as *mut u8, 64000);
        let (c, d) = match (common(iso), panel::panel_data(iso, exe)) {
            (Ok(c), Ok(d)) => (c, d),
            (Err(e), _) | (_, Err(e)) => return e,
        };
        let mut table0 = c.remap[0];
        let mut p = panel::Painter { d: &d, buf: scr, table0: &mut table0, table29: panel::table29(&c) };
        msgbox::draw(&mut p, &text, &c.remap[table as usize], msgbox::frame_colour(table as usize));
        0
    })
}

/// strategic.md "The battle box": the box of host op 334 battle_box (s, enemies, guards, tappers, kind) with darkening
/// table `table` (0..33 except 28) on `screen` (320 x 200, the tactical view as shown; in/out). 0 = done, < 0 = error.
///
/// # Safety
/// `iso`, `exe`: NUL-terminated; `screen` has 64000 bytes.
#[no_mangle]
#[allow(clippy::too_many_arguments)]
pub unsafe extern "C" fn ja1f_battle_box(iso: *const c_char, exe: *const c_char, table: i32, s: i32, enemies: i32,
                                         guards: i32, tappers: i32, kind: i32, screen: *mut u8) -> i32 {
    let (Some(iso), Some(exe)) = (cstr(iso), cstr(exe)) else { return E_ARG };
    if screen.is_null() || !(0..34).contains(&table) || table == 28 {
        return E_ARG;
    }
    let screen = screen as usize;
    guard(move || {
        let scr = std::slice::from_raw_parts_mut(screen as *mut u8, 64000);
        let (c, d) = match (common(iso), panel::panel_data(iso, exe)) {
            (Ok(c), Ok(d)) => (c, d),
            (Err(e), _) | (_, Err(e)) => return e,
        };
        let mut table0 = c.remap[0];
        let mut p = panel::Painter { d: &d, buf: scr, table0: &mut table0, table29: panel::table29(&c) };
        battlebox::draw(&mut p, &c.remap[table as usize], s, enemies as u8, guards as u8, tappers as u8, kind);
        0
    })
}

/// panel.md "Question box": steps 2 + 3 for question `text` and MAIN.DAT picture `pic` (NULL = the frame only) on
/// `screen` (320 x 200, in/out); `pressed` 0 / 1 then draws the pressed face of button 5 / 6 (-1 = none). 0 = done,
/// < 0 = error.
///
/// # Safety
/// `iso`, `exe`, `text`, `pic` (when not NULL): NUL-terminated; `screen` has 64000 bytes.
#[no_mangle]
pub unsafe extern "C" fn ja1f_question(iso: *const c_char, exe: *const c_char, text: *const c_char,
                                       pic: *const c_char, pressed: i32, screen: *mut u8, item: i32, item2: i32) -> i32 {
    let (Some(iso), Some(exe)) = (cstr(iso), cstr(exe)) else { return E_ARG };
    if text.is_null() || screen.is_null() || !(-1..2).contains(&pressed) {
        return E_ARG;
    }
    let pic = if pic.is_null() { None } else { cstr(pic).map(|s| s.to_string()) };
    let text = std::ffi::CStr::from_ptr(text).to_bytes().to_vec();
    let screen = screen as usize;
    guard(move || {
        let scr = std::slice::from_raw_parts_mut(screen as *mut u8, 64000);
        let (c, d, inv) = match (common(iso), panel::panel_data(iso, exe), invdraw::inv_data(iso, exe)) {
            (Ok(c), Ok(d), Ok(i)) => (c, d, i),
            (Err(e), _, _) | (_, Err(e), _) | (_, _, Err(e)) => return e,
        };
        let mut table0 = c.remap[0];
        let mut p = panel::Painter { d: &d, buf: scr, table0: &mut table0, table29: panel::table29(&c) };
        let it = (item >= 0).then_some(((item & 0xFF) as u8, item >> 16 & 0xFF, item >> 24 & 0x7F, (item >> 8 & 0xFF) as u8));
        let mut items: Vec<question::BoxItem> = it.into_iter().collect();
        if item2 >= 0 {
            items.push(((item2 & 0xFF) as u8, item2 >> 16 & 0xFF, item2 >> 24 & 0x7F, (item2 >> 8 & 0xFF) as u8));
        }
        question::draw_items(&mut p, &inv, &text, pic.as_deref(), &items, "PANEL_P.PCC");
        if pressed >= 0 {
            question::button(&mut p, pressed as usize, true);
        }
        0
    })
}

#[no_mangle]
pub unsafe extern "C" fn ja1f_prompt_mouse(sub: i32, x: i32, y: i32, stack: i32, dx: i32, dy: i32, out: *mut i32) -> i32 {
    // The mouse calls of YesNoPrompt? 0x28C84: out[2 n], out[2 n + 1] = the place of call n; returns the number.
    if out.is_null() {
        return E_ARG;
    }
    let mut n = 0;
    let moved = question::prompt_warp(sub != 0, (x, y));
    if let Some(at) = moved {
        *out = at.0;
        *out.add(1) = at.1;
        n = 1;
    }
    let after = moved.unwrap_or((x, y));
    if stack != 0 && question::prompt_back(after, (after.0 + dx, after.1 + dy)) {
        *out.add(2 * n) = x;
        *out.add(2 * n + 1) = y;
        n += 1;
    }
    n as i32
}

/// The mouse calls of ItemInfoBox 0x25D20 for iteminfo_diff: out[3 n .. 3 n + 2] = (0 save | 1 move | 2 back, x, y);
/// the mouse goes to the place `(dx, dy)` from its place after the move before the box ends. Returns the number of calls.
#[no_mangle]
pub unsafe extern "C" fn ja1f_item_mouse(opt_info: i32, m: i32, picture: i32, x: i32, y: i32, dx: i32, dy: i32,
                                          out: *mut i32) -> i32 {
    if out.is_null() {
        return E_ARG;
    }
    if picture != 0 || opt_info == 0 {
        return 0;
    }
    let mut n = 0;
    let mut put = |kind: i32, at: (i32, i32)| {
        *out.add(3 * n) = kind;
        *out.add(3 * n + 1) = at.0;
        *out.add(3 * n + 2) = at.1;
        n += 1;
    };
    if m != 0 {
        put(0, (x, y));
    }
    let moved = iteminfo::mouse_open(m != 0, (x, y));
    if let Some(at) = moved {
        put(1, at);
    }
    let after = moved.unwrap_or((x, y));
    if let Some(at) = iteminfo::mouse_close(m != 0, after, (after.0 + dx, after.1 + dy)) {
        put(2, at);
    }
    n as i32
}

/// The merc-face question box (docs/spec/panel.md "Merc-face question box") on `screen` (320 x 200): text, profile,
/// nickname, `office` ≠ 0 = PANEL_B.PCC; `pressed` 0 / 1 = that button's pressed face after (−1 = none); −2 = the box with the text only (ECX 0).
#[no_mangle]
#[allow(clippy::too_many_arguments)]
pub unsafe extern "C" fn ja1f_face_question(iso: *const c_char, exe: *const c_char, text: *const c_char, profile: i32,
                                            nick: *const c_char, office: i32, pressed: i32, screen: *mut u8) -> i32 {
    let (Some(iso), Some(exe)) = (cstr(iso), cstr(exe)) else { return E_ARG };
    if text.is_null() || nick.is_null() || screen.is_null() || !(-2..2).contains(&pressed) {
        return E_ARG;
    }
    let text = std::ffi::CStr::from_ptr(text).to_bytes().to_vec();
    let nick = std::ffi::CStr::from_ptr(nick).to_bytes().to_vec();
    let screen = screen as usize;
    guard(move || {
        let scr = std::slice::from_raw_parts_mut(screen as *mut u8, 64000);
        let (c, d, inv) = match (common(iso), panel::panel_data(iso, exe), invdraw::inv_data(iso, exe)) {
            (Ok(c), Ok(d), Ok(i)) => (c, d, i),
            (Err(e), _, _) | (_, Err(e), _) | (_, _, Err(e)) => return e,
        };
        let mut table0 = c.remap[0];
        let mut p = panel::Painter { d: &d, buf: scr, table0: &mut table0, table29: panel::table29(&c) };
        let panel = if office != 0 { "PANEL_B.PCC" } else { "PANEL_P.PCC" };
        if pressed == -2 {
            question::draw_face_text(&mut p, &inv, &text, profile as u8, &nick, panel);
        } else {
            question::draw_face(&mut p, &inv, &text, profile as u8, &nick, panel);
        }
        if pressed >= 0 {
            question::button(&mut p, pressed as usize, true);
        }
        0
    })
}

/// The salary box (strategic.md "Salary raise", host op 297) over `screen` (320 x 200): profile, nickname, the start
/// offer, then the presses `keys` (0 NO, 1 OK, 2 down, 3 up; a press of a button that is not tested does nothing).
/// `frames` gets n screens: the screen before each press. After the presses, `pressed` >= 0 draws that button's
/// pressed face on `screen`. Returns the box's offer (NO = 0), or -100000 when no press ended the box.
#[no_mangle]
#[allow(clippy::too_many_arguments)]
pub unsafe extern "C" fn ja1f_salary(iso: *const c_char, exe: *const c_char, profile: i32, nick: *const c_char,
                                     start: i32, keys: *const i32, n: i32, pressed: i32, frames: *mut u8,
                                     screen: *mut u8) -> i32 {
    let (Some(iso), Some(exe)) = (cstr(iso), cstr(exe)) else { return E_ARG };
    if nick.is_null() || screen.is_null() || frames.is_null() || (n > 0 && keys.is_null()) || n < 0 {
        return E_ARG;
    }
    let nick = std::ffi::CStr::from_ptr(nick).to_bytes().to_vec();
    let keys: Vec<i32> = (0..n as usize).map(|i| *keys.add(i)).collect();
    let (screen, frames) = (screen as usize, frames as usize);
    guard(move || {
        let scr = std::slice::from_raw_parts_mut(screen as *mut u8, 64000);
        let out = std::slice::from_raw_parts_mut(frames as *mut u8, 64000 * keys.len());
        let (c, d, inv) = match (common(iso), panel::panel_data(iso, exe), invdraw::inv_data(iso, exe)) {
            (Ok(c), Ok(d), Ok(i)) => (c, d, i),
            (Err(e), _, _) | (_, Err(e), _) | (_, _, Err(e)) => return e,
        };
        let mut table0 = c.remap[0];
        let mut p = panel::Painter { d: &d, buf: scr, table0: &mut table0, table29: panel::table29(&c) };
        let mut b = salary::SalaryBox::new(&mut p, &inv, profile as u8, &nick, start as i16);
        b.show(p.buf);
        let mut r = -100000;
        for (i, &k) in keys.iter().enumerate() {
            out[64000 * i..64000 * (i + 1)].copy_from_slice(&p.buf[..64000]);
            if r == -100000 && (0..4).contains(&k) {
                if let Some(v) = b.press(&mut p, k as usize) {
                    r = v as i32;
                }
            }
        }
        if (0..4).contains(&pressed) {
            b.face(&mut p, pressed as usize, true);
        }
        r
    })
}

/// The item info box (docs/spec/panel.md "Item info box") on `screen` (320 x 200): profile, type, count (signed
/// byte), value (signed 16-bit), numbers flag a; `opts` bit 0 = Item Info, bit 1 = Subtitles; `info` = the mouse
/// flag (the mouse is not drawn here). Returns the result of the box (1 = a picture item: draw the screen again).
#[no_mangle]
#[allow(clippy::too_many_arguments)]
pub unsafe extern "C" fn ja1f_item_info(iso: *const c_char, exe: *const c_char, profile: i32, t: i32, count: i32,
                                        value: i32, a: i32, opts: i32, _info: i32, screen: *mut u8) -> i32 {
    let (Some(iso), Some(exe)) = (cstr(iso), cstr(exe)) else { return E_ARG };
    if screen.is_null() {
        return E_ARG;
    }
    let screen = screen as usize;
    guard(move || {
        let scr = std::slice::from_raw_parts_mut(screen as *mut u8, 64000);
        let (c, d, inv) = match (common(iso), panel::panel_data(iso, exe), invdraw::inv_data(iso, exe)) {
            (Ok(c), Ok(d), Ok(i)) => (c, d, i),
            (Err(e), _, _) | (_, Err(e), _) | (_, _, Err(e)) => return e,
        };
        let mut table0 = c.remap[0];
        let mut p = panel::Painter { d: &d, buf: scr, table0: &mut table0, table29: panel::table29(&c) };
        let i = iteminfo::Info { profile: profile as u8, t: t as u8, count: count as i8, value: value as i16,
                                 numbers: a == 1, opt_info: opts & 1 != 0, opt_sub: opts & 2 != 0 };
        iteminfo::draw(&mut p, &inv, i).result()
    })
}

/// The payroll screen (docs/spec/strategic.md "The payroll screen (host op 301)"): `screen` (320 x 200, the island
/// map before) becomes the screen at the end, `panel` the panel buffer at the end; `sounds` (8 i32) gets the page
/// sound and the slide sound (effect, rate, volume, times) in their order. Other arguments as ja1f_assign_map;
/// `prev` = `prev_balance`. Returns the number of shown screens.
#[no_mangle]
pub unsafe extern "C" fn ja1f_payroll(iso: *const c_char, exe: *const c_char, soldiers: *const u8, faces: *const u8,
                                      stat_changed: *const u8, accounts: *const u8, prev: i32, screen: *mut u8,
                                      panel: *mut u8, sounds: *mut i32) -> i32 {
    let (Some(iso), Some(exe)) = (cstr(iso), cstr(exe)) else { return E_ARG };
    if soldiers.is_null() || faces.is_null() || stat_changed.is_null() || accounts.is_null() || screen.is_null()
        || panel.is_null() || sounds.is_null() {
        return E_ARG;
    }
    let (so, fa, sc, ac, sr, pa, sd) = (soldiers as usize, faces as usize, stat_changed as usize, accounts as usize,
                                        screen as usize, panel as usize, sounds as usize);
    guard(move || {
        let so = std::slice::from_raw_parts(so as *const u8, 9 * assign::SOLDIER);
        let fa = std::slice::from_raw_parts(fa as *const u8, 8 * 1024);
        let sc = std::slice::from_raw_parts(sc as *const u8, 640);
        let ac = std::slice::from_raw_parts(ac as *const u8, 0xA8);
        let sr = std::slice::from_raw_parts_mut(sr as *mut u8, 64000);
        let pa = std::slice::from_raw_parts_mut(pa as *mut u8, 64000);
        let sd = std::slice::from_raw_parts_mut(sd as *mut i32, 8);
        let (c, d, inv) = match (common(iso), panel::panel_data(iso, exe), invdraw::inv_data(iso, exe)) {
            (Ok(c), Ok(d), Ok(i)) => (c, d, i),
            (Err(e), _, _) | (_, Err(e), _) | (_, _, Err(e)) => return e,
        };
        let mut table0 = c.remap[0];
        let mut p = panel::Painter { d: &d, buf: pa, table0: &mut table0, table29: panel::table29(&c) };
        let steps = payroll::run(&mut p, &inv, sr, so, fa, sc, ac, prev);
        let mut n = 0;
        for st in &steps {
            if let (Some((a, b, v, t)), true) = (st.sound, n < 2) {
                sd[4 * n..4 * n + 4].copy_from_slice(&[a, b, v, t]);
                n += 1;
            }
        }
        if let Some(last) = steps.last() {
            sr.copy_from_slice(&last.screen);
        }
        steps.len() as i32
    })
}

/// The assignment picture (docs/spec/strategic.md "The assignment picture") into `panel` (320 x 200).
/// `soldiers` = 9 soldier records (team slots 0..8; duties may change), `faces` = 8 face buffers of 32 x 32,
/// `stat_changed` = 640 bytes, `accounts` = the 0xA8 bytes of the day accounts. Returns 0.
#[no_mangle]
pub unsafe extern "C" fn ja1f_assign_map(iso: *const c_char, exe: *const c_char, soldiers: *mut u8,
                                         faces: *const u8, stat_changed: *const u8, accounts: *const u8,
                                         panel: *mut u8) -> i32 {
    let (Some(iso), Some(exe)) = (cstr(iso), cstr(exe)) else { return E_ARG };
    if soldiers.is_null() || faces.is_null() || stat_changed.is_null() || accounts.is_null() || panel.is_null() {
        return E_ARG;
    }
    let (so, fa, sc, ac, pa) = (soldiers as usize, faces as usize, stat_changed as usize, accounts as usize,
                                panel as usize);
    guard(move || {
        let so = std::slice::from_raw_parts_mut(so as *mut u8, 9 * assign::SOLDIER);
        let fa = std::slice::from_raw_parts(fa as *const u8, 8 * 1024);
        let sc = std::slice::from_raw_parts(sc as *const u8, 640);
        let ac = std::slice::from_raw_parts(ac as *const u8, 0xA8);
        let pa = std::slice::from_raw_parts_mut(pa as *mut u8, 64000);
        let (c, d, inv) = match (common(iso), panel::panel_data(iso, exe), invdraw::inv_data(iso, exe)) {
            (Ok(c), Ok(d), Ok(i)) => (c, d, i),
            (Err(e), _, _) | (_, Err(e), _) | (_, _, Err(e)) => return e,
        };
        let mut table0 = c.remap[0];
        let mut p = panel::Painter { d: &d, buf: pa, table0: &mut table0, table29: panel::table29(&c) };
        assign::draw(&mut p, &inv, &c.remap[30], so, fa, sc, ac);
        0
    })
}

/// The duty menu (strategic.md "The duty menu") driven by `n` input polls `events` (x, y, buttons per poll; poll
/// 0 = the press on a duty name box), from the assignment picture in `panel` / `screen`. After each poll j the
/// screen goes to `shots` + 64000 j. `result` = [polls used, chosen duty or -1, training subject or -1, 1 when
/// CalcLedgerAll would run (the caller counts the accounts; the picture is drawn with `accounts` as given),
/// mouse y of Open step 6 or -1]. `soldiers` = team slots 0..8 (9 records), changed by the choice.
#[no_mangle]
pub unsafe extern "C" fn ja1f_duty_script(iso: *const c_char, exe: *const c_char, soldiers: *mut u8,
                                          faces: *const u8, stat_changed: *const u8, accounts: *const u8,
                                          panel: *mut u8, screen: *mut u8, events: *const i32, n: i32,
                                          shots: *mut u8, result: *mut i32) -> i32 {
    let (Some(iso), Some(exe)) = (cstr(iso), cstr(exe)) else { return E_ARG };
    if soldiers.is_null() || faces.is_null() || stat_changed.is_null() || accounts.is_null() || panel.is_null()
        || screen.is_null() || events.is_null() || n < 1 || shots.is_null() || result.is_null() {
        return E_ARG;
    }
    let (so, fa, sc, ac, pa, scr, ev, sh, re) = (soldiers as usize, faces as usize, stat_changed as usize,
        accounts as usize, panel as usize, screen as usize, events as usize, shots as usize, result as usize);
    let n = n as usize;
    guard(move || {
        let so = std::slice::from_raw_parts_mut(so as *mut u8, 9 * assign::SOLDIER);
        let fa = std::slice::from_raw_parts(fa as *const u8, 8 * 1024);
        let sc = std::slice::from_raw_parts(sc as *const u8, 640);
        let ac = std::slice::from_raw_parts(ac as *const u8, 0xA8);
        let pa = std::slice::from_raw_parts_mut(pa as *mut u8, 64000);
        let scr = std::slice::from_raw_parts_mut(scr as *mut u8, 64000);
        let ev = std::slice::from_raw_parts(ev as *const i32, 3 * n);
        let sh = std::slice::from_raw_parts_mut(sh as *mut u8, 64000 * n);
        let re = std::slice::from_raw_parts_mut(re as *mut i32, 5);
        let (c, d, inv) = match (common(iso), panel::panel_data(iso, exe), invdraw::inv_data(iso, exe)) {
            (Ok(c), Ok(d), Ok(i)) => (c, d, i),
            (Err(e), _, _) | (_, Err(e), _) | (_, _, Err(e)) => return e,
        };
        let (mut t0, mut t1) = (c.remap[0], c.remap[0]);
        let mut pp = panel::Painter { d: &d, buf: pa, table0: &mut t0, table29: panel::table29(&c) };
        let mut sp = panel::Painter { d: &d, buf: scr, table0: &mut t1, table29: panel::table29(&c) };
        re.copy_from_slice(&[1, -1, -1, 0, -1]);
        let k = match assign::duty_box_at(ev[0], ev[1]) {
            Some(k) if ev[2] & 1 != 0 && assign::in_sector(so, k) => k,
            _ => {
                sh[..64000].copy_from_slice(&sp.buf[..64000]);
                return 0;
            }
        };
        let (mut menu, wy) = assign::DutyMenu::open(&mut pp, &mut sp, &inv, so, ac, k);
        re[4] = wy;
        sh[..64000].copy_from_slice(&sp.buf[..64000]);
        for j in 1..n {
            re[0] = j as i32 + 1;
            let mut click = false;
            if let Some(ch) = menu.poll(&mut sp, ev[3 * j], ev[3 * j + 1], ev[3 * j + 2], &mut click) {
                if let Some((dt, tw)) = ch {
                    re[1] = dt as i32;
                    re[2] = tw.map_or(-1, |t| t as i32);
                }
                re[3] = assign::choose(so, k, ch) as i32;
                assign::draw(&mut pp, &inv, &c.remap[30], so, fa, sc, ac);
                sp.buf[..64000].copy_from_slice(&pp.buf[..64000]);
                sh[64000 * j..64000 * (j + 1)].copy_from_slice(&sp.buf[..64000]);
                return 0;
            }
            sh[64000 * j..64000 * (j + 1)].copy_from_slice(&sp.buf[..64000]);
        }
        0
    })
}

/// Merc selection on the assignment screen (strategic.md "Selecting a merc"): a left press at (x, y) on the
/// assignment picture in `panel` / `screen` (both 320 x 200, in/out). `soldiers` = team slots 0..8 (9 records, the
/// item places may change them), `faces` = 8 face buffers, `accounts` = 0xA8 bytes, `view` = the store view,
/// `counts` = stash_counts (136), `records` = `nrec` (≤ 4) store records of 3 bytes from the current record on.
/// `result` (9 x i32) = [selected slot or 99, mouse x, mouse y (-1 = no move), loader result, top type (0 at the
/// screen's opening), shown types 0..3].
#[no_mangle]
pub unsafe extern "C" fn ja1f_assign_select(iso: *const c_char, exe: *const c_char, soldiers: *mut u8,
                                            faces: *const u8, accounts: *const u8, view: i32, counts: *const u8,
                                            records: *const u8, nrec: i32, panel: *mut u8, screen: *mut u8,
                                            x: i32, y: i32, result: *mut i32) -> i32 {
    let (Some(iso), Some(exe)) = (cstr(iso), cstr(exe)) else { return E_ARG };
    if soldiers.is_null() || faces.is_null() || accounts.is_null() || counts.is_null() || panel.is_null()
        || screen.is_null() || result.is_null() || !(0..=4).contains(&nrec) || (nrec > 0 && records.is_null()) {
        return E_ARG;
    }
    let (so, fa, ac, co, rc, pa, scr, re) = (soldiers as usize, faces as usize, accounts as usize, counts as usize,
        records as usize, panel as usize, screen as usize, result as usize);
    guard(move || {
        let so = std::slice::from_raw_parts_mut(so as *mut u8, 9 * assign::SOLDIER);
        let fa = std::slice::from_raw_parts(fa as *const u8, 8 * 1024);
        let ac = std::slice::from_raw_parts(ac as *const u8, 0xA8);
        let co = std::slice::from_raw_parts(co as *const u8, 136);
        let recs: Vec<[u8; 3]> = (0..nrec as usize)
            .map(|j| { let r = std::slice::from_raw_parts((rc + 3 * j) as *const u8, 3); [r[0], r[1], r[2]] })
            .collect();
        let pa = std::slice::from_raw_parts_mut(pa as *mut u8, 64000);
        let scr = std::slice::from_raw_parts_mut(scr as *mut u8, 64000);
        let re = std::slice::from_raw_parts_mut(re as *mut i32, 9);
        let (c, d, inv) = match (common(iso), panel::panel_data(iso, exe), invdraw::inv_data(iso, exe)) {
            (Ok(c), Ok(d), Ok(i)) => (c, d, i),
            (Err(e), _, _) | (_, Err(e), _) | (_, _, Err(e)) => return e,
        };
        let (mut t0, mut t1) = (c.remap[0], c.remap[0]);
        let mut pp = panel::Painter { d: &d, buf: pa, table0: &mut t0, table29: panel::table29(&c) };
        let mut sp = panel::Painter { d: &d, buf: scr, table0: &mut t1, table29: panel::table29(&c) };
        re.copy_from_slice(&[99, -1, -1, 0, 0, 0, 0, 0, 0]);
        let k = match assign::face_box_at(x, y) {
            Some(k) if assign::duty_box_at(x, y).is_none() && assign::in_sector(so, k) => k,
            _ => return 0,
        };
        let mut st = assign::Store { view: view as u8, counts: co, records: &recs, cur: 0, top: 0, shown: [0; 4],
                                     v: 0x38 };
        re[3] = assign::select(&mut pp, &mut sp, &inv, so, k, fa, ac, &mut st);
        re[0] = k as i32;
        (re[1], re[2]) = assign::SELECT_MOUSE;
        re[4] = st.top as i32;
        for j in 0..4 {
            re[5 + j] = st.shown[j] as i32;
        }
        0
    })
}

/// The panel buttons of the assignment screen (strategic.md "The panel buttons"): a left press at (x, y) selects a
/// merc as ja1f_assign_select, then each of the `npress` presses `presses` (x, y as i32 pairs; each = left down,
/// then up at the same place) acts. Stops after a deselect. `stat_changed` (640) is for the picture of the
/// deselect. `state` (12 x i32, in/out) = [view, current record (nrec or more = none), top type, shown types 0..3,
/// V, last listed slot, selected slot (out, 99 = none), loader result (out), mouse moved (out: x * 1000 + y or -1)];
/// `records` = `nrec` (≤ 16) store records of 3 bytes from record 0 on.
#[no_mangle]
pub unsafe extern "C" fn ja1f_assign_panel(iso: *const c_char, exe: *const c_char, soldiers: *mut u8,
                                           faces: *const u8, stat_changed: *const u8, accounts: *const u8,
                                           counts: *const u8, records: *const u8, nrec: i32, panel: *mut u8,
                                           screen: *mut u8, x: i32, y: i32, presses: *const i32, npress: i32,
                                           state: *mut i32) -> i32 {
    let (Some(iso), Some(exe)) = (cstr(iso), cstr(exe)) else { return E_ARG };
    if soldiers.is_null() || faces.is_null() || stat_changed.is_null() || accounts.is_null() || counts.is_null()
        || panel.is_null() || screen.is_null() || state.is_null() || !(0..=16).contains(&nrec)
        || (nrec > 0 && records.is_null()) || npress < 0 || (npress > 0 && presses.is_null()) {
        return E_ARG;
    }
    let (so, fa, sc, ac, co, rc, pa, scr, pr, stp) = (soldiers as usize, faces as usize, stat_changed as usize,
        accounts as usize, counts as usize, records as usize, panel as usize, screen as usize, presses as usize,
        state as usize);
    guard(move || {
        let so = std::slice::from_raw_parts_mut(so as *mut u8, 9 * assign::SOLDIER);
        let fa = std::slice::from_raw_parts(fa as *const u8, 8 * 1024);
        let sc = std::slice::from_raw_parts(sc as *const u8, 640);
        let ac = std::slice::from_raw_parts(ac as *const u8, 0xA8);
        let co = std::slice::from_raw_parts(co as *const u8, 136);
        let recs: Vec<[u8; 3]> = (0..nrec as usize)
            .map(|j| { let r = std::slice::from_raw_parts((rc + 3 * j) as *const u8, 3); [r[0], r[1], r[2]] })
            .collect();
        let pa = std::slice::from_raw_parts_mut(pa as *mut u8, 64000);
        let scr = std::slice::from_raw_parts_mut(scr as *mut u8, 64000);
        let pr = std::slice::from_raw_parts(pr as *const i32, 2 * npress as usize);
        let s = std::slice::from_raw_parts_mut(stp as *mut i32, 12);
        let (c, d, inv) = match (common(iso), panel::panel_data(iso, exe), invdraw::inv_data(iso, exe)) {
            (Ok(c), Ok(d), Ok(i)) => (c, d, i),
            (Err(e), _, _) | (_, Err(e), _) | (_, _, Err(e)) => return e,
        };
        let (mut t0, mut t1) = (c.remap[0], c.remap[0]);
        let mut pp = panel::Painter { d: &d, buf: pa, table0: &mut t0, table29: panel::table29(&c) };
        let mut sp = panel::Painter { d: &d, buf: scr, table0: &mut t1, table29: panel::table29(&c) };
        let mut st = assign::Store { view: s[0] as u8, counts: co, records: &recs, cur: s[1].max(0) as usize,
                                     top: s[2] as u8, shown: [s[3] as u8, s[4] as u8, s[5] as u8, s[6] as u8],
                                     v: s[7] as u8 };
        let last = s[8].max(0) as usize;
        s[9] = 99;
        s[11] = -1;
        let mut k = match assign::face_box_at(x, y) {
            Some(k) if assign::duty_box_at(x, y).is_none() && assign::in_sector(so, k) => k,
            _ => return 0,
        };
        st.top = 0;
        s[10] = assign::select(&mut pp, &mut sp, &inv, so, k, fa, ac, &mut st);
        s[11] = assign::SELECT_MOUSE.0 * 1000 + assign::SELECT_MOUSE.1;
        let mut selected = true;
        for j in 0..npress as usize {
            let (px, py) = (pr[2 * j], pr[2 * j + 1]);
            if assign::panel_face_at(px, py) {
                assign::draw(&mut pp, &inv, &c.remap[30], so, fa, sc, ac);
                sp.buf[..64000].copy_from_slice(&pp.buf[..64000]);
                let (mx, my) = assign::deselect_mouse(k);
                s[11] = mx * 1000 + my;
                s[10] = 0;
                selected = false;
                break;
            }
            if assign::panel_body_at(px, py) {
                continue;
            }
            let Some(i) = assign::panel_button_at(px, py) else { continue };
            assign::panel_button(&mut sp, i, true, st.v);
            if i == 4 {
                assign::toggle_view(&mut st);
            }
            assign::panel_button(&mut sp, i, false, st.v);
            match i {
                0 | 1 => {
                    k = assign::other_merc(k, last, i == 1);
                    s[10] = assign::show_merc(&mut pp, &mut sp, &inv, so, k, fa, ac, &mut st);
                }
                2 | 3 => {
                    if st.scroll(i == 3) {
                        assign::store_again(&mut pp, &mut sp, &inv, &mut st);
                    }
                }
                _ => assign::store_view(&mut pp, &mut sp, &inv, &mut st),
            }
        }
        s[0] = st.view as i32;
        s[1] = st.cur.min(recs.len()) as i32;
        s[2] = st.top as i32;
        for q in 0..4 {
            s[3 + q] = st.shown[q] as i32;
        }
        s[7] = st.v as i32;
        s[9] = if selected { k as i32 } else { 99 };
        0
    })
}

/// Hover on the assignment screen (strategic.md "Hover on the assignment screen", panel.md "Tooltip box"): the
/// screen is open with no merc selected, tooltip T = 0, kept position (0, 0), lit row 99; then the `nev` polls
/// `events` (x, y, button as i32 triples). A poll with no button runs the hover; a left press on a face box selects
/// that merc as ja1f_assign_panel (no merc selected) or on the item panel's face deselects (merc selected); a press
/// uses its poll and the next one. Other presses are not allowed (E_ARG). `state` (14 x i32, in/out) = [view,
/// current record, top type, shown types 0..3, V, last listed slot, selected slot (out, 99 = none), T (out), kx
/// (out), ky (out), lit row L (out)]; the other arguments as ja1f_assign_panel.
#[no_mangle]
pub unsafe extern "C" fn ja1f_assign_hover(iso: *const c_char, exe: *const c_char, soldiers: *mut u8,
                                           faces: *const u8, stat_changed: *const u8, accounts: *const u8,
                                           counts: *const u8, records: *const u8, nrec: i32, panel: *mut u8,
                                           screen: *mut u8, events: *const i32, nev: i32, state: *mut i32) -> i32 {
    let (Some(iso), Some(exe)) = (cstr(iso), cstr(exe)) else { return E_ARG };
    if soldiers.is_null() || faces.is_null() || stat_changed.is_null() || accounts.is_null() || counts.is_null()
        || panel.is_null() || screen.is_null() || state.is_null() || !(0..=16).contains(&nrec)
        || (nrec > 0 && records.is_null()) || nev < 0 || (nev > 0 && events.is_null()) {
        return E_ARG;
    }
    let (so, fa, sc, ac, co, rc, pa, scr, ev, stp) = (soldiers as usize, faces as usize, stat_changed as usize,
        accounts as usize, counts as usize, records as usize, panel as usize, screen as usize, events as usize,
        state as usize);
    guard(move || {
        let so = std::slice::from_raw_parts_mut(so as *mut u8, 9 * assign::SOLDIER);
        let fa = std::slice::from_raw_parts(fa as *const u8, 8 * 1024);
        let sc = std::slice::from_raw_parts(sc as *const u8, 640);
        let ac = std::slice::from_raw_parts(ac as *const u8, 0xA8);
        let co = std::slice::from_raw_parts(co as *const u8, 136);
        let recs: Vec<[u8; 3]> = (0..nrec as usize)
            .map(|j| { let r = std::slice::from_raw_parts((rc + 3 * j) as *const u8, 3); [r[0], r[1], r[2]] })
            .collect();
        let pa = std::slice::from_raw_parts_mut(pa as *mut u8, 64000);
        let scr = std::slice::from_raw_parts_mut(scr as *mut u8, 64000);
        let ev = std::slice::from_raw_parts(ev as *const i32, 3 * nev as usize);
        let s = std::slice::from_raw_parts_mut(stp as *mut i32, 14);
        let (c, d, inv) = match (common(iso), panel::panel_data(iso, exe), invdraw::inv_data(iso, exe)) {
            (Ok(c), Ok(d), Ok(i)) => (c, d, i),
            (Err(e), _, _) | (_, Err(e), _) | (_, _, Err(e)) => return e,
        };
        let (mut t0, mut t1) = (c.remap[0], c.remap[0]);
        let mut pp = panel::Painter { d: &d, buf: pa, table0: &mut t0, table29: panel::table29(&c) };
        let mut sp = panel::Painter { d: &d, buf: scr, table0: &mut t1, table29: panel::table29(&c) };
        let mut st = assign::Store { view: s[0] as u8, counts: co, records: &recs, cur: s[1].max(0) as usize,
                                     top: s[2] as u8, shown: [s[3] as u8, s[4] as u8, s[5] as u8, s[6] as u8],
                                     v: s[7] as u8 };
        let (mut tip, mut lit, mut sel) = (assign::Tooltip::default(), 99u8, None::<usize>);
        let mut j = 0;
        while j < nev as usize {
            let (x, y, b) = (ev[3 * j], ev[3 * j + 1], ev[3 * j + 2]);
            j += 1;
            if b == 0 {
                assign::hover(&mut sp, so, sel, (x, y), &mut tip, &mut lit);
                continue;
            }
            match sel {
                None => {
                    let k = match assign::face_box_at(x, y) {
                        Some(k) if assign::duty_box_at(x, y).is_none() && assign::in_sector(so, k) => k,
                        _ => return E_ARG,
                    };
                    st.top = 0;
                    assign::select(&mut pp, &mut sp, &inv, so, k, fa, ac, &mut st);
                    sel = Some(k);
                }
                Some(_) if assign::panel_face_at(x, y) => {
                    assign::draw(&mut pp, &inv, &c.remap[30], so, fa, sc, ac);
                    sp.buf[..64000].copy_from_slice(&pp.buf[..64000]);
                    sel = None;
                }
                _ => return E_ARG,
            }
            tip.n = 0;
            j += 1;
        }
        s[9] = sel.map_or(99, |k| k as i32);
        s[10] = tip.n as i32;
        (s[11], s[12]) = tip.at();
        s[13] = lit as i32;
        0
    })
}

/// The tooltip box (panel.md "Tooltip box") on `screen`, from T = 0 and the kept position (0, 0): `nop` operations
/// `ops` (6 x i32 each: number n, x0, y0, x1, y1, font; n = 0 = hide) with `texts` (48 bytes per operation: t1, t2,
/// t3, each NUL-padded to 16). `state` (3 x i32, out) = [T, kx, ky].
#[no_mangle]
pub unsafe extern "C" fn ja1f_tooltip(iso: *const c_char, exe: *const c_char, screen: *mut u8, ops: *const i32,
                                      texts: *const u8, nop: i32, state: *mut i32) -> i32 {
    let (Some(iso), Some(exe)) = (cstr(iso), cstr(exe)) else { return E_ARG };
    if screen.is_null() || state.is_null() || nop < 0 || (nop > 0 && (ops.is_null() || texts.is_null())) {
        return E_ARG;
    }
    let (scr, op, tx, stp) = (screen as usize, ops as usize, texts as usize, state as usize);
    guard(move || {
        let scr = std::slice::from_raw_parts_mut(scr as *mut u8, 64000);
        let op = std::slice::from_raw_parts(op as *const i32, 6 * nop as usize);
        let tx = std::slice::from_raw_parts(tx as *const u8, 48 * nop as usize);
        let s = std::slice::from_raw_parts_mut(stp as *mut i32, 3);
        let (c, d) = match (common(iso), panel::panel_data(iso, exe)) {
            (Ok(c), Ok(d)) => (c, d),
            (Err(e), _) | (_, Err(e)) => return e,
        };
        let mut t0 = c.remap[0];
        let mut sp = panel::Painter { d: &d, buf: scr, table0: &mut t0, table29: panel::table29(&c) };
        let mut tip = assign::Tooltip::default();
        for q in 0..nop as usize {
            let o = &op[6 * q..6 * q + 6];
            if o[0] == 0 {
                tip.hide(&mut sp);
                continue;
            }
            let t = |j: usize| {
                let a = &tx[48 * q + 16 * j..48 * q + 16 * j + 16];
                &a[..a.iter().position(|&b| b == 0).unwrap_or(16)]
            };
            tip.show(&mut sp, o[0] as u8, (o[1], o[2], o[3], o[4]), [t(0), t(1), t(2)], o[5].clamp(0, 2) as usize);
        }
        s[0] = tip.n as i32;
        (s[1], s[2]) = tip.at();
        0
    })
}

/// Item moves on the item panel (strategic.md "Item moves on the item panel"): a left press at (x, y) selects a
/// merc as ja1f_assign_panel, then each of the `npress` presses (x, y as i32 pairs) is one ja1_assign_item_press
/// (x < 0: the key E instead, ja1_assign_key_e; x ≥ 1000: a right press at (x − 1000, y), ja1_assign_right_press,
/// its polls get two right-button-up events) (left down at the place; its polls get the left button up at the same place). `counts` (136, in/out),
/// `records` (256 x 3 bytes, in/out) with `nrec` (in/out) = the item store. `state` (16 x i32, in/out) = [view,
/// current record (nrec or more = none), top type, shown types 0..3, V, vest owner, shift (in), ctrl (in), the
/// question answer (in), loader result (out), the result of the last press (out)]. `cursor` / `swap` = 12 bytes
/// each (in/out). `log` (512 i32) gets (kind, value) pairs as assign_items::AssignHost writes them, `log_n` the
/// pair count.
///
/// # Safety
/// Pointers as described, sizes as listed.
#[no_mangle]
#[allow(clippy::too_many_arguments)]
pub unsafe extern "C" fn ja1f_assign_items(iso: *const c_char, exe: *const c_char, soldiers: *mut u8,
                                           faces: *const u8, accounts: *const u8, counts: *mut u8,
                                           records: *mut u8, nrec: *mut i32, panel: *mut u8, screen: *mut u8,
                                           x: i32, y: i32, presses: *const i32, npress: i32, state: *mut i32,
                                           cursor: *mut u8, swap: *mut u8, log: *mut i32, log_n: *mut i32) -> i32 {
    let (Some(iso), Some(exe)) = (cstr(iso), cstr(exe)) else { return E_ARG };
    if soldiers.is_null() || faces.is_null() || accounts.is_null() || counts.is_null() || records.is_null()
        || nrec.is_null() || !(0..=256).contains(&*nrec) || panel.is_null() || screen.is_null() || state.is_null()
        || npress < 0 || (npress > 0 && presses.is_null()) || cursor.is_null() || swap.is_null() || log.is_null()
        || log_n.is_null() {
        return E_ARG;
    }
    let (so, fa, ac, co, rc, nr, pa, scr, pr, stp, cu, sw, lg, ln) = (soldiers as usize, faces as usize,
        accounts as usize, counts as usize, records as usize, nrec as usize, panel as usize, screen as usize,
        presses as usize, state as usize, cursor as usize, swap as usize, log as usize, log_n as usize);
    guard(move || {
        let so = std::slice::from_raw_parts_mut(so as *mut u8, 9 * assign::SOLDIER);
        let fa = std::slice::from_raw_parts(fa as *const u8, 8 * 1024);
        let ac = std::slice::from_raw_parts(ac as *const u8, 0xA8);
        let co = std::slice::from_raw_parts_mut(co as *mut u8, 136);
        let nr = &mut *(nr as *mut i32);
        let recs = std::slice::from_raw_parts_mut(rc as *mut u8, 3 * 256);
        let pa = std::slice::from_raw_parts_mut(pa as *mut u8, 64000);
        let scr = std::slice::from_raw_parts_mut(scr as *mut u8, 64000);
        let pr = std::slice::from_raw_parts(pr as *const i32, 2 * npress as usize);
        let s = std::slice::from_raw_parts_mut(stp as *mut i32, 16);
        let cu = std::slice::from_raw_parts_mut(cu as *mut u8, 12);
        let sw = std::slice::from_raw_parts_mut(sw as *mut u8, 12);
        let lg = std::slice::from_raw_parts_mut(lg as *mut i32, 512);
        let ln = &mut *(ln as *mut i32);
        let (c, d, inv) = match (common(iso), panel::panel_data(iso, exe), invdraw::inv_data(iso, exe)) {
            (Ok(c), Ok(d), Ok(i)) => (c, d, i),
            (Err(e), _, _) | (_, Err(e), _) | (_, _, Err(e)) => return e,
        };
        let Ok(exe_bytes) = std::fs::read(exe) else { return E_FILE };
        let tab = |off: usize, n: usize| exe_bytes.get(off..off + n).map(|b| b.to_vec());
        let (Some(item_types), Some(weapons), Some(attach)) = (tab(900764, 1632), tab(900309, 1536), tab(899918, 328))
        else { return E_FILE };
        let mut store = assign_items::ItemStore {
            view: s[0] as u8,
            records: (0..*nr as usize).map(|j| [recs[3 * j], recs[3 * j + 1], recs[3 * j + 2]]).collect(),
            cur: 0, top: s[2] as u8, shown: [s[3] as u8, s[4] as u8, s[5] as u8, s[6] as u8], v: s[7] as u8,
        };
        store.cur = if (s[1].max(0) as usize) < store.records.len() { s[1] as usize } else { assign_items::NONE };
        let (mut t0, mut t1) = (c.remap[0], c.remap[0]);
        let mut pp = panel::Painter { d: &d, buf: pa, table0: &mut t0, table29: panel::table29(&c) };
        let mut sp = panel::Painter { d: &d, buf: scr, table0: &mut t1, table29: panel::table29(&c) };
        let k = match assign::face_box_at(x, y) {
            Some(k) if assign::duty_box_at(x, y).is_none() && assign::in_sector(so, k) => k,
            _ => return 0,
        };
        store.top = 0;
        cu[..6].fill(0);                          // the screen opens with cursor and swap bytes 0..5 = 0
        sw[..6].fill(0);
        let loader = store.with(co, |st| assign::select(&mut pp, &mut sp, &inv, so, k, fa, ac, st));
        let mut armour = invdraw::armour(&so[k * assign::SOLDIER..(k + 1) * assign::SOLDIER]);
        let (mut picked, mut give) = ([0u8; 12], [0u8; 12]);
        let rects = assign_items::item_rects();
        let mut w: host::Ja1World = std::mem::zeroed();
        w.soldiers = so.as_mut_ptr();
        w.item_types = item_types.as_ptr();
        w.weapons = weapons.as_ptr();
        w.attach = attach.as_ptr();
        w.inv_cursor = cu.as_mut_ptr();
        w.inv_swap = sw.as_mut_ptr();
        w.inv_picked = picked.as_mut_ptr();
        w.inv_give = give.as_mut_ptr();
        w.item_rects = rects.as_ptr();
        w.vest_owner = s[8] as u8;
        w.key_shift = s[9] as u8;
        w.key_ctrl = s[10] as u8;
        w.stash_counts = co.as_mut_ptr();
        let mut h = assign_items::AssignHost {
            pp, sp, inv: &inv, soldiers: so.as_mut_ptr(), faces: fa, store, world: &mut w,
            events: Default::default(), log: vec![(1, 0)], loader, quote: None, answer: s[11], show_info: false, info_under: None, info_mouse: false, info_shot: None,
            opt_info: crate::iteminfo::OPT_INFO, opt_sub: crate::iteminfo::OPT_SUB, extra: None,
        };
        let jh = assign_items::ja1_host(&mut h);
        let mut last = 0;
        for j in 0..npress as usize {
            let (px, py) = (pr[2 * j], pr[2 * j + 1]);
            if px < 0 {
                // key E (strategic.md "Key E: all items into the store"); the mouse stays where it was
                h.events.clear();
                assign_items::ja1_assign_key_e(h.world, &jh, k as i32);
                continue;
            }
            if px >= 1000 {
                (*h.world).mouse_x = px - 1000;
                (*h.world).mouse_y = py;
                (*h.world).mouse_buttons = 2;
                h.events.clear();
                h.events.extend([(px - 1000, py, 0), (px - 1000, py, 0)]);
                last = assign_items::ja1_assign_right_press(h.world, &jh, k as i32);
                if last != 0 {
                    break;
                }
                continue;
            }
            (*h.world).mouse_x = px;
            (*h.world).mouse_y = py;
            (*h.world).mouse_buttons = 1;
            h.events.clear();
            h.events.push_back((px, py, 0));
            last = assign_items::ja1_assign_item_press(h.world, &jh, k as i32, &mut armour);
            if last != 0 {
                break;
            }
        }
        let st = &h.store;
        *nr = st.records.len().min(256) as i32;
        for (j, r) in st.records.iter().take(256).enumerate() {
            recs[3 * j..3 * j + 3].copy_from_slice(r);
        }
        s[0] = st.view as i32;
        s[1] = st.cur.min(st.records.len()) as i32;
        s[2] = st.top as i32;
        for q in 0..4 {
            s[3 + q] = st.shown[q] as i32;
        }
        s[7] = st.v as i32;
        s[8] = (*h.world).vest_owner as i32;
        s[12] = h.loader;
        s[13] = last;
        *ln = h.log.len().min(256) as i32;
        for (j, &(a, b)) in h.log.iter().take(256).enumerate() {
            lg[2 * j] = a;
            lg[2 * j + 1] = b;
        }
        0
    })
}

/// The office's team ledger page (strategic.md "The team ledger", steps 1 to 4; the count between steps 3 and 4 is
/// the caller's): `soldiers` = team slots 0..7 (8 x 0x200 bytes), `faces` = 8 face buffers (32 x 32), `acc` = the
/// 0xA8 `accounts` bytes, `room` = the room buffer (in and out: the health bars are drawn there), `screen` = the
/// 320 x 200 screen (in and out), `pal` = the page's palette (768 bytes, 6-bit). Returns 0.
#[no_mangle]
#[allow(clippy::too_many_arguments)]
pub unsafe extern "C" fn ja1f_ledger(iso: *const c_char, exe: *const c_char, soldiers: *const u8, faces: *const u8,
                                     acc: *const u8, difficulty: u8, room: *mut u8, screen: *mut u8, pal: *mut u8)
    -> i32 {
    let (Some(iso), Some(exe)) = (cstr(iso), cstr(exe)) else { return E_ARG };
    if soldiers.is_null() || faces.is_null() || acc.is_null() || room.is_null() || screen.is_null() || pal.is_null() {
        return E_ARG;
    }
    let (so, fa, ac, ro, sc, pa) = (soldiers as usize, faces as usize, acc as usize, room as usize, screen as usize,
                                    pal as usize);
    guard(move || {
        let so = std::slice::from_raw_parts(so as *const u8, 8 * 0x200);
        let fa = std::slice::from_raw_parts(fa as *const u8, 8 * 1024);
        let ac = std::slice::from_raw_parts(ac as *const u8, 0xA8);
        let ro = std::slice::from_raw_parts_mut(ro as *mut u8, 64000);
        let sc = std::slice::from_raw_parts_mut(sc as *mut u8, 64000);
        let pa = std::slice::from_raw_parts_mut(pa as *mut u8, 768);
        let (c, d, od) = match (common(iso), panel::panel_data(iso, exe), office::OfficeData::load(exe)) {
            (Ok(c), Ok(d), Ok(o)) => (c, d, o),
            (Err(e), _, _) | (_, Err(e), _) | (_, _, Err(e)) => return e,
        };
        let mut table0 = c.remap[0];
        let mut p = panel::Painter { d: &d, buf: sc, table0: &mut table0, table29: panel::table29(&c) };
        let day = i32::from_le_bytes([ac[4], ac[5], ac[6], ac[7]]);
        pa.copy_from_slice(&office::ledger_team(&od, &mut p, ro, so, fa, day));
        office::ledger_accounts(&mut p, ac, difficulty);
        0
    })
}

/// The switch box (strategic.md "The switch box (Control Panel)"): opens over the office picture `scr` (320 x 200)
/// with the 20 bytes `opts` (changed in place), then the left presses `xy` (n pairs) until the close lever (then the
/// close with the plain room `plain`). Each step goes to `log` (4 i32 each): (0, picture index, ms, 0) with the
/// picture at `pics` + 64000 index, (1, effect, rate, volume), (2, on, act, 0), (3, k, value, 0) per toggle;
/// `counts` = [log entries, pictures]. Returns 0, or E_ARG when a buffer is too small.
#[no_mangle]
#[allow(clippy::too_many_arguments)]
pub unsafe extern "C" fn ja1f_switchbox(iso: *const c_char, exe: *const c_char, scr: *const u8, plain: *const u8,
                                        opts: *mut u8, sound: i32, music: i32, wanted: i32, playing: i32,
                                        xy: *const i32, n: i32, log: *mut i32, log_cap: i32, pics: *mut u8,
                                        pic_cap: i32, counts: *mut i32) -> i32 {
    let (Some(iso), Some(exe)) = (cstr(iso), cstr(exe)) else { return E_ARG };
    if scr.is_null() || plain.is_null() || opts.is_null() || xy.is_null() || log.is_null() || pics.is_null()
        || counts.is_null() || n < 0 {
        return E_ARG;
    }
    let (sc, pl, op, xy, lg, pc, ct) = (scr as usize, plain as usize, opts as usize, xy as usize, log as usize,
                                        pics as usize, counts as usize);
    guard(move || {
        let sc = std::slice::from_raw_parts(sc as *const u8, 64000);
        let pl = std::slice::from_raw_parts(pl as *const u8, 64000);
        let op = &mut *(op as *mut [u8; 20]);
        let xy = std::slice::from_raw_parts(xy as *const i32, 2 * n as usize);
        let lg = std::slice::from_raw_parts_mut(lg as *mut i32, 4 * log_cap.max(0) as usize);
        let pc = std::slice::from_raw_parts_mut(pc as *mut u8, 64000 * pic_cap.max(0) as usize);
        let ct = std::slice::from_raw_parts_mut(ct as *mut i32, 2);
        let (c, d, od) = match (common(iso), panel::panel_data(iso, exe), office::OfficeData::load(exe)) {
            (Ok(c), Ok(d), Ok(o)) => (c, d, o),
            (Err(e), _, _) | (_, Err(e), _) | (_, _, Err(e)) => return e,
        };
        let mut table0 = c.remap[0];
        let mut buf = sc.to_vec();
        let mut steps = vec![];
        let mut p = panel::Painter { d: &d, buf: &mut buf, table0: &mut table0, table29: panel::table29(&c) };
        let mut b = switchbox::SwitchBox::open(&od, &mut p, op, &mut steps);
        let mut out: Vec<(Vec<switchbox::Step>, Vec<(usize, u8)>)> = vec![(std::mem::take(&mut steps), vec![])];
        for q in xy.chunks(2) {
            let (t, closed) = b.press(&od, &mut p, q[0], q[1], op, sound != 0, music != 0, wanted, playing, &mut steps);
            if closed {
                b.close(&od, pl, op[0] != 0, &mut steps);
                out.push((std::mem::take(&mut steps), t));
                break;
            }
            out.push((std::mem::take(&mut steps), t));
        }
        let (mut nl, mut np) = (0usize, 0usize);
        let mut put = |e: [i32; 4], lg: &mut [i32]| -> bool {
            if nl * 4 + 4 > lg.len() {
                return false;
            }
            lg[nl * 4..nl * 4 + 4].copy_from_slice(&e);
            nl += 1;
            true
        };
        for (st, t) in out {
            for s in st {
                let ok = match s {
                    switchbox::Step::Show(img, ms) => {
                        if np * 64000 + 64000 > pc.len() {
                            return E_ARG;
                        }
                        pc[np * 64000..(np + 1) * 64000].copy_from_slice(&img);
                        np += 1;
                        put([0, np as i32 - 1, ms as i32, 0], lg)
                    }
                    switchbox::Step::Sfx(fx, rate, vol, _) => put([1, fx, rate, vol], lg),
                    switchbox::Step::Music(on, act) => put([2, on as i32, act as i32, 0], lg),
                };
                if !ok {
                    return E_ARG;
                }
            }
            for (k, v) in t {
                if !put([3, k as i32, v as i32, 0], lg) {
                    return E_ARG;
                }
            }
        }
        ct[0] = nl as i32;
        ct[1] = np as i32;
        0
    })
}

/// The tactical Control Panel's opening picture (panel.md "The Control Panel (key F10, `opt` button)") over the
/// screen `under` (320 x 200, in and out): the 16 switches with their ON / OFF values, Continue Game and the Quick
/// Save / Quick Restore buttons. `opts` = the 16 switch bytes; `combat` = the in-tactical-combat byte and `quick` =
/// the save byte (the two that show Quick Save). `places` (17 x 2 i32) = the item positions. 0 = done.
#[no_mangle]
pub unsafe extern "C" fn ja1f_ctlpanel_open(iso: *const c_char, exe: *const c_char, save: *const c_char,
                                            under: *mut u8, opts: *const u8, combat: i32, quick: i32,
                                            restore: i32, places: *mut i32) -> i32 {
    let (Some(iso), Some(exe), Some(save)) = (cstr(iso), cstr(exe), cstr(save)) else { return E_ARG };
    if under.is_null() || opts.is_null() || places.is_null() {
        return E_ARG;
    }
    let (op, un, pl) = (opts as usize, under as usize, places as usize);
    guard(move || {
        let opt = std::slice::from_raw_parts(op as *const u8, 16);
        let un = std::slice::from_raw_parts_mut(un as *mut u8, 64000);
        let pl = std::slice::from_raw_parts_mut(pl as *mut i32, 34);
        let (c, d) = match (common(iso), panel::panel_data(iso, exe)) {
            (Ok(c), Ok(d)) => (c, d),
            (Err(e), _) | (_, Err(e)) => return e,
        };
        let base = match world(iso, exe, save) {
            Ok(w) => w,
            Err(e) => return e,
        };
        let mut w = (*base).clone();
        w.combat = combat as u8;
        for (k, &v) in opt.iter().enumerate() {
            ctlpanel::set(&mut w, k, v);
        }
        let cp = ctlpanel::Panel::open(&w, quick != 0, restore != 0, true, true);
        let mut table0 = c.remap[0];
        let mut p = panel::Painter { d: &d, buf: un, table0: &mut table0, table29: panel::table29(&c) };
        ctlpanel::paint(&mut p, &[], &cp.log);
        for k in 0..17 {
            let (x, y) = ctlpanel::place(k);
            pl[2 * k] = x;
            pl[2 * k + 1] = y;
        }
        0
    })
}

/// The tactical Control Panel's input loop (ControlPanelInput 0x4BAA4; panel.md "The Control Panel"): the same
/// world as `ja1f_ctlpanel_open` with `opts`, `combat`, `quick`, `restore`, the sound / music driver bytes and the
/// music's wanted / playing themes, then the passes `tape` (4 i32 each: mouse x, y, left press, escape). Each pass
/// appends its effects to `effects` (4 i32 each): (1, 0, 0x2B11, 20) a click sound, (2, on, track, 0) a SetMusic
/// (on = 1) / StopMusic (on = 0) call, (3, 0, 0, 0) Show Paths went off, (4, result, 0, 0) the panel closes (the
/// run stops there). `opts` (17 bytes) = the 16 switches and byte 21554. `switches` (16 bytes) = the switch bytes at
/// the end; `counts[0]` = the effects written. `draws` gets one text line per drawing call of the passes (not of the
/// opening): `T x y fg shadow text` or `C x y`; `counts[1]` = its bytes. Returns 0, or E_ARG when a buffer is too small.
#[no_mangle]
#[allow(clippy::too_many_arguments)]
pub unsafe extern "C" fn ja1f_ctlpanel_pass(iso: *const c_char, exe: *const c_char, save: *const c_char,
                                            opts: *const u8, combat: i32, quick: i32, restore: i32, sound: i32,
                                            music: i32, wanted: i32, playing: i32, tape: *const i32, n: i32,
                                            effects: *mut i32, eff_cap: i32, switches: *mut u8,
                                            counts: *mut i32, draws: *mut u8, draw_cap: i32) -> i32 {
    let (Some(iso), Some(exe), Some(save)) = (cstr(iso), cstr(exe), cstr(save)) else { return E_ARG };
    if opts.is_null() || tape.is_null() || effects.is_null() || switches.is_null() || counts.is_null() || n < 0
        || draws.is_null() || draw_cap < 0 {
        return E_ARG;
    }
    let (op, ta, ef, sw, ct) = (opts as usize, tape as usize, effects as usize, switches as usize, counts as usize);
    let dr = draws as usize;
    guard(move || {
        let opt = std::slice::from_raw_parts(op as *const u8, 17);
        let dr = std::slice::from_raw_parts_mut(dr as *mut u8, draw_cap as usize);
        let tape = std::slice::from_raw_parts(ta as *const i32, 4 * n as usize);
        let ef = std::slice::from_raw_parts_mut(ef as *mut i32, 4 * eff_cap.max(0) as usize);
        let sw = std::slice::from_raw_parts_mut(sw as *mut u8, 16);
        let ct = std::slice::from_raw_parts_mut(ct as *mut i32, 2);
        let base = match world(iso, exe, save) {
            Ok(w) => w,
            Err(e) => return e,
        };
        let mut w = (*base).clone();
        w.combat = combat as u8;
        for (k, &v) in opt.iter().enumerate() {
            ctlpanel::set(&mut w, k, v);
        }
        let mut cp = ctlpanel::Panel::open(&w, quick != 0, restore != 0, sound != 0, music != 0);
        cp.log.clear();
        let mut lines: Vec<u8> = Vec::new();
        let mut nl = 0usize;
        let mut put = |e: [i32; 4], ef: &mut [i32]| -> bool {
            if nl * 4 + 4 > ef.len() {
                return false;
            }
            ef[nl * 4..nl * 4 + 4].copy_from_slice(&e);
            nl += 1;
            true
        };
        for q in tape.chunks(4) {
            let out = cp.pass(&mut w, q[0], q[1], q[2] != 0, q[3] != 0);
            for d in cp.log.drain(..) {
                match d.call() {
                    ctlpanel::Call::Text { s, x, y, fg, shadow } => {
                        lines.extend_from_slice(format!("T {x} {y} {fg} {shadow} ").as_bytes());
                        lines.extend_from_slice(s);
                        lines.push(b'\n');
                    }
                    ctlpanel::Call::Copy { x, y } => lines.extend_from_slice(format!("C {x} {y}\n").as_bytes()),
                }
            }
            if out.clicks >= 1 && !put([1, 0, 0x2B11, 20], ef) {
                return E_ARG;
            }
            if out.hide_marks && !put([3, 0, 0, 0], ef) {
                return E_ARG;
            }
            if let Some(on) = out.music {
                // The music module acts only with a wanted theme (on) or a playing theme (off): music.rs switch.
                let act = if on { wanted > 0 } else { playing > 0 };
                if act && !put([2, on as i32, if on { wanted } else { 0 }, 0], ef) {
                    return E_ARG;
                }
            }
            if out.clicks >= 2 && !put([1, 0, 0x2B11, 20], ef) {
                return E_ARG;
            }
            if let Some(r) = out.close {
                if !put([4, r, 0, 0], ef) {
                    return E_ARG;
                }
                break;
            }
        }
        for k in 0..16 {
            sw[k] = ctlpanel::get(&w, k);
        }
        ct[0] = nl as i32;
        if lines.len() > dr.len() {
            return E_ARG;
        }
        dr[..lines.len()].copy_from_slice(&lines);
        ct[1] = lines.len() as i32;
        0
    })
}

/// The office picture (strategic.md "The office"): the room (dark ≠ 0: the dark room) with the A.I.M. link byte and
/// `day` into `buf` (320 x 200), its palette (6-bit) into `pal` (768); then, when 1 ≤ hk ≤ 7, highlight frame hj
/// (0..3) of hot spot hk; then, when `label` is not null, the label text at the mouse (mx, my). 0 = done.
#[no_mangle]
pub unsafe extern "C" fn ja1f_office(iso: *const c_char, exe: *const c_char, link: u8, day: i32, dark: i32, hk: i32,
                                     hj: i32, label: *const c_char, mx: i32, my: i32, buf: *mut u8, pal: *mut u8) -> i32 {
    let (Some(iso), Some(exe)) = (cstr(iso), cstr(exe)) else { return E_ARG };
    if buf.is_null() || pal.is_null() || !(0..4).contains(&hj) {
        return E_ARG;
    }
    let label = if label.is_null() { None } else { Some(CStr::from_ptr(label).to_bytes().to_vec()) };
    let (bu, pa) = (buf as usize, pal as usize);
    guard(move || {
        let bu = std::slice::from_raw_parts_mut(bu as *mut u8, 64000);
        let pa = std::slice::from_raw_parts_mut(pa as *mut u8, 768);
        let (c, d, od) = match (common(iso), panel::panel_data(iso, exe), office::OfficeData::load(exe)) {
            (Ok(c), Ok(d), Ok(o)) => (c, d, o),
            (Err(e), _, _) | (_, Err(e), _) | (_, _, Err(e)) => return e,
        };
        let mut table0 = c.remap[0];
        let mut p = panel::Painter { d: &d, buf: bu, table0: &mut table0, table29: panel::table29(&c) };
        pa.copy_from_slice(&office::room(&od, &mut p, link, day, dark != 0));
        if (1..=7).contains(&hk) {
            office::frame(&od, p.buf, hk as usize, hj as usize);
        }
        if let Some(l) = label {
            office::draw_label(&mut p, &String::from_utf8_lossy(&l), mx, my);
        }
        0
    })
}

/// sound::quote_plan for speechwait_diff: `out` gets (part, volume) pairs, part 0 whole, 1 A, 2 name, 3 B, 4 kept
/// (no member: the stream behind the directory). Returns the count.
#[no_mangle]
pub unsafe extern "C" fn ja1f_quote_plan(whole: i32, a: i32, b: i32, out: *mut i32) -> i32 {
    let plan = sound::quote_plan(whole != 0, a != 0, b != 0);
    for (i, (part, vol)) in plan.iter().enumerate() {
        let code = match part {
            sound::QuotePart::Whole => 0,
            sound::QuotePart::A => 1,
            sound::QuotePart::Name => 2,
            sound::QuotePart::B => 3,
            sound::QuotePart::Kept => 4,
        };
        *out.add(2 * i) = code;
        *out.add(2 * i + 1) = *vol;
    }
    plan.len() as i32
}

/// SpeechWait over a tape (speechwait_diff). `sheet`: 148 x 69. `rules` = [text_ms, voiced, hold, subtitles], `spot` =
/// [x, y, b] of the face. `tape`: n x [now, x, y, buttons, key, playing, silent]. `out` (n + 1) x [redraw, text, end,
/// 0], `blocks` (n + 1) x 56 x 50: the face block each redraw shows; entry n is the plain face at the end.
/// `counts[0]` = the passes run. A pass that ends has its entry too.
#[no_mangle]
#[allow(clippy::too_many_arguments)]
pub unsafe extern "C" fn ja1f_speech_wait(iso: *const c_char, exe: *const c_char, sheet: *const u8, rules: *const i32,
                                          spot: *const i32, text: *const c_char, seed: u32, tape: *const i32, n: i32,
                                          out: *mut i32, blocks: *mut u8, counts: *mut i32) -> i32 {
    let (Some(iso), Some(exe), Some(text)) = (cstr(iso), cstr(exe), cstr(text)) else { return E_ARG };
    if sheet.is_null() || rules.is_null() || spot.is_null() || tape.is_null() || out.is_null() || blocks.is_null()
        || counts.is_null() || n < 0 {
        return E_ARG;
    }
    let (sh, ru, sp, ta, ou, bl, ct) = (sheet as usize, rules as usize, spot as usize, tape as usize, out as usize,
                                        blocks as usize, counts as usize);
    guard(move || {
        let n = n as usize;
        let sheet = std::slice::from_raw_parts(sh as *const u8, 148 * 69).to_vec();
        let ru = std::slice::from_raw_parts(ru as *const i32, 4);
        let sp = std::slice::from_raw_parts(sp as *const i32, 3);
        let tape = std::slice::from_raw_parts(ta as *const i32, 7 * n);
        let out = std::slice::from_raw_parts_mut(ou as *mut i32, 4 * (n + 1));
        let blocks = std::slice::from_raw_parts_mut(bl as *mut u8, 2800 * (n + 1));
        let spot = faceanim::FaceSpot { sheet, x: sp[0], y: sp[1], b: sp[2] };
        let face = faceanim::WaitFace::new(spot.clone(), None, seed);
        let rules = faceanim::WaitRules { text_ms: ru[0] as i64, voiced: ru[1] != 0, hold: ru[2] as i64, subtitles: ru[3] != 0,
                                          has_text: !text.is_empty() };
        let mut w = faceanim::SpeechWait::new(face, rules);
        let block = |spot: &faceanim::FaceSpot, e, m, txt: bool, o: &mut [u8]| -> Result<(), i32> {
            let mut scr = vec![0u8; 64000];
            spot.paint(&mut scr, e, m);
            if txt {
                balloon::wait_text(iso, exe, &mut scr, spot, text.as_bytes())?;
            }
            for j in 0..50 {
                let at = (spot.y as usize + j) * 320 + spot.x as usize;
                o[j * 56..(j + 1) * 56].copy_from_slice(&scr[at..at + 56]);
            }
            Ok(())
        };
        let mut k = 0;
        while k < n {
            let t = &tape[k * 7..k * 7 + 7];
            let i = faceanim::WaitInput { now: t[0] as i64, x: t[1], y: t[2], left: t[3] & 1 != 0, right: t[3] & 2 != 0,
                                          key: faceanim::wait_key_ends(t[4] as u8), playing: t[5] != 0, played_us: 0,
                                          silent: Some(t[6] != 0) };
            let st = w.pass(&i);
            out[k * 4] = st.redraw as i32;
            out[k * 4 + 1] = st.text as i32;
            out[k * 4 + 2] = st.end.map_or(0, |e| e as i32);
            if st.redraw {
                let (e, m) = (w.face.anim.e, w.face.anim.m);
                if let Err(c) = block(&spot, e, m, st.text, &mut blocks[k * 2800..(k + 1) * 2800]) {
                    return c;
                }
            }
            k += 1;
            if st.end.is_some() {
                break;
            }
        }
        // "When the wait ends the plain face (frames 0) is drawn once more."
        if let Err(c) = block(&spot, 0, 0, false, &mut blocks[n * 2800..(n + 1) * 2800]) {
            return c;
        }
        std::slice::from_raw_parts_mut(ct as *mut i32, 1)[0] = k as i32;
        0
    })
}

/// Sunrise::run with its log on (sunrise_screen_diff): `frames` holds cap x (64000 + 768) bytes (screen + 6-bit
/// palette), `sounds` holds sound_cap x 5 i32; counts = [frames, sounds, final seed].
#[no_mangle]
#[allow(clippy::too_many_arguments)]
pub unsafe extern "C" fn ja1f_sunrise_screen(iso: *const c_char, exe: *const c_char, day: i32, balance: i32, arrivals: i16,
                                             poll_at: i32, seed: u32, frames: *mut u8, cap: i32, sounds: *mut i32,
                                             sound_cap: i32, counts: *mut i32) -> i32 {
    let (Some(iso), Some(exe)) = (cstr(iso), cstr(exe)) else { return E_ARG };
    if frames.is_null() || sounds.is_null() || counts.is_null() || cap < 0 || sound_cap < 0 {
        return E_ARG;
    }
    let (fp, sp, ct) = (frames as usize, sounds as usize, counts as usize);
    guard(move || {
        let t0 = match common(iso) {
            Ok(c) => c.remap[0],
            Err(e) => return e,
        };
        let mut s = sunrise::Sunrise::new(iso, exe, t0);
        s.poll_at = u32::try_from(poll_at).ok();
        s.log = Some(sunrise::RunLog::default());
        let mut rng = host::Rng::new(seed);
        s.run(day, balance, arrivals, &mut rng);
        let log = s.log.take().unwrap_or_default();
        if log.frames.len() > cap as usize || log.sounds.len() > sound_cap as usize {
            return E_ARG;
        }
        let out = std::slice::from_raw_parts_mut(fp as *mut u8, cap as usize * (64000 + 768));
        for (k, (scr, pal)) in log.frames.iter().enumerate() {
            let o = &mut out[k * (64000 + 768)..(k + 1) * (64000 + 768)];
            o[..64000].copy_from_slice(scr);
            o[64000..].copy_from_slice(pal);
        }
        let ev = std::slice::from_raw_parts_mut(sp as *mut i32, sound_cap as usize * 5);
        for (k, e) in log.sounds.iter().enumerate() {
            ev[k * 5..k * 5 + 5].copy_from_slice(e);
        }
        let ct = std::slice::from_raw_parts_mut(ct as *mut i32, 3);
        ct[0] = log.frames.len() as i32;
        ct[1] = log.sounds.len() as i32;
        ct[2] = rng.seed as i32;
        0
    })
}

/// The office input loop (OfficeScreen 0x4EE64; strategic.md "The office"): a world from `save` with the A.I.M.
/// link byte, the day and the first `team` soldier slots in the current sector, then `Office::new` and one poll for
/// each tape entry (4 i32: mouse x, y, buttons, key — a left press is the rising edge of bit 0, `key` ≠ 0 = the key
/// V). Each poll appends its effects to `events` (5 i32 each, the last = the tape index): (1, hot spot, on, 0) an
/// OfficeHighlight call,
/// (2, label id, x, y) the label drawn at every move (empty label = 8; the original draws it only on a move),
/// (3, text id, 0, 0) a message box (1 = the Sleep text, 2 = the version text), (4, what, a, 0) an action
/// (0 slots with a = 1 Save / 0 Restore, 1 A.I.M., 2 View Team, 3 Control Panel, 4 Sleep) and (5, result, 0, 0)
/// the loop ends (4 Sleep, 7 Leave). A message box eats the tape entries until one has a press or a key, as the
/// original's StatusMessage does. `counts[0]` = the events written. Returns 0, or E_ARG when a buffer is too small.
#[no_mangle]
#[allow(clippy::too_many_arguments)]
pub unsafe extern "C" fn ja1f_office_pass(iso: *const c_char, exe: *const c_char, save: *const c_char, link: i32,
                                          day: i32, team: i32, tape: *const i32, n: i32, events: *mut i32,
                                          cap: i32, counts: *mut i32) -> i32 {
    let (Some(iso), Some(exe), Some(save)) = (cstr(iso), cstr(exe), cstr(save)) else { return E_ARG };
    if tape.is_null() || events.is_null() || counts.is_null() || n < 0 || cap < 0 {
        return E_ARG;
    }
    let (ta, ev, ct) = (tape as usize, events as usize, counts as usize);
    guard(move || {
        let tape = std::slice::from_raw_parts(ta as *const i32, 4 * n as usize);
        let ev = std::slice::from_raw_parts_mut(ev as *mut i32, 5 * cap as usize);
        let ct = std::slice::from_raw_parts_mut(ct as *mut i32, 1);
        let base = match world(iso, exe, save) {
            Ok(w) => w,
            Err(e) => return e,
        };
        let mut w = (*base).clone();
        if let Some(b) = w.accounts.get_mut(0xA3) {
            *b = link as u8;
        }
        w.day = day;
        for k in 0..8 {
            if let Some(b) = w.soldiers.get_mut(k * 0x200 + 0x182) {
                *b = u8::from((k as i32) < team);
            }
        }
        let mut o = match office::Office::new(iso, exe, &w) {
            Ok(o) => o,
            Err(e) => return e,
        };
        // The harness leaves the original's mouse at (-1, -1) before the run.
        o.set_mouse(-1, -1);
        let mut nl = 0usize;
        let mut put = |e: [i32; 5]| -> bool {
            if nl * 5 + 5 > ev.len() {
                return false;
            }
            ev[nl * 5..nl * 5 + 5].copy_from_slice(&e);
            nl += 1;
            true
        };
        let label_id = |s: &str| -> i32 {
            match s {
                "Save Game" => 0,
                "Restore Game" => 1,
                "Contact A.I.M." => 2,
                "Sleep" => 4,
                "View Team" => 5,
                "Control Panel" => 6,
                "Leave" => 7,
                "" => 8,
                _ => 9,
            }
        };
        let mut prev = 0i32;
        let mut q = 0usize;
        while q < n as usize {
            let (x, y, btn, key) = (tape[4 * q], tape[4 * q + 1], tape[4 * q + 2], tape[4 * q + 3]);
            let press = btn & 1 != 0 && prev & 1 == 0;
            prev = btn;
            let qi = q as i32;
            q += 1;
            // The office loop's key handling: only V (any case) opens the version box; any other key does nothing.
            let out = o.poll(x, y, press, key == 0x56 || key == 0x76);
            for &(k, on) in &out.highlights {
                if !put([1, k as i32, on as i32, 0, qi]) {
                    return E_ARG;
                }
            }
            if let Some((t, lx, ly)) = o.label_drawn() {
                if !put([2, label_id(t), lx, ly, qi]) {
                    return E_ARG;
                }
            }
            if out.msg {
                if let Some(m) = o.msg {
                    if !put([3, if m == office::SLEEP_MSG { 1 } else { 2 }, 0, 0, qi]) {
                        return E_ARG;
                    }
                }
            }
            if let Some(m) = out.slots {
                if !put([4, 0, m, 0, qi]) {
                    return E_ARG;
                }
            }
            if out.aim && !put([4, 1, 0, 0, qi]) {
                return E_ARG;
            }
            if out.ledger && !put([4, 2, 0, 0, qi]) {
                return E_ARG;
            }
            if out.switchbox && !put([4, 3, 0, 0, qi]) {
                return E_ARG;
            }
            if out.sleep && !put([4, 4, 0, 0, qi]) {
                return E_ARG;
            }
            if out.aim {
                // The A.I.M. screen changed the link, the team and the faces: Office::new again, the mouse where
                // the screen left it (OfficeScreen 0x4F2C1), so the next poll at the same point is no move.
                o = match office::Office::new(iso, exe, &w) {
                    Ok(o) => o,
                    Err(e) => return e,
                };
                o.set_mouse(x, y);
            }
            if out.sleep || out.leave {
                let r = if out.sleep { 4 } else { 7 };
                put([5, r, 0, 0, qi]);
                break;
            }
            if out.msg {
                // The box waits: its entries never reach the office loop (StatusMessage).
                while q < n as usize {
                    let (bx, by, bb, bk) = (tape[4 * q], tape[4 * q + 1], tape[4 * q + 2], tape[4 * q + 3]);
                    let bp = bb & 1 != 0 && prev & 1 == 0;
                    prev = bb;
                    q += 1;
                    let closes = bp || bk != 0;
                    o.poll(bx, by, bp, bk != 0);
                    if closes {
                        break;
                    }
                }
            }
        }
        ct[0] = nl as i32;
        0
    })
}

/// The game-over screen (ending.md "The game-over screen") into `buf` (320 x 200), its palette (6-bit) into `pal`
/// (768). 0 = done.
#[no_mangle]
pub unsafe extern "C" fn ja1f_game_over(iso: *const c_char, exe: *const c_char, buf: *mut u8, pal: *mut u8) -> i32 {
    let (Some(iso), Some(exe)) = (cstr(iso), cstr(exe)) else { return E_ARG };
    if buf.is_null() || pal.is_null() {
        return E_ARG;
    }
    let (bu, pa) = (buf as usize, pal as usize);
    guard(move || {
        let bu = std::slice::from_raw_parts_mut(bu as *mut u8, 64000);
        let pa = std::slice::from_raw_parts_mut(pa as *mut u8, 768);
        let (c, d, od) = match (common(iso), panel::panel_data(iso, exe), office::OfficeData::load(exe)) {
            (Ok(c), Ok(d), Ok(o)) => (c, d, o),
            (Err(e), _, _) | (_, Err(e), _) | (_, _, Err(e)) => return e,
        };
        let mut table0 = c.remap[0];
        let mut p = panel::Painter { d: &d, buf: bu, table0: &mut table0, table29: panel::table29(&c) };
        pa.copy_from_slice(&office::game_over(&od, &mut p));
        0
    })
}

/// The slot list (strategic.md "Save Game and Restore Game") on the screen `buf` (320 x 200) as it is: mode 1 = Save
/// Game, 0 = Restore Game; slot headers `used` (10 bytes, ≠ 0 = GAMEn.SAV opened), `days` (10 u16), `names` (10 x 26
/// bytes); the save name S in `name` (26 bytes, written back); the current `day`. `tape` holds the polls, each one
/// x (i16), y (i16), buttons (u8), n (u8), then n key bytes (queued before the poll). `out[0]` = the outcome (-1 not
/// ended, -2 cancel, -3 no saved games, else the slot), `out[1]` = the polls used. 0 = done.
#[no_mangle]
#[allow(clippy::too_many_arguments)]
pub unsafe extern "C" fn ja1f_slots(iso: *const c_char, exe: *const c_char, mode: i32, used: *const u8, days: *const u16,
                                    names: *const u8, name: *mut u8, day: i32, tape: *const u8, tape_len: i32,
                                    buf: *mut u8, out: *mut i32) -> i32 {
    let (Some(iso), Some(exe)) = (cstr(iso), cstr(exe)) else { return E_ARG };
    if [used as usize, days as usize, names as usize, name as usize, tape as usize, buf as usize, out as usize]
        .contains(&0) || tape_len < 0 || !(0..=1).contains(&mode)
    {
        return E_ARG;
    }
    let mut hdr = saveslots::Headers::default();
    for n in 0..10 {
        hdr.used[n] = *used.add(n) != 0;
        hdr.days[n] = *days.add(n);
        hdr.names[n].copy_from_slice(std::slice::from_raw_parts(names.add(26 * n), 26));
    }
    let tape = std::slice::from_raw_parts(tape, tape_len as usize).to_vec();
    let (na, bu, ou) = (name as usize, buf as usize, out as usize);
    guard(move || {
        let na = std::slice::from_raw_parts_mut(na as *mut u8, 26);
        let bu = std::slice::from_raw_parts_mut(bu as *mut u8, 64000);
        let ou = std::slice::from_raw_parts_mut(ou as *mut i32, 2);
        let (c, d, od) = match (common(iso), panel::panel_data(iso, exe), office::OfficeData::load(exe)) {
            (Ok(c), Ok(d), Ok(o)) => (c, d, o),
            (Err(e), _, _) | (_, Err(e), _) | (_, _, Err(e)) => return e,
        };
        let mut table0 = c.remap[0];
        let mut p = panel::Painter { d: &d, buf: bu, table0: &mut table0, table29: panel::table29(&c) };
        let mut s0 = [0u8; 26];
        s0.copy_from_slice(na);
        let mut sl = saveslots::SlotList::new(mode, hdr, s0, day);
        let (mut i, mut polls, mut res) = (0usize, 0, None);
        while i + 6 <= tape.len() && res.is_none() {
            let x = i16::from_le_bytes([tape[i], tape[i + 1]]) as i32;
            let y = i16::from_le_bytes([tape[i + 2], tape[i + 3]]) as i32;
            let (b, n) = (tape[i + 4] as i32, tape[i + 5] as usize);
            for k in 0..n {
                sl.key(tape.get(i + 6 + k).copied().unwrap_or(0));
            }
            i += 6 + n;
            polls += 1;
            res = sl.poll(&mut p, &od.slab, x, y, b);
        }
        ou[0] = match res {
            None => -1,
            Some(saveslots::Outcome::Cancel) => -2,
            Some(saveslots::Outcome::NoSaves) => -3,
            Some(saveslots::Outcome::Slot(n)) => n as i32,
        };
        ou[1] = polls;
        na.copy_from_slice(&sl.name);
        0
    })
}

/// panel.md "Speech balloon" for soldier record `rec` (512 bytes) and quote `q`: the 9 screens (4 slide-in frames,
/// the screen at the wait, 4 slide-out frames) into `out` (9 x 64000 bytes), drawn over `screen` (64000 bytes).
/// 9 = done, < 0 = error.
///
/// # Safety
/// `iso`, `exe`: NUL-terminated; `rec` 512 bytes; `screen` 64000 bytes; `out` 576000 bytes.
#[no_mangle]
pub unsafe extern "C" fn ja1f_speech_balloon(iso: *const c_char, exe: *const c_char, rec: *const u8, q: i32,
                                             text_on: i32, screen: *const u8, out: *mut u8) -> i32 {
    let (Some(iso), Some(exe)) = (cstr(iso), cstr(exe)) else { return E_ARG };
    if rec.is_null() || screen.is_null() || out.is_null() {
        return E_ARG;
    }
    let (rec, screen, out) = (rec as usize, screen as usize, out as usize);
    guard(move || {
        let r = std::slice::from_raw_parts(rec as *const u8, 512);
        let scr = std::slice::from_raw_parts(screen as *const u8, 64000);
        let out = std::slice::from_raw_parts_mut(out as *mut u8, 9 * 64000);
        match balloon::screens(iso, exe, r, q, text_on != 0, scr) {
            Ok((v, _)) => {
                for (n, f) in v.iter().enumerate() {
                    out[n * 64000..(n + 1) * 64000].copy_from_slice(f);
                }
                v.len() as i32
            }
            Err(e) => e,
        }
    })
}

/// turns.md "New turn" step 14 e: the screens of view wipe `kind` (0 = op 164 close 0x124FC, 1 = op 165 open 0x1259C
/// with the w x h picture `view`) over `screen` (64000 bytes) for the view (left, top, w, h); at most `max` screens
/// into `out` (max x 64000 bytes). Returns the number of screens, or < 0 = error.
///
/// # Safety
/// `screen` 64000 bytes; `view` w x h bytes (kind 1); `out` max x 64000 bytes.
#[no_mangle]
pub unsafe extern "C" fn ja1f_wipe(kind: i32, screen: *const u8, view: *const u8, left: i32, top: i32, w: i32, h: i32,
                                   max: i32, out: *mut u8) -> i32 {
    if screen.is_null() || out.is_null() || w <= 0 || h <= 0 || (kind == 1 && view.is_null()) {
        return E_ARG;
    }
    let r = wipe::Rect { left, top, w, h };
    let scr = std::slice::from_raw_parts(screen, 64000);
    let v = if kind == 1 { wipe::open(scr, std::slice::from_raw_parts(view, (w * h) as usize), r) } else { wipe::close(scr, r) };
    if v.len() > max.max(0) as usize {
        return E_ARG;
    }
    let out = std::slice::from_raw_parts_mut(out, v.len() * 64000);
    for (n, f) in v.iter().enumerate() {
        out[n * 64000..(n + 1) * 64000].copy_from_slice(f);
    }
    v.len() as i32
}

/// turns.md step 19: host op 167 speech_ready (CountLiveMercs? 0x410C4) over the 8 merc records `mercs` (4096 bytes).
///
/// # Safety
/// `mercs` holds 4096 bytes.
#[no_mangle]
pub unsafe extern "C" fn ja1f_live_mercs(mercs: *const u8) -> i32 {
    if mercs.is_null() {
        return E_ARG;
    }
    sighting::live_count(std::slice::from_raw_parts(mercs, 8 * 512))
}

/// turns.md "Sector line": the 9 screens of the native guide's sector line (record `rec`, NATQUOTE record `sector`)
/// over `screen` into `out` (9 x 64000 bytes); returns 9 or an error code.
///
/// # Safety
/// `iso`, `exe`: NUL-terminated; `rec` 512 bytes; `screen` 64000 bytes; `out` 576000 bytes.
#[no_mangle]
pub unsafe extern "C" fn ja1f_sector_balloon(iso: *const c_char, exe: *const c_char, rec: *const u8, sector: i32,
                                             text_on: i32, screen: *const u8, out: *mut u8) -> i32 {
    let (Some(iso), Some(exe)) = (cstr(iso), cstr(exe)) else { return E_ARG };
    if rec.is_null() || screen.is_null() || out.is_null() {
        return E_ARG;
    }
    let (rec, screen, out) = (rec as usize, screen as usize, out as usize);
    guard(move || {
        let r = std::slice::from_raw_parts(rec as *const u8, 512);
        let scr = std::slice::from_raw_parts(screen as *const u8, 64000);
        let out = std::slice::from_raw_parts_mut(out as *mut u8, 9 * 64000);
        match balloon::sector_screens(iso, exe, r, sector, text_on != 0, scr) {
            Ok((v, _)) => {
                for (n, f) in v.iter().enumerate() {
                    out[n * 64000..(n + 1) * 64000].copy_from_slice(f);
                }
                v.len() as i32
            }
            Err(e) => e,
        }
    })
}

/// panel.md "Santino's balloon": the 9 screens of Santino's balloon into `out` (9 x 64000 bytes), drawn over
/// `screen` (64000 bytes). 9 = done, < 0 = error.
///
/// # Safety
/// `iso`, `exe`: NUL-terminated; `screen` 64000 bytes; `out` 576000 bytes.
/// panel.md "The guide's and Brenda's balloons": the 9 screens of host op `op` (302 guide_say (q, c) or 317
/// brenda_say (q)) over the 320 x 200 `screen` into `out` (9 x 64000 bytes); returns 9 or an error code.
#[no_mangle]
pub unsafe extern "C" fn ja1f_npc_balloon(iso: *const c_char, exe: *const c_char, op: i32, q: i32, c: i32, screen: *const u8, out: *mut u8) -> i32 {
    let (Some(iso), Some(exe)) = (cstr(iso), cstr(exe)) else { return E_ARG };
    if screen.is_null() || out.is_null() {
        return E_ARG;
    }
    let (screen, out) = (screen as usize, out as usize);
    guard(move || {
        let scr = std::slice::from_raw_parts(screen as *const u8, 64000);
        let out = std::slice::from_raw_parts_mut(out as *mut u8, 9 * 64000);
        match balloon::npc_screens(iso, exe, op, &[q, c], scr) {
            Ok((v, _)) => {
                for (n, f) in v.iter().enumerate() {
                    out[n * 64000..(n + 1) * 64000].copy_from_slice(f);
                }
                v.len() as i32
            }
            Err(e) => e,
        }
    })
}

/// newgame_diff: the last 4 profile records of a new game (`World::new_game`; EDT\\NEWPROF.EDT bytes 5400..5760: the day
/// save holds only the first 60 records) into `out` (360 bytes). Returns 360, or an error below 0.
#[no_mangle]
pub unsafe extern "C" fn ja1f_new_game_tail(iso: *const c_char, exe: *const c_char, out: *mut u8) -> i32 {
    let (Some(iso), Some(exe)) = (cstr(iso), cstr(exe)) else { return E_ARG };
    if out.is_null() {
        return E_ARG;
    }
    let out = out as usize;
    guard(move || match crate::world::World::new_game(iso, exe, 0, 0) {
        Ok(w) => {
            std::slice::from_raw_parts_mut(out as *mut u8, 360).copy_from_slice(&w.profiles[5400..5760]);
            360
        }
        Err(_) => E_ARG,
    })
}

/// balloon_diff: the Subtitles-on waits of ops 302 / 317 (`balloon::npc_sub_screens`) into `out` (2 x i32; -1 = until a
/// key, `balloon::UNTIL_KEY`). Returns the number of parts, or an error below 0.
#[no_mangle]
pub unsafe extern "C" fn ja1f_npc_sub_waits(iso: *const c_char, exe: *const c_char, op: i32, q: i32, c: i32, out: *mut i32) -> i32 {
    let (Some(iso), Some(exe)) = (cstr(iso), cstr(exe)) else { return E_ARG };
    if out.is_null() {
        return E_ARG;
    }
    let out = out as usize;
    guard(move || {
        let out = std::slice::from_raw_parts_mut(out as *mut i32, 2);
        let scr = vec![0u8; 64000];
        match balloon::npc_sub_screens(iso, exe, op, &[q, c], &scr, "PANEL_P.PCC") {
            Ok(v) => {
                for (n, x) in v.iter().take(2).enumerate() {
                    out[n] = x.2 as i32;
                }
                v.len().min(2) as i32
            }
            Err(e) => e,
        }
    })
}

/// question_diff --face: the day-end Fire question of team slot `k` (`MapUi::payroll_fire_imgs`) after the day save
/// `save`: the three pictures (up, NO pressed, OK pressed) over `under` into `out` (3 x 64000 bytes), the merc's record
/// into `rec` (512 bytes). Returns 3, or an error below 0.
#[no_mangle]
pub unsafe extern "C" fn ja1f_payroll_fire(iso: *const c_char, exe: *const c_char, save: *const u8, save_len: usize, k: i32,
                                           under: *const u8, out: *mut u8, rec: *mut u8) -> i32 {
    let (Some(iso), Some(exe)) = (cstr(iso), cstr(exe)) else { return E_ARG };
    let (save, under, out, rec) = (save as usize, under as usize, out as usize, rec as usize);
    guard(move || {
        let files = match world_files(iso, exe) {
            Ok(f) => f,
            Err(e) => return e,
        };
        let save = std::slice::from_raw_parts(save as *const u8, save_len);
        let Ok(w) = world::World::from_parts(exe, &files.0, &files.1, save) else { return E_FILE };
        let map = mapui::MapUi::new(iso, exe, 1);
        let under = std::slice::from_raw_parts(under as *const u8, 64000);
        let k = k as usize;
        std::slice::from_raw_parts_mut(rec as *mut u8, 512).copy_from_slice(&w.soldiers[k * 0x200..(k + 1) * 0x200]);
        match map.payroll_fire_imgs(&w, k, under) {
            Ok(v) => {
                let out = std::slice::from_raw_parts_mut(out as *mut u8, 3 * 64000);
                for (n, f) in v.iter().enumerate() {
                    out[n * 64000..(n + 1) * 64000].copy_from_slice(f);
                }
                v.len() as i32
            }
            Err(e) => e,
        }
    })
}

/// balloon_diff: the speaker's quote list of host op 302 / 317 (`sound::npc_parts`, the voices that play): up to 3
/// quotes into `out`; returns their number, or -1 for another op.
#[no_mangle]
pub unsafe extern "C" fn ja1f_npc_parts(op: i32, q: i32, c: i32, out: *mut i32) -> i32 {
    match sound::npc_parts(op, &[q, c]) {
        Some((_, _, qs)) => {
            for (k, &v) in qs.iter().take(3).enumerate() {
                *out.add(k) = v;
            }
            qs.len() as i32
        }
        None => -1,
    }
}

/// balloon_diff: the last quote (6 bytes into `out`) after host op 302 guide_say (q, c) / 317 brenda_say (q) on a
/// record of 0x77 bytes (`play::npc_quote`).
#[no_mangle]
pub unsafe extern "C" fn ja1f_npc_quote(op: i32, q: i32, c: i32, out: *mut u8) -> i32 {
    let mut lq = [0x77u8; 6];
    play::npc_quote(&mut lq, op, &[q, c]);
    std::ptr::copy_nonoverlapping(lq.as_ptr(), out, 6);
    0
}

/// cursor.md "Debug keys (level 4)" 1, Ctrl+R: the full clips on the 8 merc records `soldiers` (8 x 512 bytes, changed
/// in place) with the item table `item_types` (136 x 12 bytes). Returns 0 or an error code.
#[no_mangle]
pub unsafe extern "C" fn ja1f_full_clips(soldiers: *mut u8, item_types: *const u8) -> i32 {
    if soldiers.is_null() || item_types.is_null() {
        return E_ARG;
    }
    let (s, t) = (soldiers as usize, item_types as usize);
    guard(move || {
        let s = std::slice::from_raw_parts_mut(s as *mut u8, 8 * 512);
        let t = std::slice::from_raw_parts(t as *const u8, 136 * 12);
        debugkeys::full_clips(s, t);
        0
    })
}

/// panel.md "The guide's and Brenda's balloons": the waits of host op 302 / 317 (quote q, category c) as triples
/// (quote, text time ms, hold ms) into `out` (at most 2 x 3 i32; text time -1 = until a key). Returns the wait count
/// or an error code.
#[no_mangle]
pub unsafe extern "C" fn ja1f_npc_waits(iso: *const c_char, exe: *const c_char, op: i32, q: i32, c: i32, out: *mut i32) -> i32 {
    let (Some(iso), Some(exe)) = (cstr(iso), cstr(exe)) else { return E_ARG };
    if out.is_null() {
        return E_ARG;
    }
    let out = out as usize;
    guard(move || {
        let out = std::slice::from_raw_parts_mut(out as *mut i32, 6);
        match balloon::npc_waits(iso, exe, op, &[q, c]) {
            Ok(w) => {
                for (n, &(q, t, h)) in w.iter().take(2).enumerate() {
                    out[3 * n..3 * n + 3].copy_from_slice(&[q, t as i32, h as i32]);
                }
                w.len().min(2) as i32
            }
            Err(e) => e,
        }
    })
}

/// cursor.md "Tile popups": the popup of tile `t` with `tiles` (3136 x 8 bytes), `ground` (200 x 12 bytes), `view`
/// (6 i32: view_x, view_y, view left, view top, view_w, view_h) and sector `sector` over the 320 x 200 `screen` with
/// the cursor's saved 16 x 14 block `save`: its screens into `out` (at most 20 x 64000 bytes). Returns the screen
/// count (0 = no popup) or an error code.
#[no_mangle]
pub unsafe extern "C" fn ja1f_tile_popup(iso: *const c_char, exe: *const c_char, tiles: *const u8, ground: *const u8, t: i32,
                                         view: *const i32, sector: i32, screen: *const u8, save: *const u8, out: *mut u8) -> i32 {
    let (Some(iso), Some(exe)) = (cstr(iso), cstr(exe)) else { return E_ARG };
    if tiles.is_null() || ground.is_null() || view.is_null() || screen.is_null() || save.is_null() || out.is_null() {
        return E_ARG;
    }
    let (tiles, ground, view, screen, save, out) = (tiles as usize, ground as usize, view as usize, screen as usize,
                                                    save as usize, out as usize);
    guard(move || {
        let tiles = std::slice::from_raw_parts(tiles as *const u8, 3136 * 8);
        let ground = std::slice::from_raw_parts(ground as *const u8, 2400);
        let v = std::slice::from_raw_parts(view as *const i32, 6);
        let scr = std::slice::from_raw_parts(screen as *const u8, 64000);
        let save = std::slice::from_raw_parts(save as *const u8, 224);
        let out = std::slice::from_raw_parts_mut(out as *mut u8, 20 * 64000);
        let inv = match invdraw::inv_data(iso, exe) {
            Ok(i) => i,
            Err(e) => return e,
        };
        let Some(p) = tilepopup::which(tiles, ground, &inv.item_types, t, (v[0], v[1], v[2], v[3], v[4], v[5]), sector) else {
            return 0;
        };
        match tilepopup::screens(iso, exe, &p, scr, save) {
            Ok((v, _)) => {
                for (n, f) in v.iter().take(20).enumerate() {
                    out[n * 64000..(n + 1) * 64000].copy_from_slice(f);
                }
                v.len() as i32
            }
            Err(e) => e,
        }
    })
}

/// strategic.md "The A.I.M. statistics page": the page for `soldiers` (8 x 512 bytes, the team), `accounts` (0xA8
/// bytes, after the ledger recalculation) and `rating` into `screen` (64000 bytes) and `pal` (768 bytes, 6-bit).
/// 0 = done, < 0 = error.
///
/// # Safety
/// `iso`, `exe`: NUL-terminated; the buffers have the sizes above.
#[no_mangle]
pub unsafe extern "C" fn ja1f_aim_stats(iso: *const c_char, exe: *const c_char, soldiers: *const u8, accounts: *const u8,
                                        rating: i32, screen: *mut u8, pal: *mut u8) -> i32 {
    let (Some(iso), Some(exe)) = (cstr(iso), cstr(exe)) else { return E_ARG };
    if soldiers.is_null() || accounts.is_null() || screen.is_null() || pal.is_null() {
        return E_ARG;
    }
    let (soldiers, accounts, screen, pal) = (soldiers as usize, accounts as usize, screen as usize, pal as usize);
    guard(move || {
        let soldiers = std::slice::from_raw_parts(soldiers as *const u8, 8 * 512);
        let accounts = std::slice::from_raw_parts(accounts as *const u8, 0xA8);
        let (c, d, inv) = match (common(iso), panel::panel_data(iso, exe), invdraw::inv_data(iso, exe)) {
            (Ok(c), Ok(d), Ok(i)) => (c, d, i),
            (Err(e), _, _) | (_, Err(e), _) | (_, _, Err(e)) => return e,
        };
        match aim::stats_page(&inv.main, &d, &c, soldiers, accounts, rating) {
            Some((img, p)) => {
                std::slice::from_raw_parts_mut(screen as *mut u8, 64000).copy_from_slice(&img);
                std::slice::from_raw_parts_mut(pal as *mut u8, 768).copy_from_slice(&p);
                0
            }
            None => E_FILE,
        }
    })
}

/// strategic.md "Button faces": the pressed face of A.I.M. screen button `b` (0..4) drawn over `screen` (64000 bytes,
/// in and out); `view` = the view before the press. Returns 0 or an error code.
///
/// # Safety
/// `iso`, `exe`: NUL-terminated; `screen` has 64000 bytes.
#[no_mangle]
pub unsafe extern "C" fn ja1f_aim_button(iso: *const c_char, exe: *const c_char, b: i32, view: i32, screen: *mut u8) -> i32 {
    let (Some(iso), Some(exe)) = (cstr(iso), cstr(exe)) else { return E_ARG };
    if screen.is_null() || !(0..5).contains(&b) {
        return E_ARG;
    }
    let screen = screen as usize;
    guard(move || {
        let screen = std::slice::from_raw_parts_mut(screen as *mut u8, 64000);
        let (c, d) = match (common(iso), panel::panel_data(iso, exe)) {
            (Ok(c), Ok(d)) => (c, d),
            (Err(e), _) | (_, Err(e)) => return e,
        };
        let mut table0 = c.remap[0];
        let mut p = panel::Painter { d: &d, buf: screen, table0: &mut table0, table29: panel::table29(&c) };
        aim::pressed_button(&mut p, b as usize, view as u8);
        0
    })
}

/// strategic.md "The salary panel": the panel of team record `k` (0..7) of `soldiers` (8 x 512 bytes) drawn over
/// `screen` (64000 bytes, in and out). Returns 0 or an error code.
///
/// # Safety
/// `iso`, `exe`: NUL-terminated; the buffers have the sizes above.
#[no_mangle]
pub unsafe extern "C" fn ja1f_aim_salary(iso: *const c_char, exe: *const c_char, soldiers: *const u8, k: i32,
                                         screen: *mut u8) -> i32 {
    let (Some(iso), Some(exe)) = (cstr(iso), cstr(exe)) else { return E_ARG };
    if soldiers.is_null() || screen.is_null() || !(0..8).contains(&k) {
        return E_ARG;
    }
    let (soldiers, screen) = (soldiers as usize, screen as usize);
    guard(move || {
        let soldiers = std::slice::from_raw_parts(soldiers as *const u8, 8 * 512);
        let screen = std::slice::from_raw_parts_mut(screen as *mut u8, 64000);
        let (c, d, inv) = match (common(iso), panel::panel_data(iso, exe), invdraw::inv_data(iso, exe)) {
            (Ok(c), Ok(d), Ok(i)) => (c, d, i),
            (Err(e), _, _) | (_, Err(e), _) | (_, _, Err(e)) => return e,
        };
        let k = k as usize;
        let mut table0 = c.remap[0];
        let mut p = panel::Painter { d: &d, buf: screen, table0: &mut table0, table29: panel::table29(&c) };
        aim::salary_panel(&mut p, &inv, k, &aim::salary_values(&soldiers[k * 512..]));
        0
    })
}

/// panel.md "The travel animation": row `row` of archive `arc` (1 = SANIM.DAT, 2 = SANIM2.DAT) with `number` (< 0 =
/// none): its frames 0 .. n − 1 (w x h bytes each)
/// into `out` (at most 64 x 3000 bytes) and its text (40 bytes, 0-filled) into `text`. Returns n or an error code.
///
/// # Safety
/// `iso`, `exe`: NUL-terminated; the buffers have the sizes above.
#[no_mangle]
pub unsafe extern "C" fn ja1f_sanim(iso: *const c_char, exe: *const c_char, arc: i32, row: i32, number: i32,
                                    out: *mut u8, text: *mut u8) -> i32 {
    let (Some(iso), Some(exe)) = (cstr(iso), cstr(exe)) else { return E_ARG };
    if out.is_null() || text.is_null() {
        return E_ARG;
    }
    let (out, text) = (out as usize, text as usize);
    guard(move || {
        let name = if arc == 1 { "SANIM.DAT" } else { sanim::DAY_ARC };
        let a = match sanim::load(iso, exe, name, row, if number < 0 { None } else { Some(number) }) {
            Ok(a) => a,
            Err(e) => return e,
        };
        if a.n > 64 || a.w * a.h > 3000 {
            return E_ARG;
        }
        let out = std::slice::from_raw_parts_mut(out as *mut u8, 64 * 3000);
        for (k, f) in a.frames.iter().enumerate() {
            out[k * a.w * a.h..(k + 1) * a.w * a.h].copy_from_slice(f);
        }
        let t = std::slice::from_raw_parts_mut(text as *mut u8, 40);
        t.fill(0);
        t[..a.text.len()].copy_from_slice(&a.text);
        a.n as i32
    })
}

#[no_mangle]
pub unsafe extern "C" fn ja1f_santino_balloon(iso: *const c_char, exe: *const c_char, screen: *const u8, out: *mut u8) -> i32 {
    let (Some(iso), Some(exe)) = (cstr(iso), cstr(exe)) else { return E_ARG };
    if screen.is_null() || out.is_null() {
        return E_ARG;
    }
    let (screen, out) = (screen as usize, out as usize);
    guard(move || {
        let scr = std::slice::from_raw_parts(screen as *const u8, 64000);
        let out = std::slice::from_raw_parts_mut(out as *mut u8, 9 * 64000);
        match balloon::santino_screens(iso, exe, &balloon::santino_text(iso, exe, 0), scr) {
            Ok((v, _)) => {
                for (n, f) in v.iter().enumerate() {
                    out[n * 64000..(n + 1) * 64000].copy_from_slice(f);
                }
                v.len() as i32
            }
            Err(e) => e,
        }
    })
}

/// panel.md "Speech balloon" step 7 and "Santino's balloon": the wait without a voice (ms) of the balloon of record
/// `rec`, quote `q`; a null `rec` = Santino. Negative = error.
#[no_mangle]
pub unsafe extern "C" fn ja1f_balloon_wait(iso: *const c_char, exe: *const c_char, rec: *const u8, q: i32, text_on: i32) -> i32 {
    let (Some(iso), Some(exe)) = (cstr(iso), cstr(exe)) else { return E_ARG };
    let rec = rec as usize;
    guard(move || {
        let scr = vec![0u8; 64000];
        let got = if rec == 0 {
            balloon::santino_screens(iso, exe, &balloon::santino_text(iso, exe, 0), &scr)
        } else {
            balloon::screens(iso, exe, std::slice::from_raw_parts(rec as *const u8, 512), q, text_on != 0, &scr)
        };
        match got {
            Ok((_, w)) => w as i32,
            Err(e) => e,
        }
    })
}

/// The nickname of the guide offer's box for profile `p` over the `len` bytes of `profiles`, into `out` (16 bytes).
/// Returns its length.
#[no_mangle]
pub unsafe extern "C" fn ja1f_guide_nick(profiles: *const u8, len: usize, p: i32, out: *mut u8) -> i32 {
    if profiles.is_null() || out.is_null() {
        return E_ARG;
    }
    let (profiles, out) = (profiles as usize, out as usize);
    guard(move || {
        let n = mapui::guide_nick(std::slice::from_raw_parts(profiles as *const u8, len), p as u8);
        let k = n.len().min(16);
        std::slice::from_raw_parts_mut(out as *mut u8, 16)[..k].copy_from_slice(&n[..k]);
        k as i32
    })
}

/// The overview picture (panel.md "The overview picture") drawn into a given back buffer.
/// flags: bit 0 overview filter, bit 1 roofs on, bit 2 "show all".
///
/// # Safety
/// `iso_path`, `exe_path`: NUL-terminated paths; `tiles`: 3136 x 8 bytes; `items`: 200 x 12; `soldiers`: 48 x 512;
/// `fx`: 16 x 18; `bb`: 384 x 854 bytes, read and written. `cur`: the overview cursor (step 4) or null (no cursor);
/// `table0`: remap table 0 (256 bytes) or null (the game's table).
#[no_mangle]
#[allow(clippy::too_many_arguments)]
pub unsafe extern "C" fn ja1f_overview(
    iso_path: *const c_char,
    exe_path: *const c_char,
    tileset: i32,
    tiles: *const u8,
    items: *const u8,
    soldiers: *const u8,
    fx: *const u8,
    sees_slot: i32,
    flags: i32,
    bb: *mut u8,
    cur: *const overview::OvCursor,
    table0: *const u8,
) -> i32 {
    let (Some(iso), Some(exe)) = (cstr(iso_path), cstr(exe_path)) else { return E_ARG };
    if tiles.is_null() || items.is_null() || soldiers.is_null() || fx.is_null() || bb.is_null() || !(0..10).contains(&tileset) {
        return E_ARG;
    }
    let tiles = std::slice::from_raw_parts(tiles, 3136 * 8).to_vec();
    let items = std::slice::from_raw_parts(items, 200 * 12).to_vec();
    let soldiers = std::slice::from_raw_parts(soldiers, 48 * 512).to_vec();
    let fx = std::slice::from_raw_parts(fx, 16 * 18).to_vec();
    let cur = if cur.is_null() { None } else { Some(*cur) };
    let t0 = if table0.is_null() { None } else { Some(std::slice::from_raw_parts(table0, 256).to_vec()) };
    let n = (render::PITCH * render::HEIGHT) as usize;
    let bbp = bb as usize;
    guard(move || {
        let run = || -> Result<(), i32> {
            let (c, e, t) = (common(iso)?, exe_tables(exe)?, tileset_data(iso, tileset as u8)?);
            let (pd, fxs) = (panel::panel_data(iso, exe)?, fx_sheets(exe)?);
            let buf = std::slice::from_raw_parts_mut(bbp as *mut u8, n);
            let w = overview::OverviewIn { tiles: &tiles, items: &items, soldiers: &soldiers, fx: &fx,
                sees_slot: sees_slot as u8, filter: flags & 1 != 0, roofs: flags & 2 != 0, show_all: flags & 4 != 0 };
            overview::draw(buf, &t, &e, &c, &pd.sheet, &fxs, &w);
            if let Some(cur) = &cur {
                let mut tab = c.remap[0];
                if let Some(t0) = &t0 {
                    tab.copy_from_slice(t0);
                }
                overview::cursor(buf, &t, &pd.fonts[0], &mut tab, cur);
            }
            Ok(())
        };
        match run() { Ok(()) => 0, Err(e) => e }
    })
}

/// The experience count (strategic.md "The experience count") in `panel` (P, 320 x 200, changed) with `bg` = B
/// (320 x 146, BASSMAP.PCC). `esc` = `n_esc` bytes, one per number counted (Escape waiting; past the end: 0);
/// `fast` = the fast flag (in and out). The events go to `evs` (at most `cap` pairs: kind 1 Show with the shot
/// index, 2 Wait with ms, 3 Click with 0) and Show j's rows 0..145 to `shots` + 46720 j (at most `cap_shots`).
/// Returns the number of events, or -2 when a buffer is too small.
#[no_mangle]
pub unsafe extern "C" fn ja1f_xp_count(iso: *const c_char, exe: *const c_char, panel: *mut u8, bg: *const u8,
                                       slot: i32, stat: i32, value: i32, delta: i32, fast: *mut i32,
                                       esc: *const u8, n_esc: i32, evs: *mut i32, cap: i32, shots: *mut u8,
                                       cap_shots: i32) -> i32 {
    let (Some(iso), Some(exe)) = (cstr(iso), cstr(exe)) else { return E_ARG };
    if panel.is_null() || bg.is_null() || fast.is_null() || evs.is_null() || shots.is_null() || !(0..10).contains(&stat) {
        return E_ARG;
    }
    let (pa, bgp, fa, es, ev, sh) = (panel as usize, bg as usize, fast as usize, esc as usize, evs as usize, shots as usize);
    guard(move || {
        let pa = std::slice::from_raw_parts_mut(pa as *mut u8, 64000);
        let bg = std::slice::from_raw_parts(bgp as *const u8, 320 * 146);
        let es: &[u8] = if es == 0 || n_esc <= 0 { &[] } else { std::slice::from_raw_parts(es as *const u8, n_esc as usize) };
        let fast = &mut *(fa as *mut i32);
        let (c, d) = match (common(iso), panel::panel_data(iso, exe)) {
            (Ok(c), Ok(d)) => (c, d),
            (Err(e), _) | (_, Err(e)) => return e,
        };
        let mut table0 = c.remap[0];
        let mut p = panel::Painter { d: &d, buf: pa, table0: &mut table0, table29: panel::table29(&c) };
        let mut f = *fast != 0;
        let mut i = 0;
        let mut esc = || {
            i += 1;
            es.get(i - 1).copied().unwrap_or(0) != 0
        };
        let out = xpcount::count(&mut p, bg, slot as u8, stat as usize, value, delta, &mut f, &mut esc);
        *fast = f as i32;
        let n_shows = out.iter().filter(|e| matches!(e, xpcount::Ev::Show(_))).count();
        if out.len() > cap as usize || n_shows > cap_shots as usize {
            return -2;
        }
        let ev = std::slice::from_raw_parts_mut(ev as *mut i32, 2 * out.len());
        let sh = std::slice::from_raw_parts_mut(sh as *mut u8, n_shows * 46720);
        let mut j = 0;
        for (k, e) in out.iter().enumerate() {
            let (a, b) = match e {
                xpcount::Ev::Show(s) => {
                    sh[j * 46720..(j + 1) * 46720].copy_from_slice(s);
                    j += 1;
                    (1, j as i32 - 1)
                }
                xpcount::Ev::Wait(ms) => (2, *ms as i32),
                xpcount::Ev::Click => (3, 0),
            };
            ev[2 * k] = a;
            ev[2 * k + 1] = b;
        }
        out.len() as i32
    })
}

/// The FM chip imitation alone (validate/diff/oplchip_diff.py): `script` = `n` triples of i32: (0, register, value) writes a
/// register, (1, ms, 0) makes ms milliseconds of mono output at sound::RATE. Returns the number of samples, -2 when `cap` is
/// too small.
#[no_mangle]
pub unsafe extern "C" fn ja1f_opl_render(script: *const i32, n: i32, out: *mut f32, cap: i32) -> i32 {
    if script.is_null() || out.is_null() || n < 0 || cap < 0 {
        return E_ARG;
    }
    let script = std::slice::from_raw_parts(script, n as usize * 3);
    let out = std::slice::from_raw_parts_mut(out, cap as usize);
    let mut chip = fm::Opl::default();
    let mut k = 0;
    for t in script.chunks(3) {
        if t[0] == 0 {
            chip.write(t[1] as u8, t[2] as u8);
        } else {
            for _ in 0..t[1] as i64 * sound::RATE as i64 / 1000 {
                let Some(o) = out.get_mut(k) else { return -2 };
                *o = chip.sample();
                k += 1;
            }
        }
    }
    k as i32
}

/// music.md "The FM driver": the driver's register writes after its start (time -2), the AIL install reset (-1),
/// the install of `tims` (`ntims` bank, patch pairs; time 0, `inst` = 1 all installed, 0 one missing) and the
/// `nmsgs` messages of `msgs` (3 bytes each, status first; time k = message k, 1..). `out` gets 4 i32 per write:
/// time, chip (0 left, 1 right), register, value. Returns the number of writes, -2 when `cap` writes are too few.
#[no_mangle]
pub unsafe extern "C" fn ja1f_mdi(bank: *const c_char, tims: *const u8, ntims: i32, msgs: *const u8, nmsgs: i32,
                                  inst: *mut i32, out: *mut i32, cap: i32) -> i32 {
    let Some(bank) = cstr(bank) else { return E_ARG };
    if tims.is_null() || msgs.is_null() || inst.is_null() || out.is_null() || ntims < 0 || nmsgs < 0 {
        return E_ARG;
    }
    let Ok(b) = std::fs::read(bank) else { return E_FILE };
    let mut d = fm::Driver::new(fm::parse_bank(&b), true);
    let mut rows: Vec<[i32; 4]> = vec![];
    let take = |d: &mut fm::Driver, rows: &mut Vec<[i32; 4]>, t: i32| {
        rows.extend(d.writes.drain(..).map(|(c, r, v)| [t, c as i32, r as i32, v as i32]));
    };
    take(&mut d, &mut rows, -2);
    d.ail_reset();
    take(&mut d, &mut rows, -1);
    let t = std::slice::from_raw_parts(tims, 2 * ntims as usize);
    let list: Vec<(u8, u8)> = t.chunks(2).map(|p| (p[0], p[1])).collect();
    *inst = d.install(&list) as i32;
    take(&mut d, &mut rows, 0);
    let m = std::slice::from_raw_parts(msgs, 3 * nmsgs as usize);
    for (k, x) in m.chunks(3).enumerate() {
        d.msg(x[0], x[1], x[2]);
        take(&mut d, &mut rows, k as i32 + 1);
    }
    if rows.len() > cap.max(0) as usize {
        return E_SECTOR;
    }
    for (i, r) in rows.iter().enumerate() {
        std::ptr::copy_nonoverlapping(r.as_ptr(), out.add(4 * i), 4);
    }
    rows.len() as i32
}

/// TacticalKeyHandler's screen-pixel to map-tile conversion (original 0x159CC).
#[no_mangle]
pub extern "C" fn ja1f_tactical_tile(c0: i32, r0: i32, vx: i32, vy: i32) -> i32 {
    viewmode::tactical_tile(c0, r0, vx, vy)
}

/// Test interface of CursorModeSet? 0x1C438: the mode setter 0x284F1B (a mode above 2 becomes 1), then
/// MoveCursorState 0x1C464 of soldier `slot`. Returns (mode << 8) | value after the call, or -1 on a bad argument.
///
/// MoveCursorState 0x1C464 with the mode byte as it stands (key handler 0x14112 sets 3 with no setter). For the test.
/// # Safety
/// `w` / `h` valid library world and host; `st` valid.
#[no_mangle]
pub unsafe extern "C" fn ja1f_cursor_move_state(
    w: *mut host::Ja1World,
    h: *const host::Ja1Host,
    st: *mut cursor::CursorState,
    slot: i32,
) -> i32 {
    if w.is_null() || h.is_null() || st.is_null() {
        return -1;
    }
    let (wp, hp, sp) = (w as usize, h as usize, st as usize);
    guard(move || {
        // SAFETY: the caller passes valid pointers.
        let s = &mut *(sp as *mut cursor::CursorState);
        cursor::move_cursor_state(s, wp as *mut host::Ja1World, hp as *const host::Ja1Host, slot as usize);
        ((s.mode as i32) << 8) | s.value as i32
    })
}

/// # Safety
/// `w` / `h` valid library world and host; `st` valid.
#[no_mangle]
pub unsafe extern "C" fn ja1f_cursor_mode_set(
    w: *mut host::Ja1World,
    h: *const host::Ja1Host,
    st: *mut cursor::CursorState,
    slot: i32,
    mode: i32,
) -> i32 {
    if w.is_null() || h.is_null() || st.is_null() {
        return -1;
    }
    let (wp, hp, sp) = (w as usize, h as usize, st as usize);
    guard(move || {
        // SAFETY: the caller passes valid pointers.
        let s = &mut *(sp as *mut cursor::CursorState);
        cursor::click_panel(s, wp as *mut host::Ja1World, hp as *const host::Ja1Host, slot, mode);
        ((s.mode as i32) << 8) | s.value as i32
    })
}

/// PictureSize? 0x46AE4: the item picture size class of picture id `pic` (width << 8 | x offset in the 42-column
/// cursor buffer), the size used when a held item is drawn.
#[no_mangle]
pub extern "C" fn ja1f_picture_size(pic: i32) -> i32 {
    let (w, dx) = invdraw::picture_size(pic as u8);
    ((w as i32) << 8) | dx
}

/// PaletteRotate? 0x528AC: rotate the palette entries `first`..`last` of a 768-byte RGB palette by one toward
/// the lower index (host op 206 pal_cycle).
///
/// # Safety
/// `pal` points to a 768-byte palette.
#[no_mangle]
pub unsafe extern "C" fn ja1f_pal_rotate(pal: *mut u8, first: i32, last: i32) {
    if pal.is_null() {
        return;
    }
    // SAFETY: the caller passes a 768-byte palette.
    eod::rotate(&mut *(pal as *mut [u8; 768]), first, last);
}

/// The island map's water rotation (`mapui::rotate_water`) on the 768-byte palette `pal`.
///
/// # Safety
/// `pal` points to a 768-byte palette.
#[no_mangle]
pub unsafe extern "C" fn ja1f_map_water(pal: *mut u8) {
    if pal.is_null() {
        return;
    }
    // SAFETY: the caller passes a 768-byte palette.
    mapui::rotate_water(&mut *(pal as *mut [u8; 768]));
}

/// FadePalette? 0x52908: the fade levels of the palette `pal` (768 bytes) over `n` steps from level `from` to
/// `to` (0..256), each level 768 bytes, one per vertical retrace; returns the level count, or -1 on a bad call.
///
/// # Safety
/// `pal` points to a 768-byte palette; `out` points to `cap` levels of 768 bytes.
#[no_mangle]
pub unsafe extern "C" fn ja1f_fade(pal: *const u8, n: i32, from: i32, to: i32, out: *mut u8, cap: i32) -> i32 {
    if pal.is_null() || out.is_null() || n < 1 || cap < 0 {
        return -1;
    }
    // SAFETY: the caller passes a 768-byte palette and cap levels of 768 bytes.
    let p: &[u8; 768] = &*(pal as *const [u8; 768]);
    let levels = office::fade(p, n, from, to);
    let k = levels.len().min(cap as usize);
    let dst = std::slice::from_raw_parts_mut(out, k * 768);
    for (i, lv) in levels.iter().take(k).enumerate() {
        dst[i * 768..(i + 1) * 768].copy_from_slice(lv);
    }
    levels.len() as i32
}

/// OverviewTileAtMouse 0x24488: the overview tile (0..3135) at screen point (x, y), 4 x 3 pixels per tile.
#[no_mangle]
pub extern "C" fn ja1f_overview_tile(x: i32, y: i32) -> i32 {
    viewmode::overview_tile(x, y)
}

/// One poll of the tape of the strategic screen's input loop (x, y, buttons).
const MAP_POLL: usize = 3;
/// The clock of the tape: every poll takes this time (the water check of `ja1f_map_pass`).
const MAP_POLL_MS: i64 = 100;

/// One water check of the loop at the time of poll `q` (`MAP_POLL_MS` per poll): event 8 (0, 0, q) after a rotation.
fn map_water(map: &mut mapui::MapUi, q: usize, ev: &mut [i32], nl: &mut usize) -> Result<(), i32> {
    if map.cycle(q as i64 * MAP_POLL_MS) {
        if *nl * 4 + 4 > ev.len() {
            return Err(E_ARG);
        }
        ev[*nl * 4..*nl * 4 + 4].copy_from_slice(&[8, 0, 0, q as i32]);
        *nl += 1;
    }
    Ok(())
}

/// One WaitLeftUp / WaitRightUp of the click block: poll until the button `bit` is up (at least once), as the
/// original does at 0x67C54 / 0x67C68.
unsafe fn map_wait_up(map: &mut mapui::MapUi, w: &mut world::World, ta: &[i32], q: &mut usize, bit: i32,
                      prev: &mut i32, ev: &mut [i32], nl: &mut usize) -> Result<bool, i32> {
    loop {
        if *q >= ta.len() / MAP_POLL {
            return Ok(false);
        }
        let (x, y, b) = (ta[MAP_POLL * *q], ta[MAP_POLL * *q + 1], ta[MAP_POLL * *q + 2]);
        *q += 1;
        // The window's calls of this poll: while the press holds, they must change nothing.
        if map.map_held() {
            map.track(w, (x, y))?;
            if b & 3 & !*prev != 0 {
                map.click(w, b & 3 & !*prev)?;
            }
            map_water(map, *q, ev, nl)?;
            if *prev & !b & 3 != 0 {
                map.release(w, *prev & !b & 3)?;
            }
            *prev = b;
            if b & bit == 0 {
                return Ok(true);
            }
            continue;
        }
        *prev = b;
        if b & bit == 0 {
            map.release(w, bit)?;
            return Ok(true);
        }
    }
}

/// The financial page's flip runs since the last call, as events: (3, first picture, count) and (4, step, 0), with
/// no poll index. The original's PlayFlipAnim call of the open runs at the field 4 press and the close at the back
/// press; the port opens the page at the field 4 release and closes it at the back release (its button boxes act on
/// the release), so a poll index would differ by one. The count of polls used still compares the tape as a whole.
unsafe fn map_flip_events(map: &mut mapui::MapUi, ev: &mut [i32], nl: &mut usize) -> Result<(), i32> {
    for (first, count, step) in std::mem::take(&mut map.flip_log) {
        if (*nl + 2) * 4 > ev.len() {
            return Err(E_ARG);
        }
        ev[*nl * 4..*nl * 4 + 4].copy_from_slice(&[3, first, count, 0]);
        *nl += 1;
        ev[*nl * 4..*nl * 4 + 4].copy_from_slice(&[4, step, 0, 0]);
        *nl += 1;
    }
    Ok(())
}

/// The strategic screen's input loop (StrategicScreen 0x432F4) on a tape of mouse polls: the map area, the
/// panel's tab row and its start button, the port side of validate/diff/strat_input_diff.py.
///
/// The world comes from `save`; `sectors` (720 bytes), `accounts` (0xA8), `plants` (48), `sector_order` (61),
/// `cur`, `team` and the hint flag `hinted` replace its state, so both sides start from the same fuzzed globals.
/// The tape holds `n` polls of (x, y, buttons): map clicks at y <= 140, panel tab presses at y >= 141. Events, 4
/// ints each: 0 (sector, colour, poll) a sector frame (FlashSectorFrame), 1 (text offset, text length, poll) a
/// message box, 2 (op, first argument, poll) a guide quote (host op 302), 3 (first picture, count, 0) and 4
/// (step, 0, 0) a finance page flip run (PlayFlipAnim). `counts` = [events, warned, cur_sector, polls used, mode,
/// hinted]. `texts` holds the message strings back to back. Returns 0 when done, or E_ARG.
///
/// The day-start question of a start press whose hint is already shown is answered NO by the harness's
/// ConfirmDialog stub (0x275E4), so the original stays in its loop; the port does too (it runs no day start).
///
/// # Safety
/// The caller passes the buffers described above.
#[no_mangle]
pub unsafe extern "C" fn ja1f_map_pass(
    iso: *const c_char, exe: *const c_char, save: *const c_char, sectors: *mut u8, accounts: *mut u8,
    plants: *mut u8, order: *mut u8, cur: i32, team: i32, mode: i32, hinted: i32, tape: *const i32, n: i32,
    events: *mut i32, cap: i32, counts: *mut i32, texts: *mut u8, tcap: i32,
) -> i32 {
    let (Some(iso), Some(exe), Some(save)) = (cstr(iso), cstr(exe), cstr(save)) else { return E_ARG };
    if sectors.is_null() || accounts.is_null() || plants.is_null() || order.is_null() || tape.is_null()
        || events.is_null() || counts.is_null() || texts.is_null() || n < 0 || cap < 0 || tcap < 0
    {
        return E_ARG;
    }
    let (se, ac, pl, or, ta, ev, ct, tx) = (sectors as usize, accounts as usize, plants as usize, order as usize,
                                            tape as usize, events as usize, counts as usize, texts as usize);
    guard(move || {
        // SAFETY: the caller passes the buffers the doc comment describes (720 + 0xA8 + 48 + 61 bytes, 3 n tape
        // ints, 4 cap event ints, 6 count ints, tcap text bytes).
        let se = std::slice::from_raw_parts_mut(se as *mut u8, 720);
        let ac = std::slice::from_raw_parts_mut(ac as *mut u8, 0xA8);
        let pl = std::slice::from_raw_parts(pl as *const u8, 48);
        let or = std::slice::from_raw_parts(or as *const u8, 61);
        let ta = std::slice::from_raw_parts(ta as *const i32, MAP_POLL * n as usize);
        let ev = std::slice::from_raw_parts_mut(ev as *mut i32, 4 * cap as usize);
        let ct = std::slice::from_raw_parts_mut(ct as *mut i32, 6);
        let tx = std::slice::from_raw_parts_mut(tx as *mut u8, tcap as usize);
        let base = match world(iso, exe, save) {
            Ok(w) => w,
            Err(e) => return e,
        };
        let mut w = (*base).clone();
        if w.sector_state.len() < 720 || w.accounts.len() < 0xA8 || w.plants.len() < 48 || w.sector_order.len() < 61 {
            return E_ARG;
        }
        w.sector_state[..720].copy_from_slice(se);
        w.accounts[..0xA8].copy_from_slice(ac);
        w.plants[..48].copy_from_slice(pl);
        w.sector_order[..61].copy_from_slice(or);
        w.cur_sector = cur as u8;
        w.team_on_duty = team;
        w.hinted = hinted as u8;
        let mut map = mapui::MapUi::new(iso, exe, 1);
        // StrategicScreen is the day loop's map: its panel's start button opens the day end.
        map.day = true;
        // The view opens with the mouse off the map (y 200): no sector framed, as the original's loop starts
        // with its current sector at 0x3C.
        if let Err(e) = map.show(&mut w, (0, 200)) {
            return e;
        }
        map.set_mode(mode);
        let (mut nl, mut toff, mut seen, mut q, mut prev) = (0usize, 0usize, 0usize, 0usize, 0i32);
        let mut asked = false;
        'polls: loop {
            // The end of the loop's round: StrategicScreen's water check 0x43E07.
            if let Err(e) = map_water(&mut map, q, ev, &mut nl) {
                return e;
            }
            if q >= n as usize {
                break;
            }
            let (x, y, btn) = (ta[MAP_POLL * q], ta[MAP_POLL * q + 1], ta[MAP_POLL * q + 2]);
            q += 1;
            let qi = (q - 1) as i32;
            if let Err(e) = map.track(&mut w, (x, y)) {
                return e;
            }
            for &(s, c) in &map.frame_log[seen..] {
                if nl * 4 + 4 > ev.len() {
                    return E_ARG;
                }
                ev[nl * 4..nl * 4 + 4].copy_from_slice(&[0, s, c as i32, qi]);
                nl += 1;
            }
            seen = map.frame_log.len();
            // A button that comes up on this poll: the finance page's back button and the number fields act on the
            // release (the window calls track, then release). The map-area clicks run WaitLeftUp instead, in
            // `map_wait_up` below, so their release polls never reach this point.
            let up = prev & !btn & 3;
            if up != 0 {
                for bit in [1, 2] {
                    if up & bit != 0 {
                        if let Err(e) = map.release(&mut w, bit) {
                            return e;
                        }
                    }
                }
                if let Err(e) = map_flip_events(&mut map, ev, &mut nl) {
                    return e;
                }
                // The assignment screen's opening and closing pictures need no time here.
                map.drain();
                // Its question "Hmmm. No one assigned to duty?" opens at the release of Exit.
                let q = map.no_duty_asked();
                if q && !asked {
                    if nl * 4 + 4 > ev.len() {
                        return E_ARG;
                    }
                    ev[nl * 4..nl * 4 + 4].copy_from_slice(&[6, 0, 0, 0]);
                    nl += 1;
                }
                asked = q;
                asked = q;
            }
            let press = btn & 3 & !prev;
            prev = btn;
            if press != 0 {
                // The wage question's box (`asking`) holds the screen: its press is the answer and the original
                // reads it in its ConfirmDialog (which this harness stubs with its own answer). The map and the
                // panel run nothing while it shows, so the press takes no start check and no click waits.
                let asking = map.asking();
                let plants = map.plants_shown();
                // The assignment screen (AssignScreen 0x4891C) owns its presses: no start check, no click waits.
                let assigning = map.assigning();
                if let Err(e) = map.click(&mut w, press) {
                    return e;
                }
                if asking || assigning {
                    continue 'polls;
                }
                // Tab 3's processing page (DrawProcessingUsage 0x42148), shown by the press and held until the
                // button comes up; the original draws it in the press poll.
                if !plants && map.plants_shown() {
                    if nl * 4 + 4 > ev.len() {
                        return E_ARG;
                    }
                    ev[nl * 4..nl * 4 + 4].copy_from_slice(&[5, 0, 0, qi]);
                    nl += 1;
                }
                // The guide's quote of the click (host op 302), taken now: the original shows its message box
                // first and speaks after it, so the event goes out after the box events below.
                let said = map.take_said();
                // The panel's start button (field 7): the original checks ButtonAt(7,1) on every panel poll
                // (0x4386C) and shows its message in the loop's StatusMessage box. 0 = the day start path: the
                // original asks its Get to work? question there (ConfirmDialog 0x275E4), which this harness stubs
                // with the NO answer, so the loop goes on. The port runs no day start here.
                if y >= 141 && press & 1 != 0 && !map.box_shown() {
                    match map.start_pressed(&mut w, (x, y)) {
                        Ok(-1) => {}
                        Ok(0) => {}
                        Ok(n) => {
                            if let Err(e) = map.set_msg(&w, n) {
                                return e;
                            }
                        }
                        Err(e) => return e,
                    }
                }
                // The message boxes of the click, one after the other; each waits for a new mouse press (the
                // tape model of StatusMessage: a key would end it too, but this tape has no keys).
                let mut done = false;
                while map.box_shown() {
                    if let Some(t) = map.box_text() {
                        let b = t.as_bytes();
                        if toff + b.len() > tx.len() || nl * 4 + 4 > ev.len() {
                            return E_ARG;
                        }
                        tx[toff..toff + b.len()].copy_from_slice(b);
                        ev[nl * 4..nl * 4 + 4].copy_from_slice(&[1, toff as i32, b.len() as i32, qi]);
                        nl += 1;
                        toff += b.len();
                    }
                    let mut closed = false;
                    while q < n as usize {
                        let bb = ta[MAP_POLL * q + 2];
                        q += 1;
                        if bb & 3 != 0 {
                            for i in [mapui::BoxInput::Down(1), mapui::BoxInput::Up(1)] {
                                if let Err(e) = map.box_input(i) {
                                    return e;
                                }
                            }
                            closed = true;
                            // KeyOrClick 0x67E44 waits until the button is up.
                            while q < n as usize && ta[MAP_POLL * q + 2] & 3 != 0 {
                                q += 1;
                            }
                            break;
                        }
                    }
                    if !closed {
                        done = true;
                        break;
                    }
                }
                for (op, a) in said {
                    if nl * 4 + 4 > ev.len() {
                        return E_ARG;
                    }
                    // Host op 302 (guide_say) is the original's GuideSpeech(quote, category); any other op keeps
                    // its number as the first value.
                    let (x, y) = if op == 302 {
                        (a.first().copied().unwrap_or(0), a.get(1).copied().unwrap_or(0))
                    } else {
                        (op, 0)
                    };
                    ev[nl * 4..nl * 4 + 4].copy_from_slice(&[2, x, y, qi]);
                    nl += 1;
                }
                if done {
                    break 'polls;
                }
                // The WaitLeftUp and WaitRightUp of the click block, except the enemy-hands and not-linked
                // messages (1 and 2), which return before the waits. The map panel's branch (mouse y >= 141) has
                // no waits: the loop polls the release itself, so the same tape entry must not be eaten here.
                let m = map.last_click_msg();
                if y < 141 && m != 1 && m != 2 {
                    prev = btn;
                    for bit in [1, 2] {
                        match map_wait_up(&mut map, &mut w, ta, &mut q, bit, &mut prev, ev, &mut nl) {
                            Ok(true) => {}
                            Ok(false) => break 'polls,
                            Err(e) => return e,
                        }
                    }
                    // Both waits end on a button-up poll; the edge detector must not keep the old press.
                    prev = 0;
                }
            }
        }
        se.copy_from_slice(&w.sector_state[..720]);
        ac.copy_from_slice(&w.accounts[..0xA8]);
        for &(x0, y0, x1, y1) in &map.limit_log {
            if nl * 4 + 4 > ev.len() {
                return E_ARG;
            }
            ev[nl * 4..nl * 4 + 4].copy_from_slice(&[7, (y0 << 16) | x0, (y1 << 16) | x1, 0]);
            nl += 1;
        }
        ct[0] = nl as i32;
        ct[1] = map.warned() as i32;
        ct[2] = w.cur_sector as i32;
        ct[3] = q as i32;
        ct[4] = map.mode();
        ct[5] = w.hinted as i32;
        0
    })
}

/// The mouse on the merc slots over a tape (slotmouse_diff). `soldiers`: 8 records. `state` = [cont, hover, last, over,
/// selected, alt, where]: where 0 = off the map, 1 = on the map, 2 = off the map with g_redrawOff 2, 3 = 0x92DCC at 0
/// (`pass_runs`); written back. `tape`: n x [x, y, buttons]; entry 0 is the pass, the rest the open switch
/// view. `ans` = [on screen, use cost, AP enough, ground item]. `out`: calls x 8 ints. Answers the number of calls.
#[no_mangle]
pub unsafe extern "C" fn ja1f_slot_mouse(soldiers: *mut u8, state: *mut i32, tape: *const i32, n: i32, ans: *const i32,
                                         out: *mut i32, cap: i32) -> i32 {
    use slotmouse::{Act, Call, ContPlan, Mouse, SlotMouse};
    if soldiers.is_null() || state.is_null() || tape.is_null() || ans.is_null() || out.is_null() || n < 1 {
        return E_ARG;
    }
    let (so, st, ta, an, ou) = (soldiers as usize, state as usize, tape as usize, ans as usize, out as usize);
    guard(move || {
        let soldiers = std::slice::from_raw_parts_mut(so as *mut u8, 8 * soldier::REC);
        let st = std::slice::from_raw_parts_mut(st as *mut i32, 7);
        let tape = std::slice::from_raw_parts(ta as *const i32, 3 * n as usize);
        let an = std::slice::from_raw_parts(an as *const i32, 4);
        let out = std::slice::from_raw_parts_mut(ou as *mut i32, 8 * cap as usize);
        let (selected, alt) = (st[4] as u8, st[5]);
        let mut sm = SlotMouse { cont: st[0] as u8, hover: st[1] as u8, last: st[2] as u8, over: st[3] as u8 };
        let mut log: Vec<[i32; 8]> = Vec::new();
        let log_cell = std::cell::RefCell::new(&mut log);
        let flush = |calls: &mut Vec<Call>, log: &mut Vec<[i32; 8]>| {
            for c in calls.drain(..) {
                log.push(match c {
                    Call::Redraw(k) => [1, k as i32, 1, 0, 0, 0, 0, 0],
                    Call::Frame(a, b, c, d) => [2, a, b, c, d, 0, 0, 0],
                    Call::Copy(k) => [3, k as i32, 0, 0, 0, 0, 0, 0],
                    Call::ContText(x, y) => [4, x, y, 0, 0, 0, 0, 0],
                });
            }
        };
        let mouse = Mouse { x: tape[0], y: tape[1], buttons: tape[2] };
        let mut calls = Vec::new();
        if !slotmouse::pass_runs((st[6] != 3) as u8) {
            // The handler returned: no call and no byte changes.
        } else if st[6] == 1 {
            sm.on_map(&mut calls);
        } else {
            let mut on_screen = |_: u8| an[0] != 0;
            let mut act = |sm: &mut SlotMouse, k: u8, a: Act, r: &mut [u8], calls: &mut Vec<Call>| {
                let mut log = log_cell.borrow_mut();
                flush(calls, &mut log);
                match a {
                    Act::Popup => {
                        log.push([10, k as i32, 0, 0, 0, 0, 0, 0]);
                        log.push([11, 1, 0, 0, 0, 0, 0, 0]);
                        log.push([13, 0, 0, 0, 0, 0, 0, 0]);
                        log.push([12, 0, 0, 0, 0, 0, 0, 0]);
                        log.push([13, 0, 0, 0, 0, 0, 0, 0]);
                    }
                    Act::Cont => {
                        sm.cont = slotmouse::NONE;
                        log.push([14, 0, 0, 0, 0, 0, 0, 0]);
                        match slotmouse::cont_plan(r) {
                            ContPlan::Walk { dest, sneak, b177, b196 } => {
                                log.push([17, k as i32, dest, 1, sneak as i32, b177 as i32, b196 as i32, 0]);
                            }
                            ContPlan::Use { here, tile, task } => {
                                log.push([15, k as i32, here, tile, an[3], task as i32, 0, 0]);
                                log.push([16, k as i32, an[1] & 0xFF, 0, 0, 0, 0, 0]);
                                if an[2] != 0 {
                                    log.push([18, k as i32, tile, slotmouse::click_item(task), 0, 0, 0, 0]);
                                }
                            }
                        }
                        log.push([13, 0, 0, 0, 0, 0, 0, 0]);
                    }
                    Act::Select => {
                        if slotmouse::cursor_back_first() {
                            log.push([28, 0, 0, 0, 0, 0, 0, 0]);
                        }
                        log.push([19, k as i32, alt, 1, 0, 0, 0, 0]);
                        log.push([13, 0, 0, 0, 0, 0, 0, 0]);
                    }
                    Act::Switch => {
                        let view = |r: &mut [u8], log: &mut Vec<[i32; 8]>| {
                            let open = slotmouse::view_step(r);
                            log.push([21, r[0x17F] as i32, open as i32, 0, 0, 0, 0, 0]);
                        };
                        log.push([11, 1, 0, 0, 0, 0, 0, 0]);
                        log.push([20, 0, 0, 0, 0, 0, 0, 0]);
                        view(r, &mut log);
                        log.push([22, 0, 0, 0, 0, 0, 0, 0]);
                        let l = slotmouse::switch_limit(k);
                        log.push([23, l.0, l.1, l.2, l.3, 0, 0, 0]);
                        for t in tape.chunks(3).skip(1) {
                            if t[2] & 1 != 0 {
                                if let Some(i) = panel::switch_at(k, t[0], t[1]) {
                                    let e = slotmouse::switch_press(r, k, selected, i);
                                    let (x, y) = panel::slot_origin(k);
                                    log.push([24, e as i32, x, y + 5 + 9 * i as i32, 0, 0, 0, 0]);
                                    log.push([25, 0, 0x2B11, 0x32, 1, 0, 0, 0]);
                                    log.push([3, k as i32, 0, 0, 0, 0, 0, 0]);
                                    log.push([13, 0, 0, 0, 0, 0, 0, 0]);
                                    log.push([25, 1, 0x2B11, 0x32, 1, 0, 0, 0]);
                                    if slotmouse::switch_reserves(i) {
                                        log.push([26, k as i32, 0, 0, 0, 0, 0, 0]);
                                    }
                                    log.push([1, k as i32, 1, 0, 0, 0, 0, 0]);
                                }
                            } else if t[2] & 2 != 0 {
                                log.push([22, 0, 0, 0, 0, 0, 0, 0]);
                                break;
                            }
                        }
                        view(r, &mut log);
                        log.push([23, 0, 0, 319, 199, 0, 0, 0]);
                        log.push([12, 0, 0, 0, 0, 0, 0, 0]);
                    }
                }
            };
            sm.pass(mouse, if st[6] == 2 { 2 } else { 1 }, soldiers, selected, &mut on_screen, &mut calls, &mut act);
        }
        let mut log = log_cell.borrow_mut();
        flush(&mut calls, &mut log);
        st[0] = sm.cont as i32;
        st[1] = sm.hover as i32;
        st[2] = sm.last as i32;
        st[3] = sm.over as i32;
        for (i, c) in log.iter().take(cap as usize).enumerate() {
            out[i * 8..i * 8 + 8].copy_from_slice(c);
        }
        log.len() as i32
    })
}

/// travprompt_diff: TraversePrompt 0x1CBA8's gate, loop and close on a tape of (x, y, buttons, key or -1) polls.
/// state: selected, mercs, hp, button path, over_map, tile, view cols, view rows, view left, view top (in), corner x, y (out),
/// tile, over_map (out). out: 3 values per poll (lit or -1, redraw, done), then the pick or -1 and the ShowMouse flag.
/// Returns the number of polls; 0 when the gate refuses, E_ARG on a null pointer.
#[no_mangle]
pub unsafe extern "C" fn ja1f_trav_prompt(state: *mut i32, tape: *const i32, n: i32, out: *mut i32) -> i32 {
    if state.is_null() || tape.is_null() || out.is_null() || n < 1 {
        return E_ARG;
    }
    let (st, ta, ou) = (state as usize, tape as usize, out as usize);
    guard(move || {
        let st = std::slice::from_raw_parts_mut(st as *mut i32, 14);
        let tape = std::slice::from_raw_parts(ta as *const i32, n as usize * 4);
        let out = std::slice::from_raw_parts_mut(ou as *mut i32, n as usize * 3 + 2);
        if !travprompt::gate(st[0] as u8, st[1] as u8, st[2] as u8) {
            return 0;
        }
        let (x0, y0) = if st[3] != 0 { strips::trav_button_corner() } else { strips::TRAV_KEY_CORNER };
        st[10] = x0;
        st[11] = y0;
        let mut p = travprompt::Prompt::new(x0, y0);
        let mut used = 0;
        for t in tape.chunks(4) {
            let key = (t[3] >= 0).then_some(t[3] as u8);
            let s = p.poll(t[0], t[1], t[2] as u8, key);
            out[used * 3] = s.lit.map_or(-1, |i| i as i32);
            out[used * 3 + 1] = s.redraw as i32;
            out[used * 3 + 2] = s.done as i32;
            used += 1;
            if s.done {
                let (mx, my) = (t[0], t[1]);
                let in_map = mx >= st[8] && mx < st[8] + st[6] * 16 && my >= st[9] && my < st[9] + st[7] * 14;
                let mut c = cursor::CursorState::default();
                c.over_map = st[4] as u8;
                c.tile = st[5];
                let show = travprompt::close(&mut c, s.pick, in_map);
                st[12] = c.tile;
                st[13] = c.over_map as i32;
                out[used * 3] = s.pick.map_or(-1, |i| strips::TRAV_CODES[i]);
                out[used * 3 + 1] = show as i32;
                break;
            }
        }
        used as i32
    })
}

/// travprompt_diff: the popup picture over a 320 x 200 frame, `lit` = -1 for no word.
#[no_mangle]
pub unsafe extern "C" fn ja1f_trav_picture(iso_path: *const c_char, exe_path: *const c_char, x0: i32, y0: i32, lit: i32, frame: *mut u8) -> i32 {
    let (Some(iso), Some(exe)) = (cstr(iso_path), cstr(exe_path)) else { return E_ARG };
    if frame.is_null() {
        return E_ARG;
    }
    let fr = frame as usize;
    guard(move || {
        let (c, d) = match (common(iso), panel::panel_data(iso, exe)) {
            (Ok(c), Ok(d)) => (c, d),
            (Err(e), _) | (_, Err(e)) => return e,
        };
        let buf = std::slice::from_raw_parts_mut(fr as *mut u8, 64000);
        let mut table0 = c.remap[0];
        let mut p = panel::Painter { d: &d, buf, table0: &mut table0, table29: panel::table29(&c) };
        strips::trav_popup(&mut p, x0, y0, (lit >= 0).then_some(lit as usize));
        0
    })
}

/// dayend_diff: the wait loop of DayEndScreen? 0x65BD4 on a tape of 8 ints per poll (x, y, buttons, key byte or -1, second key
/// byte, answer of a question in this poll, mouse x and y after that question). `soldiers` = 8 records of 0x200 B, a copy. out
/// gets text lines: "poll N" and the calls of the loop. The test's fire shifts the records up, as its stub in the original.
/// Returns the bytes written, or E_ARG.
#[no_mangle]
pub unsafe extern "C" fn ja1f_dayend_run(soldiers: *const u8, tape: *const i32, n: i32, subtitles: i32, out: *mut u8, cap: i32) -> i32 {
    if soldiers.is_null() || tape.is_null() || out.is_null() || n < 1 || cap < 1 {
        return E_ARG;
    }
    struct T<'a> {
        s: Vec<u8>,
        tape: &'a [i32],
        i: usize,
        m: (i32, i32, u8),
        key: Option<(u8, u8)>,
        log: String,
        subtitles: bool,
    }
    impl T<'_> {
        fn row(&self) -> &[i32] {
            &self.tape[(self.i - 1) * 8..self.i * 8]
        }
    }
    impl dayend::Host for T<'_> {
        fn poll(&mut self) -> bool {
            if self.i * 8 >= self.tape.len() {
                return false;
            }
            self.log.push_str(&format!("poll {}\n", self.i));
            self.i += 1;
            let t = self.row().to_vec();
            self.m = (t[0], t[1], t[2] as u8);
            self.key = (t[3] >= 0).then_some((t[3] as u8, t[4] as u8));
            true
        }
        fn mouse(&self) -> (i32, i32, u8) {
            self.m
        }
        fn key(&mut self) -> Option<u8> {
            self.key.map(|k| k.0)
        }
        fn key2(&mut self) -> u8 {
            self.key.map_or(0, |k| k.1)
        }
        fn exit_prompt(&mut self) -> bool {
            self.log.push_str("exit prompt\n");
            let yes = self.row()[5] != 0;
            if yes {
                self.log.push_str("exit\n");
            }
            yes
        }
        fn say_again(&mut self) {
            self.log.push_str("again\n");
        }
        fn toggle_text(&mut self) {
            self.log.push_str("toggle\n");
        }
        fn button_sound(&mut self, which: u8) {
            self.log.push_str(&format!("sound {}\n", which));
        }
        fn ok_face(&mut self, b: u8, pressed: bool) {
            if pressed {
                self.log.push_str(&format!("press {}\n", b));
            }
        }
        fn wait_left_up(&mut self) {
            self.m.2 &= !1;
        }
        fn present(&self, k: usize) -> bool {
            self.s[k * 0x200 + 0x182] != 0
        }
        fn tip(&mut self, k: Option<usize>) {
            match k {
                Some(k) => {
                    let x = dayend::face_rect(k, false).0;
                    self.log.push_str(&format!("tip {} {} 20 {} 45\n", k + 1, x + 1, x + 29));
                }
                None => self.log.push_str("hide\n"),
            }
        }
        fn ask_fire(&mut self, k: usize) -> bool {
            let r = &self.s[k * 0x200..(k + 1) * 0x200];
            let nick = &r[0x16E..0x16E + 9];
            let nick = &nick[..nick.iter().position(|&c| c == 0).unwrap_or(9)];
            let text = dayend::fire_text(nick);
            self.log.push_str(&format!("ask {} {} {}\n", String::from_utf8_lossy(&text), r[0x179], String::from_utf8_lossy(nick)));
            let t = self.row().to_vec();
            self.m.0 = t[6];
            self.m.1 = t[7];
            t[5] != 0
        }
        fn subtitles(&self) -> bool {
            self.subtitles
        }
        fn close_question(&mut self) {
            self.log.push_str("close\n");
        }
        fn fire(&mut self, k: usize) {
            let (slot, x, y) = dayend::fire_args(&self.s, k);
            self.log.push_str(&format!("fire {} {} {}\ncompact\n", slot, x, y));
            self.s.copy_within((k + 1) * 0x200..8 * 0x200, k * 0x200);
            self.s[7 * 0x200..].fill(0);
        }
        fn redraw(&mut self) {
            self.log.push_str(&format!("redraw {:?}\n", dayend::slot_marks(&self.s)));
        }
    }
    let (so, ta, ou) = (soldiers as usize, tape as usize, out as usize);
    guard(move || {
        let mut t = T { subtitles: subtitles != 0, s: std::slice::from_raw_parts(so as *const u8, 8 * 0x200).to_vec(),
                        tape: std::slice::from_raw_parts(ta as *const i32, n as usize * 8),
                        i: 0, m: (0, 0, 0), key: None, log: String::new() };
        if dayend::wait(&mut t, 15) == dayend::End::Ok {
            t.log.push_str("morning\n");
            if dayend::wait(&mut t, 16) == dayend::End::Ok {
                t.log.push_str("day end\n");
            }
        }
        let b = t.log.as_bytes();
        if b.len() > cap as usize {
            return E_ARG;
        }
        std::ptr::copy_nonoverlapping(b.as_ptr(), ou as *mut u8, b.len());
        b.len() as i32
    })
}

/// sayagain_diff: `balloon::again_text` on the save `save` with `last_quote` = `quote`, the byte after it = `member`,
/// the nicknames of the profiles `k` and `member` poked in. out = k, the name's length, the name, the text. Returns
/// the bytes written, 0 when k names nobody, or E_ARG / E_FILE.
#[no_mangle]
pub unsafe extern "C" fn ja1f_again_text(iso_path: *const c_char, exe_path: *const c_char, save: *const c_char, quote: *const u8, member: u8,
                                         nick_k: *const c_char, nick_m: *const c_char, text_buf: *const c_char, out: *mut u8, cap: i32) -> i32 {
    let (Some(iso), Some(exe), Some(save), Some(nk), Some(nm), Some(tb)) = (cstr(iso_path), cstr(exe_path), cstr(save), cstr(nick_k), cstr(nick_m), cstr(text_buf)) else { return E_ARG };
    if quote.is_null() || out.is_null() || cap < 1 {
        return E_ARG;
    }
    let (qa, ou) = (quote as usize, out as usize);
    guard(move || {
        let Ok(mut w) = world::World::load(iso, exe, save) else { return E_FILE };
        w.last_quote.copy_from_slice(std::slice::from_raw_parts(qa as *const u8, 6));
        w.quote_member = member;
        w.text_buf.fill(0);
        w.text_buf[..tb.len()].copy_from_slice(tb.as_bytes());
        let mut poke = |p: usize, n: &str| {
            if p < 64 {
                let r = &mut w.profiles[p * 0x5A..p * 0x5A + 10];
                r.fill(0);
                r[..n.len().min(10)].copy_from_slice(&n.as_bytes()[..n.len().min(10)]);
            }
        };
        poke(*(qa as *const u8) as usize, nk);
        poke(member as usize, nm);
        let Some((k, name, text)) = balloon::again_text(iso, exe, &w) else { return 0 };
        let mut v = vec![k, name.len() as u8];
        v.extend(&name);
        v.extend(&text);
        let o = std::slice::from_raw_parts_mut(ou as *mut u8, cap as usize);
        let n = v.len().min(cap as usize);
        o[..n].copy_from_slice(&v[..n]);
        n as i32
    })
}

/// complainquote_diff: host op 289 (hate 0: AimComplainQuote 0x44D28) or 291 (hate 1: HatedCheck? 0x44C10) of speaker profile p
/// about profile m with the nickname `nick`. `state` = the last quote (6) + the member byte, in and out; text_buf = 128 bytes,
/// in and out. out = per box: quote, length, text; then the shared buffer (200) and the cut StrReplace target. Returns the bytes written.
#[no_mangle]
pub unsafe extern "C" fn ja1f_complain_quote(iso_path: *const c_char, exe_path: *const c_char, nick: *const c_char, p: u8, m: u8, q: i32, first: i32,
                                             hate: i32, state: *mut u8, text_buf: *mut u8, out: *mut u8, cap: i32) -> i32 {
    let (Some(iso), Some(exe), Some(nick)) = (cstr(iso_path), cstr(exe_path), cstr(nick)) else { return E_ARG };
    if state.is_null() || text_buf.is_null() || out.is_null() || cap < 1 {
        return E_ARG;
    }
    let (sa, tb, ou) = (state as usize, text_buf as usize, out as usize);
    guard(move || {
        let st = std::slice::from_raw_parts_mut(sa as *mut u8, 7);
        let (lq, mb) = st.split_at_mut(6);
        let lq: &mut [u8; 6] = lq.try_into().unwrap();
        let (op, a) = if hate != 0 { (291, [0, m as i32, 0, 0]) } else { (289, [0, q, m as i32, first]) };
        play::merc_quote(lq, &mut mb[0], |_| p, op, &a);
        if hate == 0 {
            play::encode_name(std::slice::from_raw_parts_mut(tb as *mut u8, 128), nick.as_bytes());
        }
        // The window's path (main.rs merc_say_texts): the library op's arguments, then say_boxes.
        let (_, q, first) = play::say_args(op, &a);
        let boxes = balloon::say_boxes(iso, exe, p, q, nick.as_bytes(), first);
        let mut v = vec![boxes.len() as u8];
        for (q, t) in boxes {
            v.push(q as u8);
            v.extend((t.len() as u16).to_le_bytes());
            v.extend(t);
        }
        let (shared, target) = balloon::quote_buffers();
        v.extend(&shared);
        v.extend(target.iter().take_while(|&&b| b != 0));
        let o = std::slice::from_raw_parts_mut(ou as *mut u8, cap as usize);
        let n = v.len().min(cap as usize);
        o[..n].copy_from_slice(&v[..n]);
        n as i32
    })
}

/// aimquote_diff: `aim::quote_text` of profile p, quote q. `state` = the last quote (6 bytes), in and out; profiles = 64 records;
/// text_buf = 128 bytes, in and out. out = text length (u16), text, shared buffer (200), StrReplace target up to its 0.
#[no_mangle]
pub unsafe extern "C" fn ja1f_aim_quote_text(iso_path: *const c_char, exe_path: *const c_char, profiles: *const u8, state: *mut u8, text_buf: *mut u8,
                                             p: i32, q: i32, out: *mut u8, cap: i32) -> i32 {
    let (Some(iso), Some(exe)) = (cstr(iso_path), cstr(exe_path)) else { return E_ARG };
    if profiles.is_null() || state.is_null() || text_buf.is_null() || out.is_null() || cap < 1 || !(0..64).contains(&p) {
        return E_ARG;
    }
    let (pa, sa, tb, ou) = (profiles as usize, state as usize, text_buf as usize, out as usize);
    guard(move || {
        let quotes = aim::edt(iso, exe, "AIMQUOTE.EDT");
        let lq: &mut [u8; 6] = std::slice::from_raw_parts_mut(sa as *mut u8, 6).try_into().unwrap();
        let prof = std::slice::from_raw_parts(pa as *const u8, 64 * 90);
        let tbuf = std::slice::from_raw_parts_mut(tb as *mut u8, 128);
        let (t, _) = aim::quote_text(&quotes, lq, prof, tbuf, p as usize, q);
        let mut v = (t.len() as u16).to_le_bytes().to_vec();
        v.extend(t);
        let (shared, target) = balloon::quote_buffers();
        v.extend(&shared);
        v.extend(target.iter().take_while(|&&b| b != 0));
        let o = std::slice::from_raw_parts_mut(ou as *mut u8, cap as usize);
        let n = v.len().min(cap as usize);
        o[..n].copy_from_slice(&v[..n]);
        n as i32
    })
}

/// raiseask_diff: the member of the name voice before box quote q of host op 296 (a = reason, g = grudge) / 298 (a =
/// answer) for profile p (`play::say_args` + `play::name_voice`), or -1 for none.
#[no_mangle]
pub extern "C" fn ja1f_raise_name_voice(p: i32, op: i32, a: i32, g: i32, q: i32) -> i32 {
    let (m, _, first) = play::say_args(op, &[0, a, g]);
    let s = play::MercSay { k: 0, x0: 0, p, m, q, first, sub: false };
    play::name_voice(&s, q).map_or(-1, |(_, m)| m as i32)
}

/// raiseask_diff: one box of RaiseSalaryPrompt? 0x448A8. op 296 (a = reason, g = grudge profile or -1, nick = its name) or
/// op 298 (a = answer) of profile p. `state` and text_buf and out as in ja1f_complain_quote.
#[no_mangle]
pub unsafe extern "C" fn ja1f_raise_quote(iso_path: *const c_char, exe_path: *const c_char, nick: *const c_char, p: u8, op: i32, a: i32, g: i32,
                                          state: *mut u8, text_buf: *mut u8, out: *mut u8, cap: i32) -> i32 {
    let (Some(iso), Some(exe), Some(nick)) = (cstr(iso_path), cstr(exe_path), cstr(nick)) else { return E_ARG };
    if state.is_null() || text_buf.is_null() || out.is_null() || cap < 1 || !matches!(op, 296 | 298) {
        return E_ARG;
    }
    let (sa, tb, ou) = (state as usize, text_buf as usize, out as usize);
    guard(move || {
        let st = std::slice::from_raw_parts_mut(sa as *mut u8, 7);
        let (lq, mb) = st.split_at_mut(6);
        let lq: &mut [u8; 6] = lq.try_into().unwrap();
        let args = [0, a, g];
        if op == 296 && g >= 0 {
            // The library's order: op 295 raise_voice (k, g), then op 296.
            play::merc_quote(lq, &mut mb[0], |_| p, 295, &[0, g]);
        }
        play::merc_quote(lq, &mut mb[0], |_| p, op, &args);
        let (_, q, first) = play::say_args(op, &args);
        if op == 296 {
            play::encode_name(std::slice::from_raw_parts_mut(tb as *mut u8, 128), nick.as_bytes());
        }
        let boxes = balloon::say_boxes(iso, exe, p, q, nick.as_bytes(), first);
        let mut v = vec![boxes.len() as u8];
        for (q, t) in boxes {
            v.push(q as u8);
            v.extend((t.len() as u16).to_le_bytes());
            v.extend(t);
        }
        let (shared, target) = balloon::quote_buffers();
        v.extend(&shared);
        v.extend(target.iter().take_while(|&&b| b != 0));
        let o = std::slice::from_raw_parts_mut(ou as *mut u8, cap as usize);
        let n = v.len().min(cap as usize);
        o[..n].copy_from_slice(&v[..n]);
        n as i32
    })
}

/// playloop_diff: runs `playloop_run`; out gets its text lines. Returns the bytes written, or E_ARG / E_FILE.
#[no_mangle]
pub unsafe extern "C" fn ja1f_playloop_run(iso_path: *const c_char, exe_path: *const c_char, save: *const c_char, dir: *const c_char,
                                           start: i32, office: *const i32, n_office: i32, tact: *const i32, n_tact: i32,
                                           out: *mut u8, cap: i32) -> i32 {
    let (Some(iso), Some(exe), Some(save), Some(dir)) = (cstr(iso_path), cstr(exe_path), cstr(save), cstr(dir)) else { return E_ARG };
    if out.is_null() || cap < 1 || (n_office > 0 && office.is_null()) || (n_tact > 0 && tact.is_null()) {
        return E_ARG;
    }
    let (oa, ta, ou) = (office as usize, tact as usize, out as usize);
    guard(move || {
        let of: Vec<i32> = if n_office > 0 { std::slice::from_raw_parts(oa as *const i32, n_office as usize).to_vec() } else { Vec::new() };
        let tt: Vec<i32> = if n_tact > 0 { std::slice::from_raw_parts(ta as *const i32, n_tact as usize * 4).to_vec() } else { Vec::new() };
        let text = playloop_run(iso, exe, save, dir, start != 0, of, tt);
        let o = std::slice::from_raw_parts_mut(ou as *mut u8, cap as usize);
        let k = text.len().min(cap as usize);
        o[..k].copy_from_slice(&text.as_bytes()[..k]);
        k as i32
    })
}

/// playloop_diff: the day cycle of PlayLoop 0x4FB94 on the port's real steps (`day_steps`, `day_enter`, `abort_day`,
/// `continue_day`, `game_end`). `office` = the results of OfficeScreen (4 Sleep, 7 Leave; no entry left = 7); `tact` =
/// the results of TacticalLoop as (kind 0 normal / 1 restart / 2 abort, end code, day-end result, mark); no entry left =
/// a normal end. Day-end result 2, 3 or 4 sets the state that makes the real day's verdict stop the game with that code.
fn playloop_run(iso: &str, exe: &str, save: &str, dir: &str, start: bool, office: Vec<i32>, tact: Vec<i32>) -> String {
    use std::cell::RefCell;
    use std::rc::Rc;
    let dirp = std::path::PathBuf::from(dir);
    let mark_at = 59 * 0x5A + 0x59;
    let new_play = |seed: u32| {
        let mut p = play::Play::new(seed);
        p.files = Some((iso.to_string(), exe.to_string()));
        p.day.save_dir = Some(dirp.clone());
        p.day.trace = Some(Vec::new());
        p.host.day_phase = false;
        p
    };
    let mut w = match world::World::load(iso, exe, save) {
        Ok(w) => w,
        Err(e) => return format!("load error {}\n", e),
    };
    let mut p = new_play(1);
    let fin = |p: &mut play::Play, end: &str, m: Option<String>| {
        let mut t = p.day.trace.take().unwrap_or_default().join("\n");
        t.push('\n');
        if let Some(m) = m {
            t.push_str(&format!("error {}\n", m));
        }
        t.push_str(end);
        t.push('\n');
        t
    };
    if start {
        // The save folder as the Continue prompt finds it: the two day files and QUICK.SAV of the same day. Marks 1
        // (the office file), 2 (the day file), 3 (the quick save) tell them apart.
        w.profiles[mark_at] = 1;
        let _ = std::fs::write(dirp.join("OFFICE.SAV"), w.save_bytes());
        w.profiles[mark_at] = 2;
        p.day.trace = None;
        p.day_enter(&mut w, iso, exe).ok();
        w.profiles[mark_at] = 3;
        if let Err(m) = p.quick_save(&mut w) {
            return format!("quick save error {}\n", m);
        }
        let mut q = new_play(1);
        if let Err(m) = q.continue_day(&mut w, iso, exe) {
            return fin(&mut q, "stop", Some(m));
        }
        p = q;
    } else {
        // PlayLoop with EAX 0 starts at the office.
        p.left = true;
        p.day.from_office = true;
        let _ = std::fs::write(dirp.join("QUICK.SAV"), b"old");
    }
    let t0 = match common(iso) {
        Ok(c) => c.remap[0],
        Err(e) => return format!("common error {}\n", e),
    };
    p.host.eod = Some(eod::EodHost::new(iso, exe, t0, None));
    p.host.sunrise = Some(sunrise::Sunrise::new(iso, exe, t0));
    let of = Rc::new(RefCell::new(std::collections::VecDeque::from(office)));
    let of2 = of.clone();
    p.host.office_win = Some(Box::new(move |_w: &mut world::World, _s: &mut u32| -> i32 {
        if of2.borrow_mut().pop_front().unwrap_or(7) == 7 { 2 } else { 0 }
    }));
    let mut ops = tact.chunks(4).map(|c| (c[0], c[1], c[2], c[3])).collect::<std::collections::VecDeque<_>>();
    let mut tactical = start;
    loop {
        if !tactical {
            p.day_steps(&mut w);
            if p.day.game_over.is_some() {
                if let Err(m) = p.game_end(&mut w, iso, exe) {
                    return fin(&mut p, "stop", Some(m));
                }
                if p.day.exit_text.is_some() {
                    return fin(&mut p, "exit", None);
                }
                return fin(&mut p, "return", None);
            }
            if p.day.to_menu {
                return fin(&mut p, "return", None);
            }
            p.left = false;
            p.day.map_open = false;
            if let Err(m) = p.day_enter(&mut w, iso, exe) {
                return fin(&mut p, "stop", Some(m));
            }
        }
        tactical = false;
        // TacticalLoop returned: one result after the other until the day ended or was aborted.
        loop {
            let (kind, code, dayend, mark) = ops.pop_front().unwrap_or((0, 0, 0, 0));
            w.profiles[mark_at] = mark as u8;
            match kind {
                1 => {
                    if let Err(m) = p.abort_day(&mut w, true) {
                        return fin(&mut p, "stop", Some(m));
                    }
                }
                2 => {
                    if let Err(m) = p.abort_day(&mut w, false) {
                        return fin(&mut p, "stop", Some(m));
                    }
                    break;
                }
                _ => {
                    w.update.leave_reason = code as _;
                    match dayend {
                        // day_verdict (gate_endings): day 60 with 4 bad days in a row before it: 2.
                        2 => {
                            w.day = 60;
                            w.accounts[0x9B] = 4;
                        }
                        // day_verdict: the tree blight runs and no owned sector has trees: 3.
                        3 => {
                            w.story[0xA] = 1;
                            for n in 0..60 {
                                w.sector_state[9 * n + 6] = 0;
                            }
                        }
                        // day_verdict: the home sector is not owned: 4.
                        4 => w.sector_state[9 * 0x3B] &= !1,
                        _ => {}
                    }
                    p.left = true;
                    break;
                }
            }
        }
    }
}

/// menu_diff: the main menu, Set Difficulty and Set Quick-Save Mode (GameMain 0x4FFEC, NewGameSetup 0x4F548,
/// SetQuickSaveMode 0x4F898) on a tape of (x, y, left, key byte or -1, second key byte or -1) polls. `cont` = the file
/// test of the program start, the first row comes from `cont` (MainMenu(0x9609C)). out gets text lines: "poll N", the draw calls, "choice K", "game D Q", "exit".
/// Returns the bytes written, or E_ARG / E_FILE.
#[no_mangle]
pub unsafe extern "C" fn ja1f_menu_run(iso_path: *const c_char, exe_path: *const c_char, dir: *const c_char, cont: i32, tape: *const i32,
                                       n: i32, out: *mut u8, cap: i32) -> i32 {
    let (Some(iso), Some(exe), Some(dir)) = (cstr(iso_path), cstr(exe_path), cstr(dir)) else { return E_ARG };
    if tape.is_null() || out.is_null() || n < 1 || cap < 1 {
        return E_ARG;
    }
    let (ta, ou) = (tape as usize, out as usize);
    guard(move || {
        let tape = std::slice::from_raw_parts(ta as *const i32, n as usize * 5);
        let mut m = match menu::Menu::new(iso, exe, Some(std::path::PathBuf::from(dir))) {
            Ok(m) => m,
            Err(e) => return e,
        };
        m.cont = cont != 0;
        m.trace = Some(vec![]);
        m.open(cont);
        let mut text = String::new();
        let mut flush = |m: &mut menu::Menu, t: &mut String| {
            for l in m.trace.as_mut().unwrap().drain(..) {
                t.push_str(&l);
                t.push('\n');
            }
        };
        flush(&mut m, &mut text);
        let mut keys = std::collections::VecDeque::new();
        for (i, t) in tape.chunks(5).enumerate() {
            text.push_str(&format!("poll {}\n", i));
            keys.clear();
            if t[3] >= 0 {
                keys.push_back(t[3] as u8);
                if t[3] == 0 {
                    keys.push_back(t[4] as u8);
                }
            }
            let mut o = m.poll(t[0], t[1], t[2] & 1 != 0, &mut keys);
            // WaitLeftUp in the same poll: the button comes up at the same point.
            if m.waiting() {
                let mut none = std::collections::VecDeque::new();
                let (x, y) = m.warp.unwrap_or((t[0], t[1]));
                let warp = m.warp;
                o = m.poll(x, y, false, &mut none);
                m.warp = m.warp.or(warp);
            }
            flush(&mut m, &mut text);
            match o {
                Some(menu::Outcome::NewGame(d, q)) => {
                    text.push_str(&format!("game {} {}\n", d, q));
                    m.reopen();
                }
                Some(menu::Outcome::Continue) => {
                    text.push_str("game\n");
                    m.reopen();
                }
                Some(menu::Outcome::Exit) => {
                    text.push_str("exit\n");
                    break;
                }
                None => {}
            }
            flush(&mut m, &mut text);
        }
        let b = text.as_bytes();
        let k = b.len().min(cap as usize);
        std::slice::from_raw_parts_mut(ou as *mut u8, k).copy_from_slice(&b[..k]);
        k as i32
    })
}

/// allmove_diff: the loop of AllMoveTraverse? 0x1CFEC (drag.rs) on a tape.
/// state (in): tile, press x, press y, cursor value, cursor tile, combat, locked, shift, alt, confirm, move result;
/// (out, index 11..): value, cursor tile, end code (0 refused, 1 traverse, 2 no edge, 3 walk), direction, all,
/// marker (kind, tile, blink, life; 0 when none), MoveMercTo marker, sneak, b177, alt, squad.
/// tape rows of 5: x, y, buttons, left after WaitRightUp, edge answer. soldier: a 512-byte record (press_fields runs on it).
/// out: rows of 4 (kind 1 edge dir; 2 redraw value x y, then 5 tile; 3 refuse; 4 wait right; 6 value at a poll).
/// Returns the number of log rows; polls used are written to state[26].
#[no_mangle]
pub unsafe extern "C" fn ja1f_drag_run(state: *mut i32, tape: *const i32, n: i32, soldier: *mut u8, out: *mut i32, cap: i32) -> i32 {
    if state.is_null() || tape.is_null() || soldier.is_null() || out.is_null() || n < 1 || cap < 1 {
        return E_ARG;
    }
    let (st, ta, so, ou) = (state as usize, tape as usize, soldier as usize, out as usize);
    guard(move || {
        let st = std::slice::from_raw_parts_mut(st as *mut i32, 27);
        let tape = std::slice::from_raw_parts(ta as *const i32, n as usize * 5);
        let rec = std::slice::from_raw_parts_mut(so as *mut u8, 512);
        let out = std::slice::from_raw_parts_mut(ou as *mut i32, cap as usize * 4);
        struct H<'a> {
            out: &'a mut [i32],
            used: usize,
            edge: bool,
            tile: i32,
        }
        impl H<'_> {
            fn row(&mut self, r: [i32; 4]) {
                let i = self.used * 4;
                if i + 4 <= self.out.len() {
                    self.out[i..i + 4].copy_from_slice(&r);
                    self.used += 1;
                }
            }
        }
        impl drag::Host for H<'_> {
            fn edge(&mut self, dir: u8) -> bool {
                self.row([1, dir as i32, 0, 0]);
                self.edge
            }
            fn redraw(&mut self, value: u8, x: i32, y: i32) {
                self.row([2, value as i32, x, y]);
                self.row([5, self.tile, 0, 0]);
            }
            fn refuse(&mut self) {
                self.row([3, 0, 0, 0]);
            }
        }
        drag::press_fields(rec);
        let mut d = drag::Drag::new(st[0], (st[1], st[2]), st[3] as u8, st[4]);
        let mut h = H { out, used: 0, edge: false, tile: st[0] };
        let (combat, locked) = (st[5] != 0, st[6] != 0);
        let (mut end, mut polls) = (None, 0);
        'tape: for t in tape.chunks(5) {
            polls += 1;
            h.edge = t[4] != 0;
            h.row([6, d.value as i32, 0, 0]);
            if let Some(e) = d.poll(t[0], t[1], t[2] as u8, combat, locked, &mut h) {
                end = Some(e);
                break;
            }
            if d.waiting() {
                h.row([4, 0, 0, 0]);
                if let Some(e) = d.poll(t[0], t[1], t[3] as u8 & 1, combat, locked, &mut h) {
                    end = Some(e);
                    break 'tape;
                }
            }
        }
        st[26] = polls;
        st[11] = d.value as i32;
        st[12] = d.cursor_tile;
        let (code, dir, all) = match end {
            Some(drag::End::Refused) => (0, 0, 0),
            Some(drag::End::Traverse(v)) => (1, v as i32, 0),
            Some(drag::End::NoEdge) => (2, 0, 0),
            Some(drag::End::Walk { all }) => (3, 0, all as i32),
            None => (-1, 0, 0),
        };
        st[13] = code;
        st[14] = dir;
        st[15] = all;
        if code == 3 {
            let mut c = cursor::CursorState::default();
            let a = drag::move_args(st[7], st[8], all);
            if drag::all_marker(all != 0, st[10]) {
                cursor::marker_set(&mut c, 0x0F, st[0]);
                st[16..20].copy_from_slice(&[c.marker_kind, c.marker_tile, c.marker_blink, c.marker_life]);
            }
            st[20..25].copy_from_slice(&[a.0, a.1, a.2, a.3, a.4]);
        }
        h.used as i32
    })
}

/// confirm_diff: `confirm::run` on `screen` (320 x 200, also the screen at the end). `rows` = wait, yes, no (-1 = none);
/// `iv` = number for the row texts (-1 = none), office, restore flag, mouse flag, mouse x, mouse y, answer. `due` and
/// `stop` = the answers of the frame timer and of KeyOrClick, `n` bytes each (past the end: 1). `shots` gets the box
/// at the input, then each picture shown (64000 bytes each, at most `cap`). `log`: rows of 3 (1 sound slot loop;
/// 2 warp x y; 3 ask; 4 show; 5 stop sounds; 6 due; 7 poll). Returns the row count, or below 0; iv[6] = the answer.
#[no_mangle]
#[allow(clippy::too_many_arguments)]
pub unsafe extern "C" fn ja1f_confirm(iso: *const c_char, exe: *const c_char, arc: *const c_char, text: *const c_char,
                                      rows: *const i32, iv: *mut i32, due: *const u8, stop: *const u8, n: i32,
                                      screen: *mut u8, shots: *mut u8, cap: i32, log: *mut i32, nlog: i32) -> i32 {
    let (Some(iso), Some(exe), Some(arc)) = (cstr(iso), cstr(exe), cstr(arc)) else { return E_ARG };
    if text.is_null() || rows.is_null() || iv.is_null() || due.is_null() || stop.is_null() || screen.is_null()
        || shots.is_null() || log.is_null() || n < 0 || cap < 1 || nlog < 1 {
        return E_ARG;
    }
    let text = std::ffi::CStr::from_ptr(text).to_bytes().to_vec();
    let (ro, ivp, du, st, sc, sh, lo) = (rows as usize, iv as usize, due as usize, stop as usize, screen as usize,
                                         shots as usize, log as usize);
    guard(move || {
        struct H<'a> { due: &'a [u8], stop: &'a [u8], kd: usize, ks: usize, shots: &'a mut [u8], ns: usize, log: &'a mut [i32],
                       nl: usize, answer: bool }
        impl H<'_> {
            fn row(&mut self, r: [i32; 3]) {
                if self.nl + 3 <= self.log.len() {
                    self.log[self.nl..self.nl + 3].copy_from_slice(&r);
                    self.nl += 3;
                }
            }
            fn shot(&mut self, img: &[u8]) {
                if (self.ns + 1) * 64000 <= self.shots.len() {
                    self.shots[self.ns * 64000..(self.ns + 1) * 64000].copy_from_slice(img);
                    self.ns += 1;
                }
            }
        }
        impl confirm::Host for H<'_> {
            fn sound(&mut self, slot: i32, sound_loop: i32) { self.row([1, slot, sound_loop]); }
            fn warp(&mut self, at: (i32, i32)) { self.row([2, at.0, at.1]); }
            fn ask(&mut self, imgs: &[Vec<u8>], _wait: Option<&sanim::Anim>) -> bool {
                self.row([3, 0, 0]);
                self.shot(&imgs[0]);
                self.answer
            }
            fn due(&mut self, _ms: u32) -> bool {
                let v = self.due.get(self.kd).copied().unwrap_or(1) != 0;
                self.kd += 1;
                self.row([6, v as i32, 0]);
                v
            }
            fn show(&mut self, img: &[u8]) {
                self.row([4, 0, 0]);
                self.shot(img);
            }
            fn stopped(&mut self) -> bool {
                let v = self.stop.get(self.ks).copied().unwrap_or(1) != 0;
                self.ks += 1;
                self.row([7, v as i32, 0]);
                v
            }
            fn stop_sounds(&mut self) { self.row([5, 0, 0]); }
        }
        let n = n as usize;
        let rows = std::slice::from_raw_parts(ro as *const i32, 3);
        let iv = std::slice::from_raw_parts_mut(ivp as *mut i32, 7);
        let screen = std::slice::from_raw_parts_mut(sc as *mut u8, 64000);
        let number = (iv[0] >= 0).then_some(iv[0]);
        let load = |r: i32| -> Result<Option<sanim::Anim>, i32> {
            // Only the "Destination" rows hold a sector number in their text.
            let number = if matches!(r, 7 | 13 | 15 | 17) { number } else { None };
            if r < 0 { Ok(None) } else { sanim::load(iso, exe, arc, r, number).map(Some) }
        };
        let (wait, yes, no) = match (load(rows[0]), load(rows[1]), load(rows[2])) {
            (Ok(a), Ok(b), Ok(c)) => (a, b, c),
            (Err(e), _, _) | (_, Err(e), _) | (_, _, Err(e)) => return e,
        };
        let c = confirm::Confirm {
            iso: iso.to_string(), exe: exe.to_string(), text, panel: if iv[1] != 0 { "PANEL_B.PCC" } else { "PANEL_P.PCC" },
            wait, yes, no, flags: confirm::Flags { restore: iv[2] != 0, mouse: iv[3] != 0 },
            under: screen.to_vec(), mouse: (iv[4], iv[5]),
        };
        let mut h = H {
            due: std::slice::from_raw_parts(du as *const u8, n), stop: std::slice::from_raw_parts(st as *const u8, n),
            kd: 0, ks: 0, shots: std::slice::from_raw_parts_mut(sh as *mut u8, cap as usize * 64000), ns: 0,
            log: std::slice::from_raw_parts_mut(lo as *mut i32, nlog as usize * 3), nl: 0, answer: iv[6] != 0,
        };
        match confirm::run(&c, &mut h) {
            Ok((yes, end)) => {
                iv[6] = yes as i32;
                screen.copy_from_slice(&end);
                (h.nl / 3) as i32
            }
            Err(e) => e,
        }
    })
}

/// confirm_diff: the key table of the wait 0x2538C for key byte `c`: 1 yes, 0 no, -1 no answer.
#[no_mangle]
pub extern "C" fn ja1f_question_key(c: u8) -> i32 {
    question::key_answer(c).map_or(-1, |y| y as i32)
}

/// flipin_diff: AssignPanelFlipIn? 0x47B68. `screen`: 64000 bytes (read), `panel`: 64000 bytes (written), `frames`: `cap`
/// x 64000 bytes, `ms`: `cap` i32, `sfx`: 4 `cap` i32 (slot -1 = none), `ptr`: `cap` i32 (the pointer of each step: -1
/// hidden, else the shape). Returns the step count.
///
/// # Safety
/// The paths are C strings; the buffers hold the sizes above.
#[no_mangle]
#[allow(clippy::too_many_arguments)]
pub unsafe extern "C" fn ja1f_flipin(iso: *const c_char, exe: *const c_char, screen: *const u8, panel: *mut u8, acc: *const u8,
                                     acc_len: usize, frames: *mut u8, ms: *mut i32, sfx: *mut i32, cap: usize, ptr: *mut i32) -> i32 {
    let (Some(iso), Some(exe)) = (cstr(iso), cstr(exe)) else { return E_ARG };
    if screen.is_null() || panel.is_null() || acc.is_null() || frames.is_null() || ms.is_null() || sfx.is_null() || ptr.is_null() {
        return E_ARG;
    }
    let (sc, pa, ac, fr, mi, sf, pt) = (screen as usize, panel as usize, acc as usize, frames as usize, ms as usize, sfx as usize,
                                        ptr as usize);
    guard(move || {
        let mut map = mapui::MapUi::new(iso, exe, 1);
        let screen = std::slice::from_raw_parts(sc as *const u8, 64000);
        let panel = std::slice::from_raw_parts_mut(pa as *mut u8, 64000);
        let acc = std::slice::from_raw_parts(ac as *const u8, acc_len);
        let steps = match map.flipin_test(screen, panel, acc) {
            Ok(s) => s,
            Err(e) => return e,
        };
        if steps.len() > cap {
            return E_ARG;
        }
        for (k, (img, m, s, p)) in steps.iter().enumerate() {
            std::slice::from_raw_parts_mut((fr + k * 64000) as *mut u8, 64000).copy_from_slice(img);
            *(mi as *mut i32).add(k) = *m as i32;
            *(pt as *mut i32).add(k) = *p;
            let (a, b, c, d) = s.unwrap_or((-1, 0, 0, 0));
            for (o, v) in [a, b, c, d].into_iter().enumerate() {
                *(sf as *mut i32).add(k * 4 + o) = v;
            }
        }
        steps.len() as i32
    })
}

/// flipin_diff: a plain PlayFlipAnim 0x51264 of the port, `MapUi::flip_test`. `screen`: 64000 bytes (read), `frames`:
/// `cap` x 64000 bytes, `ms`: `cap` i32, `sfx`: 4 `cap` i32 (slot -1 = none). Returns the step count.
///
/// # Safety
/// The paths are C strings; the buffers hold the sizes above.
#[no_mangle]
#[allow(clippy::too_many_arguments)]
pub unsafe extern "C" fn ja1f_flip_steps(iso: *const c_char, exe: *const c_char, screen: *const u8, kind: i32, frames: *mut u8,
                                         ms: *mut i32, sfx: *mut i32, cap: usize) -> i32 {
    let (Some(iso), Some(exe)) = (cstr(iso), cstr(exe)) else { return E_ARG };
    if screen.is_null() || frames.is_null() || ms.is_null() || sfx.is_null() {
        return E_ARG;
    }
    let (sc, fr, mi, sf) = (screen as usize, frames as usize, ms as usize, sfx as usize);
    guard(move || {
        let mut map = mapui::MapUi::new(iso, exe, 1);
        let screen = std::slice::from_raw_parts(sc as *const u8, 64000);
        let steps = match map.flip_test(screen, kind) {
            Ok(s) => s,
            Err(e) => return e,
        };
        if steps.len() > cap {
            return E_ARG;
        }
        for (k, (img, m, s)) in steps.iter().enumerate() {
            std::slice::from_raw_parts_mut((fr + k * 64000) as *mut u8, 64000).copy_from_slice(img);
            *(mi as *mut i32).add(k) = *m as i32;
            let (a, b, c, d) = s.unwrap_or((-1, 0, 0, 0));
            for (o, v) in [a, b, c, d].into_iter().enumerate() {
                *(sf as *mut i32).add(k * 4 + o) = v;
            }
        }
        steps.len() as i32
    })
}

/// aimshow_diff: ShowAimScreen 0x1FEB0. `profiles`: 60 records, `soldiers`: 8 records, `accounts`: the accounts block,
/// `view`: 0 bio, 1 gear; `screen`: 64000 bytes, `pal`: 768 bytes. Returns the mode, with P in bits 8 up.
///
/// # Safety
/// The paths are C strings; the buffers hold the sizes above.
#[no_mangle]
pub unsafe extern "C" fn ja1f_aim_show(iso: *const c_char, exe: *const c_char, profiles: *const u8, soldiers: *const u8,
                                       accounts: *const u8, acc_len: usize, view: i32, screen: *mut u8, pal: *mut u8) -> i32 {
    let (Some(iso), Some(exe)) = (cstr(iso), cstr(exe)) else { return E_ARG };
    if profiles.is_null() || soldiers.is_null() || accounts.is_null() || screen.is_null() || pal.is_null() {
        return E_ARG;
    }
    let (pr, so, ac, sc, pa) = (profiles as usize, soldiers as usize, accounts as usize, screen as usize, pal as usize);
    guard(move || {
        let Ok(mut w) = world::World::new_game(iso, exe, 1, 0) else { return E_FILE };
        w.profiles[..60 * 0x5A].copy_from_slice(std::slice::from_raw_parts(pr as *const u8, 60 * 0x5A));
        w.soldiers[..8 * aim::SOLDIER].copy_from_slice(std::slice::from_raw_parts(so as *const u8, 8 * aim::SOLDIER));
        let n = w.accounts.len().min(acc_len);
        w.accounts[..n].copy_from_slice(std::slice::from_raw_parts(ac as *const u8, n));
        let screen = std::slice::from_raw_parts_mut(sc as *mut u8, 64000);
        let pal = std::slice::from_raw_parts_mut(pa as *mut u8, 768);
        match aim::show_test(iso, exe, &mut w, view as u8, screen, pal) {
            Ok(r) => r,
            Err(e) => e,
        }
    })
}

/// guardpic_diff: the first picture of the guard screen (`MapUi::guard_show`): `sectors` 540 bytes, `marks` the 60 old guard counts, sector `cur`, `screen`
/// 64000 bytes and `pal` 768 bytes out. Returns 0, 1 when the screen does not open, or below 0.
///
/// # Safety
/// `iso`, `exe`: NUL-terminated paths; the buffers have the sizes above.
#[no_mangle]
pub unsafe extern "C" fn ja1f_guard_picture(iso: *const c_char, exe: *const c_char, sectors: *const u8, marks: *const i8, cur: i32,
                                            screen: *mut u8, pal: *mut u8, elapsed: *const i32, passes: i32) -> i32 {
    let (Some(iso), Some(exe)) = (cstr(iso), cstr(exe)) else { return E_ARG };
    if sectors.is_null() || marks.is_null() || screen.is_null() || pal.is_null() {
        return E_ARG;
    }
    let (se, mk, sc, pa, el) = (sectors as usize, marks as usize, screen as usize, pal as usize, elapsed as usize);
    guard(move || {
        let Ok(mut w) = world::World::new_game(iso, exe, 1, 0) else { return E_FILE };
        w.sector_state[..540].copy_from_slice(std::slice::from_raw_parts(se as *const u8, 540));
        w.cur_sector = cur as u8;
        let mut old = [0i8; 60];
        old.copy_from_slice(std::slice::from_raw_parts(mk as *const i8, 60));
        island::set_guard_marks(old);
        let mut map = mapui::MapUi::new(iso, exe, 1);
        match map.guard_open(&mut w) {
            Ok(true) => {}
            Ok(false) => return 1,
            Err(e) => return e,
        }
        // guardpic_diff: the loop passes with these times on the screen timer (ms).
        for k in 0..passes.max(0) as usize {
            if let Err(e) = map.guard_pass_at(&mut w, *(el as *const i32).add(k) as i64) {
                return e;
            }
        }
        std::slice::from_raw_parts_mut(sc as *mut u8, 64000).copy_from_slice(&map.img);
        std::slice::from_raw_parts_mut(pa as *mut u8, 768).copy_from_slice(&map.pal);
        0
    })
}

/// guardpic_diff clicks: as `ja1f_guard_picture` with no rotation (one pass first), then the script of `n` x 4 i32 (x, y, buttons,
/// ctrl) per pass. A step with a button up before it is a press; a step with no button is a hover pass. Returns 0, 1 when the
/// screen does not open, 2 when a box came up, or below 0.
///
/// # Safety
/// As `ja1f_guard_picture`; `script` holds `n` x 4 i32.
#[no_mangle]
pub unsafe extern "C" fn ja1f_guard_clicks(iso: *const c_char, exe: *const c_char, sectors: *const u8, marks: *const i8, cur: i32,
                                           screen: *mut u8, script: *const i32, n: i32) -> i32 {
    let (Some(iso), Some(exe)) = (cstr(iso), cstr(exe)) else { return E_ARG };
    if sectors.is_null() || marks.is_null() || screen.is_null() || script.is_null() {
        return E_ARG;
    }
    let (se, mk, sc, sp) = (sectors as usize, marks as usize, screen as usize, script as usize);
    guard(move || {
        let Ok(mut w) = world::World::new_game(iso, exe, 1, 0) else { return E_FILE };
        w.sector_state[..540].copy_from_slice(std::slice::from_raw_parts(se as *const u8, 540));
        w.cur_sector = cur as u8;
        let mut old = [0i8; 60];
        old.copy_from_slice(std::slice::from_raw_parts(mk as *const i8, 60));
        island::set_guard_marks(old);
        let mut map = mapui::MapUi::new(iso, exe, 1);
        match map.guard_open(&mut w) {
            Ok(true) => {}
            Ok(false) => return 1,
            Err(e) => return e,
        }
        let mut was = 0;
        for k in 0..n.max(0) as usize {
            let s = std::slice::from_raw_parts((sp as *const i32).add(4 * k), 4);
            if let Err(e) = map.track(&mut w, (s[0], s[1])) {
                return e;
            }
            map.keys.1 = s[3] != 0;
            let r = if s[2] != 0 && was == 0 { map.guard_click(&mut w, s[2]) } else { map.guard_pass_at(&mut w, 0) };
            if let Err(e) = r {
                return e;
            }
            was = s[2];
            if map.box_shown() {
                return 2;
            }
        }
        std::slice::from_raw_parts_mut(sc as *mut u8, 64000).copy_from_slice(&map.img);
        0
    })
}

/// aimpage_diff: the A.I.M. page of profile `pi` (`aim::page_test`) on `screen`. Returns the mode byte or below 0.
#[no_mangle]
pub unsafe extern "C" fn ja1f_aim_page(iso: *const c_char, exe: *const c_char, profiles: *const u8, soldiers: *const u8,
                                       pi: i32, view: i32, screen: *mut u8) -> i32 {
    let (Some(iso), Some(exe)) = (cstr(iso), cstr(exe)) else { return E_ARG };
    if profiles.is_null() || soldiers.is_null() || screen.is_null() || !(0..60).contains(&pi) {
        return E_ARG;
    }
    let (pr, so, sc) = (profiles as usize, soldiers as usize, screen as usize);
    guard(move || {
        let profiles = std::slice::from_raw_parts(pr as *const u8, 60 * 0x5A);
        let soldiers = std::slice::from_raw_parts(so as *const u8, 8 * aim::SOLDIER);
        let screen = std::slice::from_raw_parts_mut(sc as *mut u8, 64000);
        match aim::page_test(iso, exe, profiles, soldiers, pi as usize, view as u8, screen) {
            Ok(m) => m as i32,
            Err(e) => e,
        }
    })
}

/// aimface_diff: `aim::face_click_test`. `profiles` (60 × 0x5A) and `accounts` (0xA8) are read and written back;
/// `out` gets the script lines (newline-separated, NUL-terminated, `out_len` bytes). Returns the page mode or below 0.
#[no_mangle]
pub unsafe extern "C" fn ja1f_aim_face_click(iso: *const c_char, exe: *const c_char, profiles: *mut u8, soldiers: *const u8,
                                             accounts: *mut u8, mode: i32, sub: i32, out: *mut u8, out_len: i32) -> i32 {
    let (Some(iso), Some(exe)) = (cstr(iso), cstr(exe)) else { return E_ARG };
    if profiles.is_null() || soldiers.is_null() || accounts.is_null() || out.is_null() || out_len < 1 {
        return E_ARG;
    }
    let (pr, so, ac, o) = (profiles as usize, soldiers as usize, accounts as usize, out as usize);
    guard(move || {
        let profiles = std::slice::from_raw_parts_mut(pr as *mut u8, 60 * 0x5A);
        let soldiers = std::slice::from_raw_parts(so as *const u8, 8 * aim::SOLDIER);
        let accounts = std::slice::from_raw_parts_mut(ac as *mut u8, 0xA8);
        let out = std::slice::from_raw_parts_mut(o as *mut u8, out_len as usize);
        match aim::face_click_test(iso, exe, profiles, soldiers, accounts, mode as u8, sub as u8) {
            Ok((m, lines)) => {
                let t = lines.join("\n");
                let n = t.len().min(out.len() - 1);
                out[..n].copy_from_slice(&t.as_bytes()[..n]);
                out[n] = 0;
                m as i32
            }
            Err(e) => e,
        }
    })
}

/// funds_diff: `aim::count_test`. `boxes` may be null: no pictures.
#[no_mangle]
pub unsafe extern "C" fn ja1f_aim_count(iso: *const c_char, exe: *const c_char, balance: i32, amount: i32, max: i32,
                                        values: *mut i32, waits: *mut u8, boxes: *mut u8) -> i32 {
    let (Some(iso), Some(exe)) = (cstr(iso), cstr(exe)) else { return E_ARG };
    if values.is_null() || waits.is_null() || max < 0 {
        return E_ARG;
    }
    let (va, wa, bo) = (values as usize, waits as usize, boxes as usize);
    guard(move || {
        let n = max as usize;
        let boxes: &mut [u8] = if bo == 0 { &mut [] } else { std::slice::from_raw_parts_mut(bo as *mut u8, n.max(1) * aim::FUNDS_PART) };
        match aim::count_test(iso, exe, balance, amount, std::slice::from_raw_parts_mut(va as *mut i32, n),
                              std::slice::from_raw_parts_mut(wa as *mut u8, n), boxes) {
            Ok(k) => k as i32,
            Err(e) => e,
        }
    })
}

/// LoadOffsetBank 0x52E00 (open_tiles = 0: file + max) or OpenTilesDat 0x36EA0 (open_tiles = 1: file only). len < 0 = no
/// file. On 0 `out` gets the buffer as the original leaves it for a buffer at `base`, without its last 4 bytes.
#[no_mangle]
pub unsafe extern "C" fn ja1f_offset_bank(file: *const u8, len: i32, max: i32, open_tiles: i32, base: u32, out: *mut u8, cap: i32) -> i32 {
    if out.is_null() || (len >= 0 && file.is_null()) {
        return E_ARG;
    }
    let f = (len >= 0).then(|| std::slice::from_raw_parts(file, len as usize));
    let got = if open_tiles != 0 { offsetbank::open_tiles(f) } else { offsetbank::load(f, max) };
    let b = match got {
        Ok(b) => b,
        Err(r) => return r,
    };
    if b.image.len() > cap as usize {
        return E_ARG;
    }
    let o = std::slice::from_raw_parts_mut(out, b.image.len());
    o.copy_from_slice(&b.image);
    for (t, &e) in o.chunks_exact_mut(4).zip(&b.entries) {
        t.copy_from_slice(&base.wrapping_add(e as u32).to_le_bytes());
    }
    0
}

/// LoadPanels 0x46EC4: the panel sheet of PanelData::load. Returns its size, or an error.
#[no_mangle]
pub unsafe extern "C" fn ja1f_panel_sheet(iso_path: *const c_char, exe_path: *const c_char, out: *mut u8, cap: i32) -> i32 {
    if iso_path.is_null() || exe_path.is_null() || out.is_null() {
        return E_ARG;
    }
    let (Ok(iso), Ok(exe)) = (CStr::from_ptr(iso_path).to_str(), CStr::from_ptr(exe_path).to_str()) else { return E_ARG };
    let d = match panel::panel_data(iso, exe) {
        Ok(d) => d,
        Err(r) => return r,
    };
    if d.sheet.len() > cap as usize {
        return E_ARG;
    }
    std::slice::from_raw_parts_mut(out, d.sheet.len()).copy_from_slice(&d.sheet);
    d.sheet.len() as i32
}

/// LoadInvDat 0x38190: the INV.DAT buffer of invdraw::inv_data for a buffer at `base` (pointers as the original writes
/// them). `same` = 1 when a second call gives the same data without a new file read (the once flag 0x949F0).
#[no_mangle]
pub unsafe extern "C" fn ja1f_inv_image(iso_path: *const c_char, exe_path: *const c_char, base: u32, out: *mut u8, cap: i32,
                                        same: *mut i32) -> i32 {
    if iso_path.is_null() || exe_path.is_null() || out.is_null() || same.is_null() {
        return E_ARG;
    }
    let (Ok(iso), Ok(exe)) = (CStr::from_ptr(iso_path).to_str(), CStr::from_ptr(exe_path).to_str()) else { return E_ARG };
    let d = match invdraw::inv_data(iso, exe) {
        Ok(d) => d,
        Err(r) => return r,
    };
    let Ok(again) = invdraw::inv_data(iso, exe) else { return E_FILE };
    *same = std::sync::Arc::ptr_eq(&d, &again) as i32;
    if d.inv.len() > cap as usize {
        return E_ARG;
    }
    let o = std::slice::from_raw_parts_mut(out, d.inv.len());
    o.copy_from_slice(&d.inv);
    for (t, &e) in o.chunks_exact_mut(4).zip(&d.inv_off) {
        t.copy_from_slice(&base.wrapping_add(e as u32).to_le_bytes());
    }
    d.inv.len() as i32
}

/// ArcSeek 0x534A4 on the archive bytes: writes offset and size. Returns 0, or -2 when no member has that name.
/// # Safety
/// `arc` holds `len` bytes, `member` is a C string, `out` holds 2 values.
#[no_mangle]
pub unsafe extern "C" fn ja1f_find_member(arc: *const u8, len: i32, member: *const c_char, out: *mut i32) -> i32 {
    if arc.is_null() || member.is_null() || out.is_null() || len < 0 {
        return E_ARG;
    }
    let Ok(name) = CStr::from_ptr(member).to_str() else { return E_ARG };
    match sab::find_member(std::slice::from_raw_parts(arc, len as usize), name) {
        Some((o, s)) => {
            *out = o as i32;
            *out.add(1) = s as i32;
            0
        }
        None => -2,
    }
}

/// 0x5C16C on a Sound with or without an old sample. len < 0 = no open archive. `stopped` = 1 when the old sample ended.
/// Returns the member's size (bytes in `out`), or -1 when nothing is found.
#[no_mangle]
pub unsafe extern "C" fn ja1f_speech_seek(arc: *const u8, len: i32, member: *const c_char, busy: i32, stopped: *mut i32, out: *mut u8,
                                          cap: i32) -> i32 {
    if member.is_null() || stopped.is_null() || out.is_null() || (len >= 0 && arc.is_null()) {
        return E_ARG;
    }
    let Ok(name) = CStr::from_ptr(member).to_str() else { return E_ARG };
    let mut s = sound::Sound::default();
    if busy != 0 {
        s.speech_raw("old", 22050, 70, &[1, 2, 3]);
    }
    let a = (len >= 0).then(|| std::slice::from_raw_parts(arc, len as usize));
    let got = s.speech_seek(a, name);
    *stopped = (busy != 0 && !s.speech_busy()) as i32;
    match got {
        Some(d) if d.len() <= cap as usize => {
            std::slice::from_raw_parts_mut(out, d.len()).copy_from_slice(&d);
            d.len() as i32
        }
        Some(_) => E_ARG,
        None => -1,
    }
}

/// OpenInvExtra 0x25CA4: the archive name as folder + "\\" + file. Returns the length.
#[no_mangle]
pub unsafe extern "C" fn ja1f_inv_extra_name(out: *mut u8, cap: i32) -> i32 {
    let n = format!("{}\\{}", sound::INV_EXTRA.0, sound::INV_EXTRA.1);
    if out.is_null() || n.len() as i32 + 1 > cap {
        return E_ARG;
    }
    let o = std::slice::from_raw_parts_mut(out, n.len() + 1);
    o[..n.len()].copy_from_slice(n.as_bytes());
    o[n.len()] = 0;
    n.len() as i32
}

/// 0x50908 (balloon::talk_box): out = up to 9 screens. `wait` = the wait in ms. Returns the screen count.
#[no_mangle]
pub unsafe extern "C" fn ja1f_talk_box(iso: *const c_char, exe: *const c_char, x0: i32, y0: i32, p: i32, nick: *const c_char, sub: i32,
                                       text: *const c_char, screen: *const u8, out: *mut u8, wait: *mut u32) -> i32 {
    let (Some(iso), Some(exe)) = (cstr(iso), cstr(exe)) else { return E_ARG };
    if nick.is_null() || text.is_null() || screen.is_null() || out.is_null() || wait.is_null() {
        return E_ARG;
    }
    let (nick, text) = (CStr::from_ptr(nick).to_bytes(), CStr::from_ptr(text).to_bytes().to_vec());
    let scr = std::slice::from_raw_parts(screen, 64000);
    match balloon::talk_box(iso, exe, x0, y0, p as u8, nick, sub != 0, text, scr) {
        Ok((v, w)) => {
            for (n, f) in v.iter().enumerate() {
                std::slice::from_raw_parts_mut(out.add(n * 64000), 64000).copy_from_slice(f);
            }
            *wait = w;
            v.len() as i32
        }
        Err(e) => e,
    }
}

/// 0x47AE4 (MapUi::assign_start): saved = the screen kept for the leave; places = assign::ITEM_PLACES as 14 x (u16, u16).
#[no_mangle]
pub unsafe extern "C" fn ja1f_assign_start(iso: *const c_char, exe: *const c_char, screen: *const u8, saved: *mut u8, places: *mut u8) -> i32 {
    let (Some(iso), Some(exe)) = (cstr(iso), cstr(exe)) else { return E_ARG };
    if screen.is_null() || saved.is_null() || places.is_null() {
        return E_ARG;
    }
    let (sc, sa, pl) = (screen as usize, saved as usize, places as usize);
    guard(move || {
        let mut map = mapui::MapUi::new(iso, exe, 1);
        let got = match map.assign_start_test(std::slice::from_raw_parts(sc as *const u8, 64000)) {
            Ok(g) => g,
            Err(e) => return e,
        };
        std::slice::from_raw_parts_mut(sa as *mut u8, 64000).copy_from_slice(&got);
        let o = std::slice::from_raw_parts_mut(pl as *mut u8, 56);
        for (c, &(x, y)) in o.chunks_exact_mut(4).zip(assign::ITEM_PLACES.iter()) {
            c[..2].copy_from_slice(&(x as u16).to_le_bytes());
            c[2..].copy_from_slice(&(y as u16).to_le_bytes());
        }
        0
    })
}

/// 0x424BC (mapui::guard::guard_cell) on `screen`.
#[no_mangle]
pub unsafe extern "C" fn ja1f_guard_cell(iso: *const c_char, exe: *const c_char, n: i32, x0: i32, y0: i32, screen: *mut u8) -> i32 {
    let (Some(iso), Some(exe)) = (cstr(iso), cstr(exe)) else { return E_ARG };
    if screen.is_null() {
        return E_ARG;
    }
    let sc = screen as usize;
    guard(move || {
        let (c, d) = match (common(iso), panel::panel_data(iso, exe)) {
            (Ok(c), Ok(d)) => (c, d),
            _ => return E_FILE,
        };
        let buf = std::slice::from_raw_parts_mut(sc as *mut u8, 64000);
        let mut table0 = c.remap[0];
        let mut p = panel::Painter { d: &d, buf, table0: &mut table0, table29: panel::table29(&c) };
        mapui::guard::guard_cell(&mut p, n, x0, y0);
        0
    })
}

/// OpenTilesDat 0x36EA0: the file name of a tile set. Writes a NUL-ended name, returns its length.
#[no_mangle]
pub unsafe extern "C" fn ja1f_tiles_name(tileset: u8, out: *mut u8, cap: i32) -> i32 {
    let n = offsetbank::tiles_name(tileset);
    if out.is_null() || n.len() as i32 + 1 > cap {
        return E_ARG;
    }
    let o = std::slice::from_raw_parts_mut(out, n.len() + 1);
    o[..n.len()].copy_from_slice(n.as_bytes());
    o[n.len()] = 0;
    n.len() as i32
}

/// MainMenu 0x4FDF4: the screen after Menu::open(row): out = 64000 pixels, pal = 768 VGA bytes (the palette of the last
/// fade), first = 768 bytes of the first fade, warp = mouse x, y.
#[no_mangle]
pub unsafe extern "C" fn ja1f_main_menu(iso_path: *const c_char, exe_path: *const c_char, row: i32, out: *mut u8, pal: *mut u8, first: *mut u8,
                                        warp: *mut i32) -> i32 {
    let (Some(iso), Some(exe)) = (cstr(iso_path), cstr(exe_path)) else { return E_ARG };
    if out.is_null() || pal.is_null() || first.is_null() || warp.is_null() {
        return E_ARG;
    }
    let (ou, pa, fi, wa) = (out as usize, pal as usize, first as usize, warp as usize);
    guard(move || {
        let mut m = match menu::Menu::new(iso, exe, None) {
            Ok(m) => m,
            Err(e) => return e,
        };
        m.fades.clear();
        m.open(row);
        std::slice::from_raw_parts_mut(ou as *mut u8, 64000).copy_from_slice(&m.buf);
        std::slice::from_raw_parts_mut(pa as *mut u8, 768).copy_from_slice(&m.pal);
        std::slice::from_raw_parts_mut(fi as *mut u8, 768).copy_from_slice(&m.fades[0].1);
        let (x, y) = m.warp.unwrap_or((-1, -1));
        *(wa as *mut i32) = x;
        *(wa as *mut i32).add(1) = y;
        0
    })
}

/// The top strip buttons as the port does them, for stripbuttons_diff: `last` = 0: 0x10398 (buttons up); else 0x10454 (shade
/// 0x13..=last). `table0` = remap table 0 (256), panel and screen = 64000 bytes. Returns 0 or an error.
#[no_mangle]
pub unsafe extern "C" fn ja1f_strip_buttons(iso: *const c_char, exe: *const c_char, table0: *mut u8, panel: *mut u8, screen: *mut u8, last: i32,
                                            show: i32, redraw_off: i32) -> i32 {
    let (Some(iso), Some(exe)) = (cstr(iso), cstr(exe)) else { return E_ARG };
    if table0.is_null() || panel.is_null() || screen.is_null() {
        return E_ARG;
    }
    let (ta, pa, sc) = (table0 as usize, panel as usize, screen as usize);
    guard(move || {
        let (c, d) = match (common(iso), panel::panel_data(iso, exe)) {
            (Ok(c), Ok(d)) => (c, d),
            (Err(e), _) | (_, Err(e)) => return e,
        };
        let table0 = &mut *(ta as *mut [u8; 256]);
        let buf = std::slice::from_raw_parts_mut(pa as *mut u8, 64000);
        let mut p = panel::Painter { d: &d, buf, table0, table29: panel::table29(&c) };
        if last == 0 {
            strips::buttons_up(&mut p);
        } else {
            strips::buttons_shade(&mut p, last);
        }
        let scr = std::slice::from_raw_parts_mut(sc as *mut u8, 64000);
        strips::buttons_copy(p.buf, scr, show != 0, redraw_off as u8);
        0
    })
}

/// The A.I.M. quote box 0x28FB8 as the port does it, for aimtalk_diff. `out` = 5 numbers: boxed, x, y, ms, hold; `name` = 8 bytes, the 0-ended face member.
#[no_mangle]
pub unsafe extern "C" fn ja1f_aim_talk(sub: i32, profile: i32, len: i32, out: *mut i32, name: *mut u8) -> i32 {
    if out.is_null() || name.is_null() || len < 0 {
        return E_ARG;
    }
    let (o, na) = (out as usize, name as usize);
    guard(move || {
        let p = aim::talk_plan(sub != 0, profile as u8, len as usize);
        let v = [p.boxed as i32, p.point.0, p.point.1, p.text_ms as i32, p.hold as i32];
        std::slice::from_raw_parts_mut(o as *mut i32, 5).copy_from_slice(&v);
        let n = std::slice::from_raw_parts_mut(na as *mut u8, 8);
        n.fill(0);
        n[..p.face.len().min(7)].copy_from_slice(&p.face.as_bytes()[..p.face.len().min(7)]);
        0
    })
}

/// ContinueDayPrompt 0x4FF00 as the port does it, for continue_diff. `tape` = the answers (0 or 1), `dir` = the save
/// folder. Out: pal = 768 palette bytes, boxes = 64000 pixels per question asked (at most `max_boxes`), log = the lines
/// `fade N FROM TO`, `flush`, `show`, `ask`, `hide`, `delete` (one 0-ended text). Returns 0 or 1 (the answer) or an error.
#[no_mangle]
pub unsafe extern "C" fn ja1f_continue_prompt(iso: *const c_char, exe: *const c_char, dir: *const c_char, tape: *const i32, n: i32, pal: *mut u8,
                                              boxes: *mut u8, max_boxes: i32, log: *mut u8, log_len: i32) -> i32 {
    let (Some(iso), Some(exe), Some(dir)) = (cstr(iso), cstr(exe), cstr(dir)) else { return E_ARG };
    if tape.is_null() || pal.is_null() || boxes.is_null() || log.is_null() || n < 0 || max_boxes < 0 || log_len <= 0 {
        return E_ARG;
    }
    let (ta, pa, bo, lo) = (tape as usize, pal as usize, boxes as usize, log as usize);
    guard(move || {
        struct Io<'a> {
            iso: &'a str,
            exe: &'a str,
            dir: &'a std::path::Path,
            tape: &'a [i32],
            at: usize,
            boxes: &'a mut [u8],
            lines: Vec<String>,
            err: i32,
        }
        impl quicksave::ContinueIo for Io<'_> {
            fn fade(&mut self, _pal: &[u8; 768], n: i32, from: i32, to: i32) {
                self.lines.push(format!("fade {} {} {}", n, from, to));
            }
            fn flush(&mut self) {
                self.lines.push("flush".into());
            }
            fn show_pointer(&mut self) {
                self.lines.push("show".into());
            }
            fn ask(&mut self, q: (&[u8], &str)) -> bool {
                self.lines.push("ask".into());
                match quicksave::continue_box(self.iso, self.exe, q) {
                    Ok(imgs) => {
                        if let Some(o) = self.boxes.chunks_mut(64000).nth(self.at) {
                            o.copy_from_slice(&imgs[0]);
                        }
                    }
                    Err(e) => self.err = e,
                }
                self.at += 1;
                // Past the end of the tape: a yes, so that a port that asks too often still ends.
                self.tape.get(self.at - 1).is_none_or(|&a| a != 0)
            }
            fn hide_pointer(&mut self) {
                self.lines.push("hide".into());
            }
            fn delete_day_files(&mut self) {
                self.lines.push("delete".into());
                quicksave::delete_day_files(self.dir);
            }
        }
        let p = match quicksave::start_palette(iso, exe) {
            Ok(p) => p,
            Err(e) => return e,
        };
        std::slice::from_raw_parts_mut(pa as *mut u8, 768).copy_from_slice(&p);
        let mut io = Io {
            iso,
            exe,
            dir: std::path::Path::new(dir),
            tape: std::slice::from_raw_parts(ta as *const i32, n as usize),
            at: 0,
            boxes: std::slice::from_raw_parts_mut(bo as *mut u8, max_boxes as usize * 64000),
            lines: Vec::new(),
            err: 0,
        };
        let yes = quicksave::continue_prompt(&p, &mut io);
        if io.err != 0 {
            return io.err;
        }
        let text = io.lines.join("\n");
        let o = std::slice::from_raw_parts_mut(lo as *mut u8, log_len as usize);
        let k = text.len().min(o.len() - 1);
        o[..k].copy_from_slice(&text.as_bytes()[..k]);
        o[k] = 0;
        yes as i32
    })
}

/// ExitToDosPrompt 0x38084 as the port does it, for exitdos_diff: the box over `noise` (64000 pixels) into `out`; when
/// `answer` is yes, question::exit_finish. `line` gets the 0-ended quit line, `seed` is the random seed in and out. Bit 0
/// of the result = the prompt ended the program, bit 1 = day files went, bit 2 = Quick Save ran.
#[no_mangle]
pub unsafe extern "C" fn ja1f_exit_prompt(iso: *const c_char, exe: *const c_char, dir: *const c_char, tactical: i32, answer: i32, seed: *mut u32,
                                          noise: *const u8, out: *mut u8, line: *mut u8, line_len: i32) -> i32 {
    let (Some(iso), Some(exe), Some(dir)) = (cstr(iso), cstr(exe), cstr(dir)) else { return E_ARG };
    if seed.is_null() || noise.is_null() || out.is_null() || line.is_null() || line_len <= 0 {
        return E_ARG;
    }
    let (se, no, ou, li) = (seed as usize, noise as usize, out as usize, line as usize);
    guard(move || {
        let imgs = match question::screens(iso, exe, question::EXIT.0, Some(question::EXIT.1), std::slice::from_raw_parts(no as *const u8, 64000)) {
            Ok(i) => i,
            Err(e) => return e,
        };
        std::slice::from_raw_parts_mut(ou as *mut u8, 64000).copy_from_slice(&imgs[0]);
        if answer == 0 {
            return 0;
        }
        let mut rng = host::Rng::new(*(se as *const u32));
        let mut quick = false;
        let (deleted, l) = question::exit_finish(tactical != 0, Some(std::path::Path::new(dir)), exe, &mut rng, || quick = true);
        *(se as *mut u32) = rng.seed;
        let o = std::slice::from_raw_parts_mut(li as *mut u8, line_len as usize);
        let k = l.len().min(o.len() - 1);
        o[..k].copy_from_slice(&l[..k]);
        o[k] = 0;
        1 | (deleted as i32) << 1 | (quick as i32) << 2
    })
}

/// TeamQuote? 0x61688 as the port does it, for teamquote_diff: `recs` = 8 soldier records, `seed` in and out; `out` (8 bytes)
/// gets the slots that speak, in order. Returns the count.
#[no_mangle]
pub unsafe extern "C" fn ja1f_team_quote(recs: *const u8, range: i32, seed: *mut u32, out: *mut u8) -> i32 {
    if recs.is_null() || seed.is_null() || out.is_null() {
        return E_ARG;
    }
    let (re, se, ou) = (recs as usize, seed as usize, out as usize);
    guard(move || {
        let mut rng = host::Rng::new(*(se as *const u32));
        let mut n = 0;
        sighting::team_quote(std::slice::from_raw_parts(re as *const u8, 8 * 512), range, |m| rng.random(m), |k| {
            *(ou as *mut u8).add(n) = k;
            n += 1;
        });
        *(se as *mut u32) = rng.seed;
        n as i32
    })
}

/// InitOfficeScreen 0x4ED1C as the port does it: Office::new on the world of `save` with the link byte and the day.
/// out = 64000 pixels, pal = 768 VGA bytes, music = the wanted theme. Returns 0 or an error.
#[no_mangle]
pub unsafe extern "C" fn ja1f_office_init(iso: *const c_char, exe: *const c_char, save: *const c_char, link: i32, day: i32, out: *mut u8,
                                          pal: *mut u8, music: *mut i32) -> i32 {
    let (Some(iso), Some(exe), Some(save)) = (cstr(iso), cstr(exe), cstr(save)) else { return E_ARG };
    if out.is_null() || pal.is_null() || music.is_null() {
        return E_ARG;
    }
    let (ou, pa, mu) = (out as usize, pal as usize, music as usize);
    guard(move || {
        let base = match world(iso, exe, save) {
            Ok(w) => w,
            Err(e) => return e,
        };
        let mut w = (*base).clone();
        if let Some(b) = w.accounts.get_mut(0xA3) {
            *b = link as u8;
        }
        w.day = day;
        let o = match office::Office::new(iso, exe, &w) {
            Ok(o) => o,
            Err(e) => return e,
        };
        std::slice::from_raw_parts_mut(ou as *mut u8, 64000).copy_from_slice(&o.picture());
        std::slice::from_raw_parts_mut(pa as *mut u8, 768).copy_from_slice(&o.pal);
        *(mu as *mut i32) = music::with(|m| m.wanted());
        0
    })
}

/// WaitConfirmClick? 0x1C514: walks a tape of polls (mouse x, y, buttons, key: 5 i32 with the mode's map view x, y in the
/// window; view x < 0 = off the map) through cursor::confirm_poll and confirm_end. state = result, cursor tile, item
/// timer, polls used. Returns 0.
#[no_mangle]
pub unsafe extern "C" fn ja1f_confirm_click(tile: i32, c0: i32, r0: i32, tape: *const i32, n: i32, timer0: i32, state: *mut i32) -> i32 {
    if tape.is_null() || state.is_null() || n < 1 {
        return E_ARG;
    }
    let tape = std::slice::from_raw_parts(tape, n as usize * 4);
    let st = std::slice::from_raw_parts_mut(state, 4);
    let (mut cur, mut timer) = (tile, timer0);
    let mut res = -1;
    let mut used = n;
    for (i, t) in tape.chunks(4).enumerate() {
        let under = (t[0] >= 0).then(|| viewmode::tactical_tile(c0, r0, t[0], t[1]));
        if let Some(a) = cursor::confirm_poll(tile, under, t[2] & 1 != 0, t[2] & 2 != 0, t[3] == 27) {
            res = cursor::confirm_end(a, tile, &mut cur, &mut timer) as i32;
            used = i as i32 + 1;
            break;
        }
    }
    st.copy_from_slice(&[res, cur, timer, used]);
    0
}

/// ReadSaveSlotHeaders 0x4DB34: saveslots::Headers::read on `dir`. used = 10 bytes, days = 10 u16, names = 260 bytes.
/// Returns the count of files.
#[no_mangle]
pub unsafe extern "C" fn ja1f_slot_headers(dir: *const c_char, used: *mut u8, days: *mut u16, names: *mut u8) -> i32 {
    let Some(dir) = cstr(dir) else { return E_ARG };
    if used.is_null() || days.is_null() || names.is_null() {
        return E_ARG;
    }
    let h = saveslots::Headers::read(std::path::Path::new(dir));
    for i in 0..10 {
        *used.add(i) = h.used[i] as u8;
        *days.add(i) = h.days[i];
        std::ptr::copy_nonoverlapping(h.names[i].as_ptr(), names.add(26 * i), 26);
    }
    h.used.iter().filter(|u| **u).count() as i32
}

/// invextra_diff: PlayInvExtra? 0x25AD8 on a tape of 4 ints per poll (ms since the box showed, button level, key byte or -1,
/// speech busy after this poll's refill). `playing` = the speech channel is busy after the start. out gets text lines: "start
/// MEMBER", "box TEXT", "boxend", "poll MODE", "refill", "queue KEY". Returns the bytes written, or E_ARG.
#[no_mangle]
pub unsafe extern "C" fn ja1f_inv_extra(snd: i32, txt: i32, subtitles: i32, playing: i32, start: i32, tape: *const i32, n: i32,
                                        out: *mut u8, cap: i32) -> i32 {
    if tape.is_null() || out.is_null() || n < 1 || cap < 1 {
        return E_ARG;
    }
    struct T<'a> {
        tape: &'a [i32],
        i: usize,
        ms: i32,
        busy: bool,
        playing: bool,
        log: String,
    }
    impl invextra::Host for T<'_> {
        fn start(&mut self, member: &str, rate: i32, volume: i32) {
            self.log.push_str(&format!("start {} {} {}\n", member, rate, volume));
            self.busy = self.playing;
        }
        fn show_box(&mut self, text: &str) -> Result<(), i32> {
            self.log.push_str(&format!("box {}\n", text));
            Ok(())
        }
        fn box_end(&mut self) {
            self.log.push_str("boxend\n");
        }
        fn poll(&mut self, mode: i32) -> Option<invextra::Poll> {
            let r = self.tape.get(self.i * 4..self.i * 4 + 4)?;
            self.i += 1;
            self.ms = r[0];
            self.log.push_str(&format!("poll {}\n", mode));
            Some(invextra::Poll { buttons: r[1] as u8, keys: if r[2] >= 0 { vec![r[2] as u8] } else { Vec::new() } })
        }
        fn elapsed(&mut self) -> i32 {
            self.ms
        }
        fn speaking(&mut self) -> bool {
            self.busy
        }
        fn refill(&mut self) {
            self.log.push_str("refill\n");
            self.busy = self.tape[(self.i - 1) * 4 + 3] != 0;
        }
        fn queue(&mut self, key: u8) {
            self.log.push_str(&format!("queue {}\n", key));
        }
    }
    let (member, text) = invscreen::extra_names(snd, txt);
    let mut t = T { tape: std::slice::from_raw_parts(tape, n as usize * 4), i: 0, ms: 0, busy: false, playing: playing != 0,
                    log: String::new() };
    let _ = invextra::run(&mut t, member, text, subtitles != 0, start as u8);
    let b = t.log.as_bytes();
    let k = b.len().min(cap as usize);
    std::ptr::copy_nonoverlapping(b.as_ptr(), out, k);
    k as i32
}

/// cursorback_diff: CursorItemBack? 0x53660 on a bound world (screen_drawn, cursor_drop, held, soldiers, selected, strip_box).
/// Returns 1 when it acted.
///
/// # Safety
/// `w`: a world whose `held` and `soldiers` buffers are valid.
#[no_mangle]
pub unsafe extern "C" fn ja1f_cursor_back(w: *mut host::Ja1World) -> i32 {
    play::cursor_back_raw(w) as i32
}

/// ja1f_cursor_back with the host (port fix P22 puts the item on the ground when the merc has no empty place).
///
/// # Safety
/// As ja1f_cursor_back; `h` is null or a valid host.
#[no_mangle]
pub unsafe extern "C" fn ja1f_cursor_back_host(w: *mut host::Ja1World, h: *const host::Ja1Host) -> i32 {
    play::cursor_back_host(w, h) as i32
}

/// guardopen_diff: the steps of FUN_1328C as text, one line each, "screen" between the two lists; `state` = wobble dx, wobble dy,
/// over_map, cursor tile, changed by `guardopen::mouse_off`. Returns the bytes written, or E_ARG.
///
/// # Safety
/// `state`: 4 ints; `out`: `cap` bytes.
#[no_mangle]
pub unsafe extern "C" fn ja1f_guard_steps(state: *mut i32, out: *mut u8, cap: i32) -> i32 {
    if state.is_null() || out.is_null() || cap < 1 {
        return E_ARG;
    }
    let mut st: cursor::CursorState = std::mem::zeroed();
    let s = std::slice::from_raw_parts_mut(state, 4);
    st.wobble_dx = s[0] as _;
    st.wobble_dy = s[1] as _;
    st.over_map = s[2] as _;
    st.tile = s[3] as _;
    guardopen::mouse_off(&mut st);
    s.copy_from_slice(&[st.wobble_dx as i32, st.wobble_dy as i32, st.over_map as i32, st.tile as i32]);
    let mut log = String::new();
    for k in guardopen::ENTER {
        log.push_str(&format!("{:?}\n", k));
    }
    log.push_str("screen\n");
    for k in guardopen::LEAVE {
        log.push_str(&format!("{:?}\n", k));
    }
    let b = log.as_bytes();
    let k = b.len().min(cap as usize);
    std::ptr::copy_nonoverlapping(b.as_ptr(), out, k);
    k as i32
}

/// mapkey_diff: MapKeyHandler? 0x426D0. `keys` = n pairs (ascii, scan); ascii 0 = an extended key. `guard` != 0 = the
/// guard screen (no Alt-X). `subs` = the Subtitles switch, in and out. `out` = 2 ints per key: kind (0 nothing, 1 Enter,
/// 2 message box, 3 Alt-X, 4 key L) and the offset of the box text in `text` (NUL ended, -1 = no text). Enter on the
/// guard screen is not pressed here: its exit test is guard_diff's. Returns 0, or an error code.
#[no_mangle]
pub unsafe extern "C" fn ja1f_map_key(iso: *const c_char, exe: *const c_char, save: *const c_char, on_guard: i32,
                                      keys: *const i32, n: i32, subs: *mut i32, out: *mut i32, text: *mut u8,
                                      cap: i32) -> i32 {
    let (Some(iso), Some(exe), Some(save)) = (cstr(iso), cstr(exe), cstr(save)) else { return E_ARG };
    if keys.is_null() || subs.is_null() || out.is_null() || text.is_null() || n < 0 || cap < 0 {
        return E_ARG;
    }
    let (ke, su, ou, te) = (keys as usize, subs as usize, out as usize, text as usize);
    guard(move || {
        // SAFETY: the caller passes 2 n key ints, 2 n out ints, cap text bytes and one int for the switch.
        let ke = std::slice::from_raw_parts(ke as *const i32, 2 * n as usize);
        let ou = std::slice::from_raw_parts_mut(ou as *mut i32, 2 * n as usize);
        let te = std::slice::from_raw_parts_mut(te as *mut u8, cap as usize);
        let su = su as *mut i32;
        let base = match world(iso, exe, save) {
            Ok(w) => w,
            Err(e) => return e,
        };
        let mut w = (*base).clone();
        ctlpanel::set(&mut w, 5, *su as u8);
        let mut map = mapui::MapUi::new(iso, exe, 1);
        map.day = true;
        if let Err(e) = map.show(&mut w, (0, 200)) {
            return e;
        }
        if on_guard != 0 {
            // QICKSTRT.SAV has no current sector; the screen needs one below 90.
            w.cur_sector = 38;
            match map.guard_show(&mut w) {
                Ok(true) => {}
                Ok(false) => return E_ARG,
                Err(e) => return e,
            }
        }
        let mut off = 0usize;
        for i in 0..n as usize {
            let (a, scan) = (ke[2 * i], ke[2 * i + 1]);
            let before = map.again_keys;
            // The window: Alt-X asks if exit_key_ok; every other extended key gives no letter.
            if a == 0 {
                ou[2 * i] = if scan == 0x2D && map.exit_key_ok() { 3 } else { 0 };
                ou[2 * i + 1] = -1;
                continue;
            }
            let c = (a as u8).to_ascii_uppercase();
            let r = match map.key(&mut w, c) {
                Ok(r) => r,
                Err(e) => return e,
            };
            let mut kind = match r {
                mapui::Key::Start => 1,
                _ => 0,
            };
            if map.again_keys != before {
                kind = 4;
            }
            ou[2 * i + 1] = -1;
            if map.box_shown() && kind == 0 {
                kind = 2;
                let t = map.box_text().unwrap_or("").as_bytes().to_vec();
                if off + t.len() + 1 > te.len() {
                    return E_ARG;
                }
                te[off..off + t.len()].copy_from_slice(&t);
                te[off + t.len()] = 0;
                ou[2 * i + 1] = off as i32;
                off += t.len() + 1;
            }
            if map.box_shown() {
                for b in [mapui::BoxInput::Down(1), mapui::BoxInput::Up(1)] {
                    if let Err(e) = map.box_input(b) {
                        return e;
                    }
                }
            }
            ou[2 * i] = kind;
        }
        *su = ctlpanel::get(&w, 5) as i32;
        0
    })
}

/// sndplan_diff: the tactical sound bank's load list (sound::tactical_plan) as text, one `folder archive member slot`
/// line per sample. `soldiers` = at least 8 records. Returns the length written, or E_ARG.
#[no_mangle]
pub unsafe extern "C" fn ja1f_snd_plan(soldiers: *const u8, len: usize, out: *mut u8, cap: usize) -> i32 {
    if soldiers.is_null() || out.is_null() {
        return E_ARG;
    }
    let mut s = String::new();
    for e in sound::tactical_plan(std::slice::from_raw_parts(soldiers, len)) {
        s.push_str(&format!("{} {} {} {}\n", e.folder, e.archive, e.member, e.slot));
    }
    if s.len() > cap {
        return E_ARG;
    }
    std::ptr::copy_nonoverlapping(s.as_ptr(), out, s.len());
    s.len() as i32
}

/// mappanel_diff: mappanel::draw_tab_switch on `screen` (64000 bytes). Returns 0, or an error code.
#[no_mangle]
pub unsafe extern "C" fn ja1f_map_tab_switch(iso: *const c_char, exe: *const c_char, accounts: *const u8, on_duty: i32,
                                             from: i32, to: i32, screen: *mut u8) -> i32 {
    let (Some(iso), Some(exe)) = (cstr(iso), cstr(exe)) else { return E_ARG };
    if accounts.is_null() || screen.is_null() || from < 0 || to < 0 {
        return E_ARG;
    }
    let (accounts, screen) = (accounts as usize, screen as usize);
    guard(move || {
        let acc = std::slice::from_raw_parts(accounts as *const u8, 0xA8);
        let scr = std::slice::from_raw_parts_mut(screen as *mut u8, 64000);
        match mappanel::draw_tab_switch(iso, exe, acc, on_duty, from as usize, to as usize, scr) {
            Ok(()) => 0,
            Err(e) => e,
        }
    })
}

/// smallrows_diff: LoadQuestRecord 0x614D4; `out` holds 250 bytes.
#[no_mangle]
pub unsafe extern "C" fn ja1f_quest_record(q: *const u8, len: usize, n: i32, out: *mut u8) {
    play::quest_record(Some(std::slice::from_raw_parts(q, len)), n, std::slice::from_raw_parts_mut(out, 250));
}

/// smallrows_diff: RecordReplayEvent 0x3FA04 `n` times on a log of `start` records; `ev` = n x (from, to, code),
/// `out` = 3 n bytes. Returns the count word.
#[no_mangle]
pub unsafe extern "C" fn ja1f_replay_record(start: usize, ev: *const i32, n: usize, out: *mut u8) -> u32 {
    let mut log = vec![[0u8; 3]; start];
    let e = std::slice::from_raw_parts(ev, n * 3);
    for c in e.chunks(3) {
        play::record_replay_event(&mut log, c[0], c[1], c[2]);
    }
    let o = std::slice::from_raw_parts_mut(out, n * 3);
    for (i, r) in log[start..].iter().enumerate() {
        o[i * 3..i * 3 + 3].copy_from_slice(r);
    }
    play::replay_count(&log)
}

/// smallrows_diff: ClearMarksBeforeQuote? 0x11418; `io` = over_map, wobble dx, wobble dy, ap, tile.
#[no_mangle]
pub unsafe extern "C" fn ja1f_clear_marks(io: *mut i32) {
    let v = std::slice::from_raw_parts_mut(io, 5);
    let mut c = cursor::CursorState { over_map: v[0] as u8, wobble_dx: v[1] as _, wobble_dy: v[2] as _, ap: v[3] as _, tile: v[4] as _, ..Default::default() };
    cursor::clear_marks_before_quote(&mut c);
    v[0] = c.over_map as i32;
    v[1] = c.wobble_dx as i32;
    v[2] = c.wobble_dy as i32;
    v[3] = c.ap as i32;
    v[4] = c.tile as i32;
}

/// smallrows_diff: InitTempFiles 0x38038. `files`: 1 = sector file k exists (`n` entries); `replay`: the record count.
#[no_mangle]
pub unsafe extern "C" fn ja1f_init_temp_files(files: *mut u8, n: usize, replay: *mut i32) {
    let f = std::slice::from_raw_parts_mut(files, n);
    let mut tmp: Vec<Option<Vec<u8>>> = f.iter().map(|&b| if b != 0 { Some(Vec::new()) } else { None }).collect();
    let mut log = vec![[0u8; 3]; *replay as usize];
    world::init_temp_files(&mut tmp, &mut log);
    for (i, b) in f.iter_mut().enumerate() {
        *b = tmp.get(i).map_or(0, |t| t.is_some() as u8);
    }
    *replay = log.len() as i32;
}

/// smallrows_diff: ShowAllStatPopups 0x40024. Returns the slot count; `out` 8 bytes; `over` = over_map in and out.
#[no_mangle]
pub unsafe extern "C" fn ja1f_key_s(soldiers: *const u8, over: *mut u8, out: *mut u8) -> i32 {
    let ks = screen::key_s(std::slice::from_raw_parts(soldiers, 8 * REC), &mut *over);
    std::slice::from_raw_parts_mut(out, ks.len()).copy_from_slice(&ks);
    ks.len() as i32
}

/// smallrows_diff: the face step of 0x2884C for one soldier record: -1 = clear, 0 alive, 1 dead.
#[no_mangle]
pub unsafe extern "C" fn ja1f_face_kind(s: *const u8) -> i32 {
    screen::face_kind(std::slice::from_raw_parts(s, REC)).unwrap_or(-1)
}

/// smallrows_diff: 0x46AC4 on a buffer of 1024 bytes.
#[no_mangle]
pub unsafe extern "C" fn ja1f_face_clear(face: *mut u8) {
    screen::clear_face(std::slice::from_raw_parts_mut(face, 1024));
}

/// squadlost_diff: the last shaded button of the next tactical frame.
#[no_mangle]
pub extern "C" fn ja1f_frame_shade(combat: u8, view_hold: u8) -> i32 {
    strips::frame_shade(combat, view_hold)
}

/// stripbuttons_diff: StopWalk 0x104C8; `io` = view_hold, turn_shown. Returns the last shaded element of the frame.
#[no_mangle]
pub unsafe extern "C" fn ja1f_stop_walk(slot: u8, flag: i32, combat: u8, selected: u8, io: *mut u8) -> i32 {
    let (mut v, mut t) = (*io, *io.add(1));
    strips::set_shade_last(0);
    strips::stop_walk(slot, flag, combat, selected, &mut v, &mut t);
    *io = v;
    *io.add(1) = t;
    strips::shade_of(v)
}

/// smallrows_diff: 0x1CB78. Returns -1 for TacticalRedraw(1), else AL for OverviewRedraw(1, AL, 0, 0).
#[no_mangle]
pub extern "C" fn ja1f_redraw_choice(mode: u8, al: u8) -> i32 {
    viewmode::redraw_choice(mode, al).map_or(-1, |a| a as i32)
}

/// smallrows_diff: 0x4E700. `kind` 0 = no slot in use, 1 = cancel, else a chosen slot. Writes the message (NUL ended) into
/// `text` (64 B; empty = no box); returns 1 when LoadGame runs.
#[no_mangle]
pub unsafe extern "C" fn ja1f_restore_steps(kind: i32, text: *mut u8) -> i32 {
    use saveslots::Outcome::*;
    let (m, load) = saveslots::restore_steps(match kind { 0 => NoSaves, 1 => Cancel, _ => Slot(0) });
    let t = std::slice::from_raw_parts_mut(text, 64);
    t.fill(0);
    if let Some(m) = m {
        t[..m.len()].copy_from_slice(m.as_bytes());
    }
    load as i32
}

/// music_file_diff: PlayXmiMusic 0x5C4C8. Writes the short name of the file that Music::load takes to `base` (32 B, NUL
/// ended; empty = none); returns its size.
#[no_mangle]
pub unsafe extern "C" fn ja1f_music_file(exe: *const std::os::raw::c_char, name: *const std::os::raw::c_char, base: *mut u8) -> i32 {
    let exe = std::ffi::CStr::from_ptr(exe).to_string_lossy();
    let name = std::ffi::CStr::from_ptr(name).to_string_lossy();
    let (f, n) = music::Music::probe(&exe, &name);
    let b = f.as_bytes();
    let k = b.len().min(31);
    std::ptr::copy_nonoverlapping(b.as_ptr(), base, k);
    *base.add(k) = 0;
    n as i32
}

/// setmusic_diff: SetMusic 0x5C5EC, StopMusic 0x5C82C. `ops` holds n triples (kind 0 = play theme a, stop first b; 1 = stop;
/// 2 = Music option a). `out` gets 3 ints per op: playing, wanted, the theme that started (-1 none). Returns the sequence count.
#[no_mangle]
pub unsafe extern "C" fn ja1f_music_ops(exe: *const std::os::raw::c_char, ops: *const i32, n: i32, out: *mut i32) -> i32 {
    let exe = std::ffi::CStr::from_ptr(exe).to_string_lossy();
    music::Music::run_ops(&exe, std::slice::from_raw_parts(ops, 3 * n as usize), std::slice::from_raw_parts_mut(out, 3 * n as usize))
}

/// quickrestore_diff: `quicksave::after_restore(theme)` as [over_map, playing theme, started theme or -1].
///
/// # Safety
/// `out` holds 3 ints.
#[no_mangle]
pub unsafe extern "C" fn ja1f_quick_after(theme: i32, out: *mut i32) {
    let (o, p, s) = quicksave::after_restore(theme as i16);
    *out = o as i32;
    *out.add(1) = p as i32;
    *out.add(2) = s.map_or(-1, |t| t as i32);
}

/// buttonat_diff: see `salary::button_hit` (k 0 NO, 1 OK, 2 down, 3 up).
#[no_mangle]
pub extern "C" fn ja1f_salary_button_hit(k: i32, x: i32, y: i32) -> i32 {
    ((0..4).contains(&k) && salary::button_hit(k as usize, x, y)) as i32
}

/// xmifind_diff: see `music::xmi_form`.
///
/// # Safety
/// `d` holds `len` bytes.
#[no_mangle]
pub unsafe extern "C" fn ja1f_xmi_form(d: *const u8, len: i32, n: i32) -> i64 {
    music::xmi_form(std::slice::from_raw_parts(d, len as usize), n)
}

/// musicfade_diff: see `Music::run_fade`. `ops` holds n triples.
///
/// # Safety
/// `exe` and `file` are C strings, `out` holds `cap` * 4 ints.
#[no_mangle]
pub unsafe extern "C" fn ja1f_music_fade(exe: *const std::os::raw::c_char, file: *const std::os::raw::c_char, t: i32, ops: *const i32, n: i32, ticks: i32,
                                         out: *mut i32, cap: i32) -> i32 {
    let (exe, file) = (std::ffi::CStr::from_ptr(exe).to_string_lossy(), std::ffi::CStr::from_ptr(file).to_string_lossy());
    music::Music::run_fade(&exe, &file, t, std::slice::from_raw_parts(ops, 3 * n as usize), ticks, std::slice::from_raw_parts_mut(out, 4 * cap as usize))
}

/// sfx_diff: PlaySfxSlot 0x5CB34, ButtonPressSound 0x5C8A0, ButtonReleaseSound 0x5C900, StopAllSounds 0x5CE08, PlayVocSample 0x5C9C0. See Sound::run_ops.
#[no_mangle]
pub unsafe extern "C" fn ja1f_sfx_ops(ops: *const i32, n: i32, out: *mut i32) {
    sound::Sound::run_ops(std::slice::from_raw_parts(ops, 5 * n as usize), std::slice::from_raw_parts_mut(out, 37 * n as usize));
}

/// voice_diff: the sound loaders on the real files. kind 0 = name_voice(a, b); 1 = op 100 (speaker a, cry b); 2 = op 186 (the death
/// cry); 3 = effect 0x2D + b of the tactical bank, soldier 0 with profile a; 4, 5 = speech sample at rate a, volume b, Sound FX on, off
/// (three LE words: step, volume, loops); 6..9 = effect b of the main, door, 17 overhead, tactical bank; 10 = click sound; 11 = speech stop.
/// Writes the sample bytes to `out`; returns their
/// count, the sample count when `cap` is smaller, or -1 for no sample. kinds 0..2 start with the cry sample [1, 2, 3].
///
/// # Safety
/// `iso` and `exe` are C strings, `out` holds `cap` bytes.
#[no_mangle]
pub unsafe extern "C" fn ja1f_voice(iso: *const std::os::raw::c_char, exe: *const std::os::raw::c_char, kind: i32, a: i32, b: i32,
                                    out: *mut u8, cap: i32) -> i32 {
    let (iso, exe) = (std::ffi::CStr::from_ptr(iso).to_string_lossy(), std::ffi::CStr::from_ptr(exe).to_string_lossy());
    let d = sound::Sound::with_files(&iso, &exe).voice_probe(kind, a, b);
    let Some(d) = d else { return -1 };
    if d.len() <= cap as usize {
        std::ptr::copy_nonoverlapping(d.as_ptr(), out, d.len());
    }
    d.len() as i32
}

/// confirm_diff: question::go_screen, the map's box after yes (row `row` of `arc` over the screen `base`, 64000 bytes), into
/// `out`. Returns 0 or an error code.
///
/// # Safety
/// `iso`, `exe` and `arc` are C strings, `base` and `out` hold 64000 bytes.
#[no_mangle]
pub unsafe extern "C" fn ja1f_go_screen(iso: *const c_char, exe: *const c_char, arc: *const c_char, row: i32, number: i32,
                                        base: *const u8, out: *mut u8) -> i32 {
    let (Some(iso), Some(exe), Some(arc)) = (cstr(iso), cstr(exe), cstr(arc)) else { return E_ARG };
    let base = std::slice::from_raw_parts(base, 64000);
    let go = match sanim::load(iso, exe, arc, row, (number >= 0).then_some(number)) {
        Ok(a) => a,
        Err(c) => return c,
    };
    match question::go_screen(iso, exe, base, &go) {
        Ok(img) => {
            std::ptr::copy_nonoverlapping(img.as_ptr(), out, 64000);
            0
        }
        Err(c) => c,
    }
}

/// itemvoice_diff: the item's voice (panel.md) on the real INV archives, as text `busy|names`: busy = 1 when the speech
/// channel plays after Sound::item_voice. Returns the text length.
///
/// # Safety
/// `iso` and `exe` are C strings, `out` holds `cap` bytes.
#[no_mangle]
pub unsafe extern "C" fn ja1f_item_voice_list(iso: *const std::os::raw::c_char, exe: *const std::os::raw::c_char, profile: i32, t: i32,
                                              count: i32, value: i32, b: i32, out: *mut u8, cap: i32) -> i32 {
    let (iso, exe) = (std::ffi::CStr::from_ptr(iso).to_string_lossy(), std::ffi::CStr::from_ptr(exe).to_string_lossy());
    let Ok(inv) = invdraw::inv_data(&iso, &exe) else { return -1 };
    let i = iteminfo::Info { profile: profile as u8, t: t as u8, count: count as i8, value: value as i16, numbers: false,
                             opt_info: false, opt_sub: false };
    let mut snd = sound::Sound::with_files(&iso, &exe);
    let v = iteminfo::voice(&i, &inv.item_types, b != 0, &|arc, m| snd.inv_has(arc, m));
    let names = snd.item_voice(v.as_deref().unwrap_or(&[]));
    let text = format!("{}|{}", snd.speech_busy() as u8, names.iter().map(|n| n.as_str()).collect::<Vec<_>>().join(" "));
    let n = text.len().min(cap as usize);
    std::ptr::copy_nonoverlapping(text.as_ptr(), out, n);
    n as i32
}

/// ai_diff door_fx: `sound::door_base` (the name stem of the door film) as text; returns its length, 0 for no film.
///
/// # Safety
/// `out` holds `cap` bytes.
#[no_mangle]
pub unsafe extern "C" fn ja1f_door_base(big: i32, r: i32, phase: i32, out: *mut u8, cap: i32) -> i32 {
    let Some(text) = sound::door_base(big, r, phase) else { return 0 };
    let n = text.len().min(cap as usize);
    std::ptr::copy_nonoverlapping(text.as_ptr(), out, n);
    n as i32
}

/// Test hook (waitms_diff): WaitMs? 0x67F48 over a tape of polls, rows of 4 bytes (elapsed low, elapsed high, buttons,
/// arrival count). A row's arrival count takes that many bytes from `keys` into the key queue. `iv`: [ms, start flags,
/// old key byte], then out: [answer, rows used, key byte, flags, keys left]. Returns 0.
#[no_mangle]
pub unsafe extern "C" fn ja1f_waitms(tape: *const u8, n: i32, keys: *const u8, nkeys: i32, iv: *mut i32) -> i32 {
    let tape = std::slice::from_raw_parts(tape, n as usize * 4);
    let keys = std::slice::from_raw_parts(keys, nkeys as usize);
    let iv = std::slice::from_raw_parts_mut(iv, 8);
    let mut w = invextra::WaitMs::new(iv[0], iv[1] as u8, iv[2] as u8);
    let (mut q, mut next, mut pos) = (std::collections::VecDeque::new(), 0usize, 0usize);
    let answer = loop {
        let Some(r) = tape.chunks(4).nth(pos) else { break -1 };
        let elapsed = i32::from(r[0]) | i32::from(r[1]) << 8;
        // The timer comes before the poll: a timed end leaves this row unread.
        if !w.in_release() && iv[0] <= elapsed {
            break w.step(elapsed, 0, None).map_or(-1, i32::from);
        }
        pos += 1;
        for _ in 0..r[3] {
            q.push_back(keys[next]);
            next += 1;
        }
        let key = if w.in_release() {
            None
        } else {
            q.pop_front().map(|k| if k == 0 { q.pop_front().unwrap_or(0) } else { k })
        };
        if let Some(a) = w.step(elapsed, r[2], key) {
            break i32::from(a);
        }
    };
    iv[3] = answer;
    iv[4] = pos as i32;
    iv[5] = i32::from(w.last_key);
    iv[6] = i32::from(w.flags());
    iv[7] = q.len() as i32;
    0
}

/// Test hook (waitms_diff): WaitKeyOrClick 0x67F34 over the tape of `ja1f_waitms` (the elapsed bytes are not read).
/// `iv`: [unused, start flags, old key byte], then out: [1, rows used, key byte, flags, keys left]. Returns 0.
#[no_mangle]
pub unsafe extern "C" fn ja1f_waitkey(tape: *const u8, n: i32, keys: *const u8, nkeys: i32, iv: *mut i32) -> i32 {
    let tape = std::slice::from_raw_parts(tape, n as usize * 4);
    let keys = std::slice::from_raw_parts(keys, nkeys as usize);
    let iv = std::slice::from_raw_parts_mut(iv, 8);
    let mut w = invextra::WaitKey::new(iv[1] as u8, iv[2] as u8);
    let (mut q, mut next, mut pos) = (std::collections::VecDeque::new(), 0usize, 0usize);
    let answer = loop {
        let Some(r) = tape.chunks(4).nth(pos) else { break -1 };
        pos += 1;
        for _ in 0..r[3] {
            q.push_back(keys[next]);
            next += 1;
        }
        let key = if w.in_release() {
            None
        } else {
            q.pop_front().map(|k| if k == 0 { q.pop_front().unwrap_or(0) } else { k })
        };
        if w.step(r[2], key) {
            break 1;
        }
    };
    iv[3] = answer;
    iv[4] = pos as i32;
    iv[5] = i32::from(w.last_key);
    iv[6] = i32::from(w.flags());
    iv[7] = q.len() as i32;
    0
}

/// Test hook (pressbtn_diff): the hold of a pressed button over a tape of queue rows (buttons only: the moves do not
/// count). `out`: [rows used, x the mouse goes to, y]. Returns 0, or -1 when the tape ends first.
#[no_mangle]
pub unsafe extern "C" fn ja1f_button_hold(x: i32, y: i32, tape: *const u8, n: i32, out: *mut i32) -> i32 {
    let tape = std::slice::from_raw_parts(tape, n as usize);
    let out = std::slice::from_raw_parts_mut(out, 3);
    let h = pointer::ButtonHold::press((x, y));
    for (i, b) in tape.iter().enumerate() {
        if let Some((sx, sy)) = h.step(*b) {
            out[0] = i as i32 + 1;
            out[1] = sx;
            out[2] = sy;
            return 0;
        }
    }
    -1
}

/// Test hook (waitms_diff): EscPressed? 0x67300 on a key queue, `calls` times. `out` gets one answer byte per call.
/// Returns the bytes left in the queue.
#[no_mangle]
pub unsafe extern "C" fn ja1f_esc_pressed(keys: *const u8, nkeys: i32, calls: i32, out: *mut u8) -> i32 {
    let mut q: std::collections::VecDeque<u8> = std::slice::from_raw_parts(keys, nkeys as usize).iter().copied().collect();
    for i in 0..calls as usize {
        *out.add(i) = u8::from(invextra::esc_pressed(&mut q));
    }
    q.len() as i32
}

/// DrawText 0x358D0 of the port (`Painter::text_mode`) on `screen` (320 x 200, in/out), with the first remap table of
/// the game. `left` gets the three table bytes 0xFF, 0xFE, 0 after the call (an unknown glyph leaves two of them
/// changed). 0 = done, < 0 = error.
///
/// # Safety
/// `iso`, `exe`, `text`: NUL-terminated; `screen` has 64000 bytes; `left` has 3 bytes.
#[no_mangle]
#[allow(clippy::too_many_arguments)]
pub unsafe extern "C" fn ja1f_draw_text(iso: *const c_char, exe: *const c_char, text: *const c_char, x: i32, y: i32,
                                        centred: i32, fg: i32, shadow: i32, font: i32, mode: i32, screen: *mut u8,
                                        left: *mut u8) -> i32 {
    let (Some(iso), Some(exe)) = (cstr(iso), cstr(exe)) else { return E_ARG };
    if text.is_null() || screen.is_null() || left.is_null() || !(0..3).contains(&font) {
        return E_ARG;
    }
    let text = std::ffi::CStr::from_ptr(text).to_bytes().to_vec();
    let (screen, left) = (screen as usize, left as usize);
    guard(move || {
        let scr = std::slice::from_raw_parts_mut(screen as *mut u8, 64000);
        let out = std::slice::from_raw_parts_mut(left as *mut u8, 3);
        let (c, d) = match (common(iso), panel::panel_data(iso, exe)) {
            (Ok(c), Ok(d)) => (c, d),
            (Err(e), _) | (_, Err(e)) => return e,
        };
        let mut table0 = c.remap[0];
        let mut p = panel::Painter { d: &d, buf: scr, table0: &mut table0, table29: panel::table29(&c) };
        p.text_mode(&text, x, y, centred != 0, fg as u8, shadow as u8, font as usize, mode);
        out.copy_from_slice(&[table0[0xFF], table0[0xFE], table0[0]]);
        0
    })
}

/// The exit branch of the A.I.M. screen as `aim::leave_route` gives it, for aimleave_diff: `soldiers` = 8 records,
/// `answers` = 2 bytes, `out` = up to 8 numbers. The count of numbers, < 0 = error.
///
/// # Safety
/// `soldiers` has 8 records of 0x200 bytes, `answers` 2 bytes, `out` 8 numbers.
#[no_mangle]
pub unsafe extern "C" fn ja1f_aim_leave(soldiers: *const u8, balance: i32, difficulty: i32, answers: *const u8, out: *mut i32) -> i32 {
    if soldiers.is_null() || answers.is_null() || out.is_null() || !(0..3).contains(&difficulty) {
        return E_ARG;
    }
    let (soldiers, answers, out) = (soldiers as usize, answers as usize, out as usize);
    guard(move || {
        let s = std::slice::from_raw_parts(soldiers as *const u8, 8 * 0x200);
        let a = std::slice::from_raw_parts(answers as *const u8, 2);
        let r = aim::leave_route(s, balance, difficulty as u8, &[a[0] != 0, a[1] != 0]);
        std::slice::from_raw_parts_mut(out as *mut i32, r.len()).copy_from_slice(&r);
        r.len() as i32
    })
}

/// Host op 57 of the end of day (eod::EodHost, DrawMercText 0x27B38) on the picture `screen` (64000 bytes): soldier 0
/// is the record `rec` (512 bytes), `n` 3 or 4 picks the text. For retreat line tests. Returns 0 when it drew.
///
/// # Safety
/// The paths are C strings, `rec` holds 512 bytes and `screen` 64000 bytes.
#[no_mangle]
pub unsafe extern "C" fn ja1f_retreat_line(iso: *const c_char, exe: *const c_char, rec: *const u8, n: i32,
                                           screen: *mut u8) -> i32 {
    let (Some(iso), Some(exe)) = (cstr(iso), cstr(exe)) else { return E_ARG };
    let t0 = match common(iso) {
        Ok(c) => c.remap[0],
        Err(e) => return e,
    };
    let mut e = eod::EodHost::new(iso, exe, t0, None);
    e.op(254, &[3], None);
    let s = std::slice::from_raw_parts_mut(screen, 64000);
    e.s.copy_from_slice(s);
    let sol = std::slice::from_raw_parts(rec, 512);
    e.op(57, &[0, n, 1500], Some(sol));
    if e.texts != 1 {
        return E_FILE;
    }
    s.copy_from_slice(&e.s);
    0
}

/// Record `pi` of EDT/BIOQUOTE.EDT as the A.I.M. page reads it (LoadBioQuoteEdt 0x234E0). Returns its length.
///
/// # Safety
/// The paths are C strings, `out` holds 320 bytes.
#[no_mangle]
pub unsafe extern "C" fn ja1f_aim_bio(iso: *const c_char, exe: *const c_char, pi: i32, out: *mut u8) -> i32 {
    let (Some(iso), Some(exe)) = (cstr(iso), cstr(exe)) else { return E_ARG };
    let bio = aim::edt(iso, exe, "BIOQUOTE.EDT");
    let rec = aim::bio_record(&bio, pi as usize);
    if rec.is_empty() {
        return 0;
    }
    std::slice::from_raw_parts_mut(out, 320).copy_from_slice(rec);
    320
}

/// The place of team face `k` on the A.I.M. page (FacePanelX? 0x23214, FacePanelY? 0x23244).
///
/// # Safety
/// `x` and `y` are valid.
#[no_mangle]
pub unsafe extern "C" fn ja1f_face_xy(k: i32, x: *mut i32, y: *mut i32) {
    let (a, b) = aim::face_xy(k as usize);
    *x = a;
    *y = b;
}

/// The text of play::merc_text or play::dead_text (MsgMercText 0x15910). Returns its length, -1 when there is no message.
///
/// # Safety
/// `rec` holds 512 bytes, `out` holds `cap` bytes.
#[no_mangle]
pub unsafe extern "C" fn ja1f_merc_text(rec: *const u8, text: i32, out: *mut u8, cap: i32) -> i32 {
    let rec = std::slice::from_raw_parts(rec, 512);
    // Code 100 = op 186's "NAME is dead!", the same routine in the original.
    let t = if text == 100 { play::dead_text(rec) } else { play::merc_text(rec, text) };
    let Some(t) = t else { return -1 };
    let n = t.len().min(cap as usize);
    std::slice::from_raw_parts_mut(out, n).copy_from_slice(&t.as_bytes()[..n]);
    n as i32
}

/// iso::game_file (OpenCDFile 0x38304): the bytes of `name`. Returns the length, -1 when no source holds it.
///
/// # Safety
/// The strings are C strings, `out` holds `cap` bytes.
#[no_mangle]
pub unsafe extern "C" fn ja1f_game_file(iso: *const c_char, exe: *const c_char, name: *const c_char, out: *mut u8,
                                        cap: i32) -> i32 {
    let (Some(iso), Some(exe), Some(name)) = (cstr(iso), cstr(exe), cstr(name)) else { return E_ARG };
    let Ok(mut i) = iso::Iso::open(iso) else { return E_FILE };
    let Ok(b) = iso::game_file(Some(exe), &mut i, name) else { return -1 };
    let n = b.len().min(cap as usize);
    std::slice::from_raw_parts_mut(out, n).copy_from_slice(&b[..n]);
    b.len() as i32
}

/// The stat-loss message state for tests: the fields of `World` that play::loss_set, loss_pass and the quick save use.
#[repr(C)]
pub struct LossIo {
    pub slot: i32,
    pub amount: i32,
    pub timer: i32,
    pub part: [u8; 16],
    pub stat: [u8; 16],
}

thread_local! {
    static LOSS_WORLD: std::cell::RefCell<Option<world::World>> = const { std::cell::RefCell::new(None) };
}

/// One step of the stat-loss message for lossmsg_diff on `io`, with the soldier records `soldiers` (size::SOLDIERS bytes):
/// mode 0 = play::loss_set (a, b, c); 1 = play::loss_pass; 2 = play::loss_timer_tick.
/// Returns 0, or for mode 1: 1 = a box (text in `out`, its length in `len`, `ms`), 2 = the team quote. Negative = error.
///
/// # Safety
/// The paths are C strings; the pointers are valid for the sizes above, `out` for `cap` bytes.
#[no_mangle]
pub unsafe extern "C" fn ja1f_loss_step(iso: *const c_char, exe: *const c_char, soldiers: *const u8, io: *mut LossIo,
                                        mode: i32, a: i32, b: i32, c: i32, out: *mut u8, cap: i32, len: *mut i32,
                                        ms: *mut i32) -> i32 {
    let (Some(iso), Some(exe)) = (cstr(iso), cstr(exe)) else { return E_ARG };
    LOSS_WORLD.with(|cell| {
        let mut cell = cell.borrow_mut();
        if cell.is_none() {
            match world::World::new_game(iso, exe, 0, 0) {
                Ok(w) => *cell = Some(w),
                Err(_) => return E_ARG,
            }
        }
        let Some(w) = cell.as_mut() else { return E_ARG };
        let n = w.soldiers.len();
        w.soldiers.copy_from_slice(std::slice::from_raw_parts(soldiers, n));
        let s = &mut *io;
        (w.loss_slot, w.accounts[0x8F], w.loss_timer) = (s.slot as u8, s.amount as u8, s.timer);
        (w.loss_part, w.loss_stat) = (s.part, s.stat);
        let mut r = 0;
        match mode {
            0 => play::loss_set(w, a, b, c),
            1 => match play::loss_pass(w) {
                Some(play::LossShow::Box(t, m)) => {
                    let k = t.len().min(cap as usize);
                    std::ptr::copy_nonoverlapping(t.as_ptr(), out, k);
                    *len = k as i32;
                    *ms = m as i32;
                    r = 1;
                }
                Some(play::LossShow::Quote) => r = 2,
                None => {}
            },
            _ => play::loss_timer_tick(&mut w.loss_timer),
        }
        (s.slot, s.amount, s.timer) = (w.loss_slot as i32, w.accounts[0x8F] as i32, w.loss_timer);
        (s.part, s.stat) = (w.loss_part, w.loss_stat);
        r
    })
}

/// world::main_words_text for main_diff: `words` holds `n` C strings one behind the other. Writes up to `cap` bytes of the text
/// to `out` and its length to `len`. Returns 1 when the program ends at an unknown word, else 0.
///
/// # Safety
/// `words` holds `n` C strings, `out` `cap` bytes, `len` is valid.
#[no_mangle]
pub unsafe extern "C" fn ja1f_main_words(words: *const u8, n: i32, out: *mut u8, cap: i32, len: *mut i32) -> i32 {
    let mut p = words as *const c_char;
    let mut list = Vec::new();
    for _ in 0..n {
        let c = CStr::from_ptr(p).to_bytes();
        list.push(c);
        p = p.add(c.len() + 1);
    }
    let (t, end) = world::main_words_text(&list);
    let k = t.len().min(cap as usize);
    std::ptr::copy_nonoverlapping(t.as_ptr(), out, k);
    *len = k as i32;
    end as i32
}

/// InitSound 0x5CED4 + InitAil 0x5C1DC on `cfg` (82 bytes), for sounddrv_diff: out = sound driver, music driver, option 0,
/// option 1, option 5 after World::apply_sound_cfg on a world with options 0, 1 and 5 = `opts`; out[5..7] = the program's start
/// volumes (music, sound). Returns 0, or a negative error.
///
/// # Safety
/// The paths are C strings; `cfg` holds 82 bytes, `opts` 3 and `out` 7 ints.
#[no_mangle]
pub unsafe extern "C" fn ja1f_sound_setup(iso: *const c_char, exe: *const c_char, cfg: *const u8, opts: *const u8, out: *mut i32) -> i32 {
    let (Some(iso), Some(exe)) = (cstr(iso), cstr(exe)) else { return E_ARG };
    TIMER_WORLD.with(|cell| {
        let mut cell = cell.borrow_mut();
        if cell.is_none() {
            match world::World::new_game(iso, exe, 0, 0) {
                Ok(w) => *cell = Some((w, play::PlayHost::new(1))),
                Err(_) => return E_ARG,
            }
        }
        let Some((w, _)) = cell.as_mut() else { return E_ARG };
        for (i, k) in [0usize, 1, 5].iter().enumerate() {
            ctlpanel::set(w, *k, *opts.add(i));
        }
        w.apply_sound_cfg(std::slice::from_raw_parts(cfg, 82));
        let v = sound::volumes();
        let r = [w.sound_setup.sound_driver as i32, w.sound_setup.music_driver as i32, ctlpanel::get(w, 0) as i32,
                 ctlpanel::get(w, 1) as i32, ctlpanel::get(w, 5) as i32, v.music, v.sound];
        std::ptr::copy_nonoverlapping(r.as_ptr(), out, 7);
        0
    })
}

/// The values of the whole timer callback TacticalTimer? 0x5EE98 that have a place in the port (timer_diff).
#[repr(C)]
pub struct TimerIo {
    pub lock2: i32,
    pub lock: i32,
    pub lit: i32,
    pub hover: i32,
    pub delay: i32,
    pub ambient_t1: i32,
    pub ambient_t2: i32,
    pub mark_timer: i32,
    pub pause_blink: i32,
    pub cursor_blink: i32,
    pub small_panel: i32,
    pub panel_ends: i32,
    pub marker_tile: i32,
    pub marker_life: i32,
    pub marker_blink: i32,
    pub show_timer: i32,
    pub tick_step: i32,
    pub shown_slot: i32,
    pub spotted: i32,
    pub view_tile: i32,
    pub loss_timer: i32,
    pub busy_timer: i32,
    pub busy: i32,
    pub fast_timer: i32,
    pub turn_based: i32,
    pub turn_clock: i32,
    pub turn_tick: i32,
    pub ripple_timer: i32,
    pub stop_timer: i32,
    pub screen_drawn: i32,
    pub fuse_beep_timer: i32,
    pub fuse_beep: i32,
    pub anim_count: [i32; 10],
    pub anim_timer: [i32; 10],
    pub anim_tile: [i32; 10],
    pub fx_timer: [i32; 16],
}

thread_local! {
    static TIMER_WORLD: std::cell::RefCell<Option<(world::World, play::PlayHost)>> = const { std::cell::RefCell::new(None) };
}

/// `n` ticks of the port's timer (play::timer_ticks + play::panel_countdown) on the given state, for the test of the
/// whole callback TacticalTimer? 0x5EE98. Returns 0, or a negative error.
///
/// # Safety
/// The paths are C strings; `soldiers`, `tiles`, `fx_anims` and `timed_fx` hold the world's buffer sizes.
#[no_mangle]
pub unsafe extern "C" fn ja1f_timer_step(iso: *const c_char, exe: *const c_char, io: *mut TimerIo, soldiers: *mut u8,
                                         tiles: *mut u8, fx_anims: *mut u8, timed_fx: *mut u8, n: i32) -> i32 {
    let (Some(iso), Some(exe)) = (cstr(iso), cstr(exe)) else { return E_ARG };
    TIMER_WORLD.with(|cell| {
        let mut cell = cell.borrow_mut();
        if cell.is_none() {
            match world::World::new_game(iso, exe, 0, 0) {
                Ok(w) => *cell = Some((w, play::PlayHost::new(1))),
                Err(_) => return E_ARG,
            }
        }
        let Some((w, host)) = cell.as_mut() else { return E_ARG };
        let (sn, tn, an, fn_) = (w.soldiers.len(), w.tiles.len(), w.fx_anims.len(), w.timed_fx.len());
        w.soldiers.copy_from_slice(std::slice::from_raw_parts(soldiers, sn));
        w.tiles.copy_from_slice(std::slice::from_raw_parts(tiles, tn));
        w.fx_anims.copy_from_slice(std::slice::from_raw_parts(fx_anims, an));
        w.timed_fx.copy_from_slice(std::slice::from_raw_parts(timed_fx, fn_));
        let v = &mut *io;
        let u = &mut w.update;
        u.round_lock2 = v.lock2 as u8;
        u.round_lock = v.lock as u8;
        u.ambient_t1 = v.ambient_t1;
        u.ambient_t2 = v.ambient_t2;
        u.show_timer = v.show_timer;
        u.tick_step = v.tick_step;
        u.shown_slot = v.shown_slot as _;
        u.spotted = v.spotted as _;
        u.view_tile = v.view_tile as _;
        u.busy_timer = v.busy_timer;
        w.busy = v.busy;
        u.fast_timer = v.fast_timer;
        w.turn_based = v.turn_based;
        u.turn_clock = v.turn_clock;
        u.turn_tick = v.turn_tick;
        u.ripple_timer = v.ripple_timer;
        u.stop_timer = v.stop_timer;
        u.screen_drawn = v.screen_drawn as _;
        u.fuse_beep_timer = v.fuse_beep_timer;
        u.fuse_beep = v.fuse_beep as _;
        w.loss_timer = v.loss_timer;
        w.small_panel = v.small_panel;
        w.panel_ends = v.panel_ends as u8;
        for k in 0..10 {
            let a = &mut host.anims.tiles[k];
            (a.count, a.timer, a.tile) = (v.anim_count[k], v.anim_timer[k], v.anim_tile[k] as i16);
        }
        host.anims.fx_timers.copy_from_slice(&v.fx_timer);
        let mut pause = pause::Pause { blink_timer: v.pause_blink, ..Default::default() };
        let mut cursor = cursor::CursorState::default();
        cursor.blink_timer = v.cursor_blink;
        cursor.mark_timer = v.mark_timer;
        cursor.marker_tile = v.marker_tile;
        cursor.marker_life = v.marker_life;
        cursor.marker_blink = v.marker_blink;
        let mut scroll = viewmode::Scroll { hover: v.hover, delay: v.delay, ..Default::default() };
        let mut lines = Vec::new();
        {
            let mut s = play::Session::new(w, host);
            play::timer_ticks(&mut s, &mut pause, &mut cursor, &mut scroll, &mut lines, n as u64);
        }
        for _ in 0..n {
            play::panel_countdown(w, v.lit as u8);
        }
        let u = &w.update;
        v.ambient_t1 = u.ambient_t1;
        v.ambient_t2 = u.ambient_t2;
        v.show_timer = u.show_timer;
        v.shown_slot = u.shown_slot as _;
        v.spotted = u.spotted as _;
        v.view_tile = u.view_tile as _;
        v.busy_timer = u.busy_timer;
        v.busy = w.busy;
        v.fast_timer = u.fast_timer;
        v.turn_based = w.turn_based;
        v.turn_clock = u.turn_clock;
        v.turn_tick = u.turn_tick;
        v.ripple_timer = u.ripple_timer;
        v.stop_timer = u.stop_timer;
        v.fuse_beep_timer = u.fuse_beep_timer;
        v.fuse_beep = u.fuse_beep as _;
        v.loss_timer = w.loss_timer;
        v.small_panel = w.small_panel;
        v.panel_ends = w.panel_ends as i32;
        for k in 0..10 {
            let a = &host.anims.tiles[k];
            (v.anim_count[k], v.anim_timer[k], v.anim_tile[k]) = (a.count, a.timer, a.tile as i32);
        }
        v.fx_timer.copy_from_slice(&host.anims.fx_timers);
        v.pause_blink = pause.blink_timer;
        v.cursor_blink = cursor.blink_timer;
        v.mark_timer = cursor.mark_timer;
        v.marker_tile = cursor.marker_tile;
        v.marker_life = cursor.marker_life;
        v.marker_blink = cursor.marker_blink;
        v.hover = scroll.hover;
        v.delay = scroll.delay;
        std::ptr::copy_nonoverlapping(w.soldiers.as_ptr(), soldiers, sn);
        std::ptr::copy_nonoverlapping(w.tiles.as_ptr(), tiles, tn);
        std::ptr::copy_nonoverlapping(w.fx_anims.as_ptr(), fx_anims, an);
        std::ptr::copy_nonoverlapping(w.timed_fx.as_ptr(), timed_fx, fn_);
        0
    })
}

/// play::AnimTimers::start_table on the given tile animations, for the StartTileAnim 0x15720 test: `counts`, `anim_tile`
/// and `frames` hold 10 entries, `tiles` size::TILES bytes. Returns the animation's number, -1 when none is free.
///
/// # Safety
/// The pointers are valid for the sizes above.
#[no_mangle]
pub unsafe extern "C" fn ja1f_tile_anim_start(counts: *mut i32, anim_tile: *mut i16, frames: *mut u8, tile: i32, table: i32,
                                              count: i32, tiles: *mut u8) -> i32 {
    let mut a = play::AnimTimers::default();
    for k in 0..10 {
        a.tiles[k].count = *counts.add(k);
        a.tiles[k].tile = *anim_tile.add(k);
        a.frames[k] = *frames.add(k);
    }
    let t = std::slice::from_raw_parts_mut(tiles, world::size::TILES);
    let r = a.start_table(tile, table as u8, count, t);
    for k in 0..10 {
        *counts.add(k) = a.tiles[k].count;
        *anim_tile.add(k) = a.tiles[k].tile;
        *frames.add(k) = a.frames[k];
    }
    r.map_or(-1, |k| k as i32)
}

/// The replay log through the quick save, for the CopyReplayRec? 0x2C7D0 and OpenReplayRec 0x2C834 test: a new-game world
/// with `n` records of `recs` writes QUICK.SAV (`tail`: its last 3 x n bytes; `count`: its block 36), and Quick Restore of
/// that file gives the log back (`back`, 3 x n bytes). Returns 0, or a negative error.
///
/// # Safety
/// The paths are C strings; `recs`, `tail` and `back` hold 3 x n bytes, `count` is valid.
#[no_mangle]
pub unsafe extern "C" fn ja1f_replay_tail(iso: *const c_char, exe: *const c_char, recs: *const u8, n: i32, tail: *mut u8,
                                          count: *mut i32, back: *mut u8) -> i32 {
    let (Some(iso), Some(exe)) = (cstr(iso), cstr(exe)) else { return E_ARG };
    let Ok(jagged) = Iso::open(iso).and_then(|mut i| i.read("DAT/JAGGED.DAT")) else { return E_FILE };
    let Ok(mut w) = world::World::new_game(iso, exe, 0, 0) else { return E_FILE };
    let k = 3 * n as usize;
    w.replay_log = std::slice::from_raw_parts(recs, k).chunks(3).map(|c| [c[0], c[1], c[2]]).collect();
    let q = quicksave::write(&w, -1);
    std::ptr::copy_nonoverlapping(q[q.len() - k..].as_ptr(), tail, k);
    *count = u16::from_le_bytes([q[0xC3B2], q[0xC3B3]]) as i32;
    w.replay_log.clear();
    if quicksave::restore(&mut w, &jagged, &q).is_err() {
        return E_FILE;
    }
    for (i, r) in w.replay_log.iter().enumerate().take(n as usize) {
        std::ptr::copy_nonoverlapping(r.as_ptr(), back.add(3 * i), 3);
    }
    if w.replay_log.len() != n as usize {
        return E_FILE;
    }
    0
}

/// sighting::merc_sees for the MercSees? 0x56D28 test: `out` holds slot, how, near.
///
/// # Safety
/// `out` is valid for 3 bytes.
#[no_mangle]
pub unsafe extern "C" fn ja1f_merc_sees(a: i32, b: i32, c: i32, out: *mut u8) {
    let mut s = sighting::Sight::default();
    sighting::merc_sees(&mut s, &[a, b, c]);
    *out = s.slot;
    *out.add(1) = s.how as u8;
    *out.add(2) = s.near;
}

/// play::stop_redraw for the RedrawMercPortrait? 0x336B4 test: bit 0 = the panel redraws, bit 1 = the cursor tile clears.
#[no_mangle]
pub extern "C" fn ja1f_stop_redraw(opt_onscreen: u8, slot: i32, selected: u8) -> i32 {
    let (d, c) = play::stop_redraw(opt_onscreen, slot, selected);
    d as i32 | (c as i32) << 1
}

/// faceanim::LipSync for the ReleaseTacticalTimer? 0x66D94 test: `n` bytes of voice, `ticks` timer steps. Bit 0 = the silence
/// flag before the release, bit 1 = after it, bit 2 = after it and `ticks` more steps.
///
/// # Safety
/// `pcm` is valid for `n` bytes.
#[no_mangle]
pub unsafe extern "C" fn ja1f_lip_release(pcm: *const u8, n: i32, ticks: i32) -> i32 {
    let mut l = faceanim::LipSync::new(std::slice::from_raw_parts(pcm, n as usize));
    let us = ticks as u64 * 2500;
    let was = l.at(us);
    l.release();
    let now = l.at(us);
    let later = l.at(us * 2 + 2500);
    was as i32 | (now as i32) << 1 | (later as i32) << 2
}

/// island::sector on the clean map for the DrawCellAndMarkers? 0x41994 test: `r` 60 x 9 bytes, sector `s`, its signed guard
/// mark; `clean` 320 x 200, in and out. Returns 0, or a negative error.
///
/// # Safety
/// `iso`, `exe`: NUL-terminated paths; the buffers have the sizes above.
#[no_mangle]
pub unsafe extern "C" fn ja1f_map_cell(iso: *const c_char, exe: *const c_char, r: *const u8, s: i32, mark: i32, clean: *mut u8) -> i32 {
    let (Some(iso), Some(exe)) = (cstr(iso), cstr(exe)) else { return E_ARG };
    let (map, d) = match (island::map_data(iso, exe), panel::panel_data(iso, exe)) {
        (Ok(m), Ok(d)) => (m, d),
        _ => return E_FILE,
    };
    let s = s as usize;
    let rec = std::slice::from_raw_parts(r.add(s * 9), 9);
    island::sector(&map, &d.sheet, rec, mark as i8, s, std::slice::from_raw_parts_mut(clean, 64000));
    0
}

/// World::from_parts on a day save for the CopySaveChunks? 0x4E840 test: `out` 60 x world::TMP_FILE bytes, the file of each
/// sector at its place; `have` 60 bytes, 1 = the sector has a file. Returns 0, or a negative error.
///
/// # Safety
/// `iso`, `exe`: NUL-terminated paths; `save` holds `n` bytes; the buffers have the sizes above.
#[no_mangle]
pub unsafe extern "C" fn ja1f_tmp_chunks(iso: *const c_char, exe: *const c_char, save: *const u8, n: usize, out: *mut u8, have: *mut u8) -> i32 {
    let (Some(iso), Some(exe)) = (cstr(iso), cstr(exe)) else { return E_ARG };
    let files = match world_files(iso, exe) {
        Ok(f) => f,
        Err(e) => return e,
    };
    let Ok(w) = world::World::from_parts(exe, &files.0, &files.1, std::slice::from_raw_parts(save, n)) else { return E_FILE };
    for (i, t) in w.tmp_files.iter().enumerate() {
        *have.add(i) = t.is_some() as u8;
        if let Some(t) = t {
            std::ptr::copy_nonoverlapping(t.as_ptr(), out.add(i * world::TMP_FILE), world::TMP_FILE);
        }
    }
    0
}

/// keyhandler_diff: `cursor::key_flags_pass`. io = [alt, ctrl, shift, saved mode, cursor mode, cursor tile]; returns 0 no
/// call, 1 MoveCursorState, 2 + mode CursorModeSet.
///
/// # Safety
/// `io` points at 6 ints.
#[no_mangle]
pub unsafe extern "C" fn ja1f_key_flags(flags: i32, io: *mut i32) -> i32 {
    let o = std::slice::from_raw_parts_mut(io, 6);
    let mut l = cursor::KeyLatches { alt: o[0] != 0, ctrl: o[1] != 0, shift: o[2] != 0, saved_mode: o[3] as u8 };
    let mut st = cursor::CursorState { mode: o[4] as u8, tile: o[5], ..Default::default() };
    let r = cursor::key_flags_pass(&mut st, &mut l, flags as u16);
    o[0] = l.alt as i32;
    o[1] = l.ctrl as i32;
    o[2] = l.shift as i32;
    o[3] = l.saved_mode as i32;
    o[4] = st.mode as i32;
    o[5] = st.tile;
    match r {
        cursor::CtrlCall::None => 0,
        cursor::CtrlCall::Hand => 1,
        cursor::CtrlCall::Back(m) => 2 + m as i32,
    }
}

/// keyhandler_diff: `tactkeys::route` for the key bytes `first`, `second` and the 15 gate values
/// (selected, combat, drawn, turn_shown, view_hold, paused, compress, held, level, save_anywhere, mode, quick_file,
/// in_play, hp, turn_moves). out: route number, its argument, back, clears marks, the debug level after a level key, the strip
/// reload (fan fix F67, under the fan_fixes mask).
///
/// # Safety
/// `gates` points to 15 i32, `out` to 6 i32.
#[no_mangle]
pub unsafe extern "C" fn ja1f_tactical_key(first: i32, second: i32, gates: *const i32, out: *mut i32) {
    use tactkeys::Route as R;
    let v = std::slice::from_raw_parts(gates, 15);
    let g = tactkeys::Gates {
        selected: v[0] as u8, combat: v[1] != 0, drawn: v[2] != 0, turn_shown: v[3] != 0, view_hold: v[4] != 0,
        paused: v[5] != 0, compress: v[6] != 0, held: v[7] != 0, level: v[8] as u8, save_anywhere: v[9] != 0,
        mode: v[10] as u8, quick_file: v[11] != 0, in_play: v[12] != 0, hp: v[13] as i8,
        turn_moves: v[14] != 0,
    };
    let r = tactkeys::route(first as u8, second as u8, &g);
    let (n, arg) = match r {
        R::None => (0, 0), R::EndPause => (1, 0), R::EndCompress => (2, 0), R::AskQuit => (3, 0), R::QuickSave => (4, 0),
        R::QuickRestore => (5, 0), R::Debug(c) => (6, c as i32), R::Merc(k) => (7, k as i32), R::SeesLine => (8, 0),
        R::Options => (9, 0), R::Arrow(s) => (10, s as i32), R::View(k) => (11, k as i32), R::ForceTurn => (12, 0),
        R::NextEnemy => (13, 0), R::Escape => (14, 0), R::NextMerc => (15, 0), R::Volume(c) => (16, c as i32),
        R::Abort => (17, 0), R::Compress => (18, 0), R::NoCompress => (19, 0), R::EndTurn => (20, 0),
        R::CursorBack => (21, 0), R::Inventory => (22, 0), R::SayAgain => (23, 0), R::Map => (24, 0), R::Pause => (25, 0),
        R::StatPopups => (26, 0), R::Traverse => (27, 0), R::Version => (28, 0), R::Swap => (29, 0), R::Popup => (30, 0),
    };
    debugkeys::set_level(g.level);
    if let R::Debug(c) = r {
        debugkeys::level_key(c);
    }
    let fan67 = ja1logic::fanfix::on(67);
    let o = std::slice::from_raw_parts_mut(out, 6);
    o[0] = n;
    o[1] = arg;
    o[2] = r.back_fan(fan67) as i32;
    o[3] = tactkeys::clears_marks(g.mode) as i32;
    o[4] = debugkeys::level() as i32;
    o[5] = r.strip_reload(fan67) as i32;
}

#[cfg(test)]
mod tests {
    use super::*;

    /// C callers read any value below 0 as an error and any other value as a result: every error code is negative
    /// and each one is different.
    #[test]
    fn error_codes_are_negative_and_distinct() {
        let codes = [E_ARG, E_SECTOR, E_VIEW, E_FILE, E_PANIC];
        assert!(codes.iter().all(|&c| c < 0));
        for (i, a) in codes.iter().enumerate() {
            assert!(codes[i + 1..].iter().all(|b| b != a));
        }
    }

    /// The view checks come before any file is read: a window off the map is E_VIEW, the whole map height is valid.
    #[test]
    fn view_bounds() {
        assert_eq!(render_view("", "", 1, -1, 0, 14, 12), Err(E_VIEW));
        assert_eq!(render_view("", "", 1, 0, -1, 14, 12), Err(E_VIEW));
        assert_eq!(render_view("", "", 1, 0, 0, 14, 57), Err(E_VIEW));
        assert_eq!(render_view("", "", 1, 43, 0, 14, 12), Err(E_VIEW));
        assert_eq!(check_view(0, 0, 1, 56), Ok(()));
        assert_eq!(check_view(0, 0, MAX_COLS, 56), Ok(()));
        assert_eq!(check_view(0, 1, 1, 56), Err(E_VIEW));
    }

    /// put_fields writes the quick-save field bytes into a buffer of exactly their size; one byte less is E_ARG.
    #[test]
    fn put_fields_fits_an_exact_buffer() {
        let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
        let (iso, exe) = (root.join("rom/JA1.gog"), root.join("build/game113b/JA.EXE"));
        if !iso.exists() || !exe.exists() {
            return;
        }
        let w = world::World::new_game(iso.to_str().unwrap(), exe.to_str().unwrap(), 0, 0).unwrap();
        let n = quicksave::field_bytes(&w).len();
        let mut buf = vec![0u8; n];
        assert_eq!(put_fields(&w, buf.as_mut_ptr(), n), n as i32);
        assert_eq!(&buf[..], &quicksave::field_bytes(&w)[..]);
        assert_eq!(put_fields(&w, buf.as_mut_ptr(), n - 1), E_ARG);
    }
}
