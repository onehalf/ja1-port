//! Entering a sector (docs/spec/world_load.md section 4) with the front-end as host (section 7), and drawing the
//! world's view with its soldiers (section 7 "Viewer", tactical_map.md "Soldiers").
use crate::host::{enter_sector, EnterHost};
use crate::iso::Iso;
use crate::render::{Scene, SceneCursor, SceneSoldiers, MAP};
use crate::soldier::{SoldierEnv, REC};
use crate::world::{World, SECTOR_COUNT, SECTOR_ROW};
use crate::{common, exe_tables, soldier_bank, tileset_data};
use std::cell::RefCell;

pub const COLS: i32 = 14;
pub const ROWS: i32 = 12;

/// A world after the sector entry, with what the drawing needs beside it.
pub struct Entered {
    pub world: World,
    /// The tileset byte of the TERRAIN.DAT record (drawing only).
    pub tileset: u8,
    /// The end seed and the count of random numbers drawn.
    pub seed: u32,
    pub draws: u64,
    /// The world as loaded, before the entry (the restart copy before the first day start, world_load.md
    /// section 8 "Abort or restart the day").
    pub loaded: Box<World>,
}

/// Step 1: `tiles` from TERRAIN.DAT record `cur_sector` (bytes 0..4), byte 5 = 0x40, byte 6 = 0xFF, byte 7 = 0.
pub fn fill_map(w: &mut World, iso_path: &str) -> Result<u8, String> {
    let c = common(iso_path).map_err(|e| format!("TERRAIN.DAT (error {})", e))?;
    let sector = w.cur_sector as usize + 1; // Common::fill_map takes the 1-based sector number
    c.fill_map(sector, &mut w.tiles);
    Ok(c.tileset(sector))
}

/// The top-left view tile from the world's view_x / view_y (pixels).
pub fn view_tile(w: &World) -> (i32, i32) {
    // SPEC GAP: the spec does not say what the viewer does with a view outside the map (or not on a tile corner);
    // the tile is view_x / 16, view_y / 14 (truncated), clamped so that the 14 x 12 window lies on the map.
    // panel.md "View modes": the window of the world's mode.
    let (cols, rows, _, _) = crate::viewmode::world_window(w);
    let c0 = (w.view_x / 16).clamp(0, MAP - cols);
    let r0 = (w.view_y / 14).clamp(0, MAP - rows);
    (c0, r0)
}

/// Sections 1-3 and 4: load, map, `ja1_enter_sector` with the section 7 host, draw the view once.
pub fn enter(iso_path: &str, exe_path: &str, save_path: &str, seed: u32) -> Result<Entered, String> {
    let mut w = World::load(iso_path, exe_path, save_path).map_err(|e| format!("world: {}", e))?;
    let loaded = Box::new(w.clone());
    let (tileset, seed, draws) = enter_world(iso_path, exe_path, &mut w, seed)?;
    Ok(Entered { world: w, tileset, seed, draws, loaded })
}

/// Section 4 on a loaded world (also the day loop's sector entry, section 9 step 4): map, prev_balance,
/// `ja1_enter_sector` with the section 7 host from `seed`, draw the view once. Returns the tileset, the end seed and
/// the count of random draws.
pub fn enter_world(iso_path: &str, exe_path: &str, w: &mut World, seed: u32) -> Result<(u8, u32, u64), String> {
    let cur = w.cur_sector as usize;
    if cur >= SECTOR_COUNT {
        // SPEC GAP: section 4 needs a tactical sector (TERRAIN.DAT has 60 records, entry tiles 0..59).
        return Err(format!("cur_sector {} is not a tactical sector (0..59)", cur));
    }
    // SPEC GAP: section 4 (op 92): a sector with row byte 0xA9 != 0 and no kept tmp file is refused.
    if w.sectors[cur * SECTOR_ROW + 0xA9] != 0 && w.tmp_files[cur].is_none() {
        return Err(format!("sector {} has a saved sector file (sectors row byte 0xA9 != 0) but none is kept", cur));
    }
    let tileset = fill_map(w, iso_path)?;
    w.tileset = tileset;
    w.accounts[0x98] = tileset; // g_tileset 0x22A038 (LoadTerrainTileset), saved at file 20836
    w.loss_slot = 0x40; // g_lossSlot 0x22A1F1: SetupSectorFromXra 0x2EACA
    w.loss_timer = -1; // g_lossTimer 0x92DE8: SetupSectorFromXra 0x2EA72
    let entry_tile = w.entry_tiles[cur];
    let mut iso = Iso::open(iso_path).map_err(|e| format!("{}: {}", iso_path, e))?;
    let mut host = EnterHost::new(&mut iso, seed);
    host.exe = Some(exe_path.to_string());
    host.tmp_files = w.tmp_files.clone();
    // Step 2: prev_balance = the i32 at accounts +0x24 (the funds at the start of the tactical day).
    w.prev_balance = i32::from_le_bytes([w.accounts[0x24], w.accounts[0x25], w.accounts[0x26], w.accounts[0x27]]);
    // strategic.md The day's verdict, Day-start copies: the accounts at the entry, and the day's death counts = 0.
    let acc = |o: usize| i32::from_le_bytes([w.accounts[o], w.accounts[o + 1], w.accounts[o + 2], w.accounts[o + 3]]);
    w.start_trees = acc(0x18);
    w.start_workable = acc(0x14);
    w.start_tapped = acc(0x1C);
    w.start_sectors = acc(0x74);
    w.start_plants = acc(0x88);
    w.start_cap = acc(0x84);
    w.update.dead_mercs = 0;
    w.update.dead_allies = 0;
    w.update.dead_npc = 0;
    w.update.enemy_deaths = 0;
    // The tactical day starts with no enemy hit (first_hit = 0).
    w.update.first_hit = 0;
    // panel.md "The interface strips": the sector load sets the strip flag.
    w.strip_flag = 1;
    // LoadTacticalSector? 0x2AD20: the entry starts these again (loadsector_diff): the big panel 0x92E30, the abort
    // byte 0x92E74, the strip box 0x92E73, 0x92DC9, 0x92DA4, 0x22A10B, 0x22A0CF, and 0x284F13 = 1 (world: face_left).
    w.small_panel = 0;
    w.accounts[0x8F] = 0; // g_lossAmount 0x22A02F: 0x2AE5E
    w.update.quit_tactical = 0;
    w.inv.strip_box = 0;
    w.update.turn_flag = 0;
    w.update.redraw_pending = 0;
    w.update.squad_lost = 0;
    w.update.enemies_warned = 0;
    w.inv.face_left = 1;
    enter_sector(w, &mut host, entry_tile);
    let (seed, draws) = (host.rng.seed, host.rng.draws);
    // SPEC GAP: section 7 "view" in the test command is read as section 4 step 3 (draw the view once at the
    // world's view_x / view_y); the library's own view step runs inside ja1_enter_sector.
    // Step 3: draw the view once (writes drawAnchorX / drawAnchorY of the drawn soldiers).
    let (c0, r0) = view_tile(w);
    render_world(iso_path, exe_path, w, tileset, c0, r0, COLS, ROWS).map_err(|c| format!("draw (error {})", c))?;
    // Section 7: opt_reload_msg = 1 as after the step 3 draw, also when nothing is drawn (render_world sets it too).
    w.update.opt_reload_msg = 1;
    Ok((tileset, seed, draws))
}

/// Script word of each of the 48 slots: the u16 at byte offset (u32 soldier +0x90) in `anim_scripts`.
pub fn script_words(w: &World) -> Vec<u16> {
    (0..48)
        .map(|k| {
            let r = &w.soldiers[k * REC..];
            let off = u32::from_le_bytes([r[0x90], r[0x91], r[0x92], r[0x93]]) as usize;
            // SPEC GAP: a script position outside anim_scripts gives script word 0.
            match w.anim_scripts.get(off..off.wrapping_add(2)) {
                Some(b) => u16::from_le_bytes([b[0], b[1]]),
                None => 0,
            }
        })
        .collect()
}

/// The cols x rows view at tile (c0, r0) of the world, soldiers drawn as section 7 "Viewer" says
/// (show all 0, sight viewer 0xFF, selected merc = `selected`, shade table 28). The soldier records get their
/// drawAnchorX / drawAnchorY, and `opt_reload_msg` = 1.
#[allow(clippy::too_many_arguments)]
pub fn render_world(iso_path: &str, exe_path: &str, w: &mut World, tileset: u8, c0: i32, r0: i32, cols: i32, rows: i32) -> Result<Vec<u8>, i32> {
    render_world_cursor(iso_path, exe_path, w, tileset, c0, r0, cols, rows, None, None, None)
}

/// The view texts after the "sees enemies" line (panel.md "The turn banner and the other view texts"): text, y, fg.
/// `banner` = the text and colour of the latest host op 194 banner.
/// `dark_flag`: the one-draw darken flag (0x93DBE): TacticalRedraw 0x176C8 shows "TIME COMPRESSION..." only without it.
pub fn view_texts(w: &World, banner: Option<(&str, u8)>, dark_flag: bool) -> Vec<(Vec<u8>, i32, u8)> {
    let u = &w.update;
    let mut out = Vec::new();
    if u.turn_timer2 >= 1 {
        if let Some((t, c)) = banner {
            out.push((t.as_bytes().to_vec(), u.turn_timer2 as i32 + 28, c));
        }
        return out;
    }
    if u.turn_timer >= 1 {
        out.push((b"INTERRUPT!".to_vec(), 48, 0x16));
    } else if w.update.time_compress != 0 && !dark_flag {
        out.push((b"TIME COMPRESSION...".to_vec(), w.view_h / 2 + 18, 0xFF));
    }
    if u.stop_timer >= 1 && u.screen_drawn != 0 {
        out.push((b"RESERVE WARNING...".to_vec(), w.view_h + 8, 0x16));
    }
    out
}

thread_local! {
    static DARKEN: std::cell::Cell<bool> = const { std::cell::Cell::new(false) };
    static LABEL_SLOTS: std::cell::Cell<(u8, u8)> = const { std::cell::Cell::new((0x40, 0x40)) };
    static VIEW_FACES: RefCell<Vec<u8>> = const { RefCell::new(Vec::new()) };
}

/// The 8 x 1024 face buffers for the slot portraits over the full view (main.rs sets them before each frame).
pub fn view_faces(faces: &[u8]) {
    VIEW_FACES.with(|f| {
        let mut f = f.borrow_mut();
        f.clear();
        f.extend_from_slice(faces);
    });
}

/// cursor.md "Hover": the lit and the named slot after the last hover step, for the soldier texts
/// (tactical_map.md "Soldier texts").
pub fn label_slots(lit: u8, named: u8) {
    LABEL_SLOTS.with(|l| l.set((lit, named)));
}

/// The lit slot (0x40 = none): the merc slot whose portrait the mouse lights (cursor.md "Light"). The slot drawing
/// gives it the white face frame and draws its portrait over the shade (panel.md "Drawing one slot", the slot that
/// the original keeps in the same byte as the speaker slot).
pub fn lit_slot() -> u8 {
    LABEL_SLOTS.with(|l| l.get().0)
}

/// panel.md "The dark view": the darken flag, a one-draw flag. The next full draw darkens the map view (as time
/// compression does) and clears it. The Control Panel sets it before its full draw.
pub fn darken_next() {
    DARKEN.with(|d| d.set(true));
}

/// `render_world` with the tile cursor (cursor.md "Drawing") when `cursor` is given.
#[allow(clippy::too_many_arguments)]
pub fn render_world_cursor(iso_path: &str, exe_path: &str, w: &mut World, tileset: u8, c0: i32, r0: i32, cols: i32, rows: i32,
                           cursor: Option<&mut crate::cursor::CursorState>, banner: Option<(&str, u8)>,
                           ov_cursor: Option<crate::overview::OvCursor>) -> Result<Vec<u8>, i32> {
    let c = common(iso_path)?;
    let e = exe_tables(exe_path)?;
    let t = tileset_data(iso_path, tileset)?;
    let bank = soldier_bank(iso_path)?;
    let sheets = crate::fx_sheets(exe_path)?;
    let fx_entries = w.fx_anims.clone();
    // panel.md "Tactical messages": the "sees enemies" line while screen_drawn != 0 and sees_slot < 0x40.
    let sees = w.update.sees_slot;
    let line = w.update.screen_drawn != 0 && sees < 0x40;
    // TacticalRedraw clears the darken flag on every frame (0x1795x), with time compression on too.
    let dark_flag = DARKEN.with(|d| d.replace(false));
    let texts = view_texts(w, banner, dark_flag);
    let time_compress = w.update.time_compress != 0 || dark_flag;
    let overview = crate::viewmode::redraw_choice(w.opt_onscreen, 1).is_some();
    let mut cursor = cursor;
    // cursor.md "Soldier and tile marks": the marks come from the world fields.
    if let Some(c) = cursor.as_deref_mut() {
        (c.shown_slot, c.spotted, c.view_tile) = (w.update.shown_slot, w.update.spotted, w.update.view_tile);
    }
    // Step 4's combat / tactical_on / overlay_up come from the cursor state.
    let ov_cursor = ov_cursor.map(|mut o| {
        if let Some(c) = cursor.as_deref() {
            (o.combat, o.tactical_on, o.overlay_up) = (c.combat as i32, c.tactical_on as i32, c.overlay_up as i32);
        }
        o
    });
    // panel.md "Pause (key P) and the clock": the clock (switch and blink byte on) with the time on the clock.
    let clock = ov_cursor.filter(|o| o.clock != 0).map(|o| (o.clock_hour as i8, o.clock_minute as i8));
    let full_view = w.opt_onscreen >= 2;
    let pd = if overview || full_view || clock.is_some() || cursor.is_some() || line || !texts.is_empty() { Some(crate::panel::panel_data(iso_path, exe_path)?) } else { None };
    // The sight viewer = sees_slot (0xFF while it is 0x40); seen[v][k] = record v byte +0x112 + k.
    let (viewer, seen) = if sees < 0x40 { (sees, w.soldiers.get(0x112..).map(|s| s.to_vec()).unwrap_or_default()) } else { (0xFF, Vec::new()) };
    let sees_text = w.sees_text.clone();
    let view_w = w.view_w;
    let words = script_words(w);
    let mut remap = c.remap.clone();
    remap[28] = t.table28;
    let env = SoldierEnv { bank: &bank, remap: &remap, seen: &seen, show_all: w.update.follow_all != 0, viewer, selected: w.selected, shade: 28 };
    let tiles = w.tiles.clone();
    let (items, filter) = (w.ground_items.clone(), w.overview_filter & 1 != 0);
    let soldier_copy = if overview { w.soldiers.clone() } else { Vec::new() };
    let sector = w.cur_sector as usize + 1;
    let shadows = crate::ctlpanel::get(w, 4) != 0;
    let dmg_text = w.dmg_text.clone();
    let (lit, named) = LABEL_SLOTS.with(|l| l.get());
    let view_faces = if full_view { VIEW_FACES.with(|f| f.borrow().clone()) } else { Vec::new() };
    let table29 = crate::panel::table29(&c);
    let mut ap_warned = [0u8; 8];
    for (k, a) in ap_warned.iter_mut().enumerate() {
        *a = w.ap_warned.get(k).copied().unwrap_or(0);
    }
    let (view_mode, view_combat) = (w.opt_onscreen, w.combat);
    let sol_texts = pd.as_ref().map(|pd| RefCell::new(crate::render::SoldierTexts {
        font0: &pd.fonts[0], font1: &pd.fonts[1], table0: c.remap[0], lit, named, dmg_text: &dmg_text }));
    let soldiers = SceneSoldiers { env, records: RefCell::new(&mut w.soldiers[..]), script_words: &words, texts: sol_texts };
    let scene = Scene {
        common: &c,
        exe: &e,
        ts: &t,
        sector,
        c0,
        r0,
        cols,
        rows,
        tilemap: Some(&tiles),
        soldiers: Some(soldiers),
        shadows,
        roofs: crate::debugkeys::roofs() != 0,
        cursor: match (cursor, &pd) {
            (Some(st), Some(pd)) => Some(SceneCursor { st: RefCell::new(st), font0: &pd.fonts[0] }),
            _ => None,
        },
        lines: match &pd {
            Some(pd) => (if line { Some((sees_text, 34, 0xFF)) } else { None })
                .into_iter()
                .chain(texts)
                .map(|(t, y, fg)| (&pd.fonts[2], t, view_w / 2 + 32, y, fg))
                .collect(),
            None => Vec::new(),
        },
        fx: Some(crate::render::SceneFx { sheets: &sheets, entries: &fx_entries, tile_frames: w.tile_frames }),
        // Time compression darkens the view after the "sees enemies" line, before the other view texts.
        dark: if time_compress { Some((line as usize, &t.table28)) } else { None },
        portraits: match &pd {
            Some(pd) if full_view => Some(RefCell::new(crate::panel::ViewPortraits {
                d: pd,
                table0: t.table0,
                table29: &table29,
                tileset: &t,
                faces: &view_faces,
                mode: view_mode,
                lit,
                combat: view_combat as u8,
                ap_warned,
            })),
            _ => None,
        },
        ground: Some(&w.ground_items),
    };
    // panel.md "The overview picture": mode 0 draws it instead of the map view (roofs flag and "show all" =
    // follow_all, cursor.md "Debug keys (level 4)").
    let img = match (&pd, overview) {
        (Some(pd), true) => {
            let ov = crate::overview::OverviewIn { tiles: &tiles, items: &items, soldiers: &soldier_copy, fx: &fx_entries,
                sees_slot: sees, filter, roofs: crate::debugkeys::roofs() != 0, show_all: w.update.follow_all != 0 };
            scene.overview_window(&pd.sheet, &ov, &pd.fonts[0], ov_cursor.as_ref())
        }
        // The normal view draws the clock after the cursor, at the window's top-left corner (32, 28).
        (Some(pd), false) if clock.is_some() => {
            let (h, m) = clock.unwrap_or_default();
            let mut table0 = c.remap[0];
            scene.window_then(|bb| crate::pause::clock(bb, &t, &pd.fonts[0], &mut table0, 32, 28, h, m))
        }
        _ => scene.window(),
    };
    drop(scene);
    // Section 4 step 3: every full draw of the tactical screen sets opt_reload_msg = 1 (the viewer's draws too).
    w.update.opt_reload_msg = 1;
    Ok(img)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// panel.md "The overview picture": mode 0 draws the overview (it needs the panel sheet), and bit 0 of the
    /// overview filter draws unseen tiles as plain ground.
    #[test]
    fn overview_mode_and_its_filter_bit() {
        let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
        let (iso, exe) = (root.join("rom/JA1.gog"), root.join("build/game113b/JA.EXE"));
        if !iso.exists() || !exe.exists() {
            return;
        }
        let (iso, exe) = (iso.to_str().unwrap(), exe.to_str().unwrap());
        let mut w = World::new_game(iso, exe, 0, 0).unwrap();
        w.cur_sector = 38;
        let ts = fill_map(&mut w, iso).unwrap();
        for k in 0..3136 {
            w.tiles[k * 8 + 4] &= 0x7F;
        }
        let mut draw = |mode: u8, filter: u8| {
            (w.opt_onscreen, w.overview_filter) = (mode, filter);
            render_world(iso, exe, &mut w, ts, 20, 20, COLS, ROWS).unwrap()
        };
        let map = draw(1, 0);
        let ov = draw(0, 0);
        assert_ne!(ov, map, "mode 0 is the overview");
        assert_ne!(draw(0, 1), ov, "filter bit 0 changes the unseen tiles");
        assert_eq!(draw(0, 2), ov, "filter bit 1 does not");
        // a view text alone (INTERRUPT!) needs the panel fonts too
        w.update.turn_timer = 1;
        let mut draw = |mode: u8, filter: u8| {
            (w.opt_onscreen, w.overview_filter) = (mode, filter);
            render_world(iso, exe, &mut w, ts, 20, 20, COLS, ROWS).unwrap()
        };
        assert_ne!(draw(1, 0), map, "the INTERRUPT! text shows");
    }
}
