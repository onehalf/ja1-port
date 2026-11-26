//! `ja1-port <JA1.gog> <JA.EXE> [sector] [--png OUT]`: the static tactical view (14 x 12 tiles) in a 4x window.
//! `ja1-port <JA1.gog> <JA.EXE> --world SAVE OUT`: the world from the files (docs/spec/world_load.md sections 1-3)
//! written as the section 6 world image.
//! `ja1-port <JA1.gog> <JA.EXE> --enter SAVE SEED OUT`: load, enter the save's sector (section 4, the front-end as
//! host: section 7), write the world image to OUT and print `seed S draws D`.
//! `ja1-port <JA1.gog> <JA.EXE> --play SAVE [SEED] [--png OUT]`: enter the sector and show it with its soldiers;
//! the window runs the sector (section 8: 10 ms ticks; per pass frame, update end, fuses; left click = shoot or
//! walk, keys 1..8 = choose a merc, D = end turn, I = the inventory screen).
//! `ja1-port <JA1.gog> <JA.EXE> --walk SAVE SEED SLOT TILE TICKS OUT`: section 8 "Test command".
//! `ja1-port <JA1.gog> <JA.EXE> --script SAVE SEED SCRIPT OUT`: section 8 "Play script" (no window, no real time).
//! `ja1-port <JA1.gog> <JA.EXE> --save-copy SAVE OUT`: the world from the files, written at once as a day save
//! (section 10); OUT equals SAVE except the sector reset.
//! `--png`, `--world`, `--enter`, `--walk` and `--script` need no SDL; the window needs the `gui` feature.
use ja1front::mapui::BoxInput;
use ja1front::strips::StripAction;
use ja1front::{palette, render_view, sector_tileset};
#[cfg(feature = "gui")]
use sdl2::{event::Event, keyboard::Keycode, pixels::PixelFormatEnum};
use std::process::exit;

const COLS: i32 = 14;
const ROWS: i32 = 12;
const W: u32 = 16 * COLS as u32;
const H: u32 = 14 * ROWS as u32;
/// The tactical screen (panel.md).
const SW: u32 = 320;
const SH: u32 = 200;
#[cfg(feature = "gui")]
const SCALE: u32 = 4;
#[cfg(feature = "gui")]
mod frame;
#[cfg(feature = "gui")]
mod setupwin;
#[cfg(feature = "gui")]
use frame::Frame;

fn usage() -> ! {
    eprintln!("usage: ja1-port <JA1.gog> <JA.EXE> [sector 1..60] [--png OUT]");
    eprintln!("       ja1-port <JA1.gog> <JA.EXE> --world SAVE OUT");
    eprintln!("       ja1-port <JA1.gog> <JA.EXE> --save-copy SAVE OUT");
    eprintln!("       ja1-port <JA1.gog> <JA.EXE> --new-game D Q OUT");
    eprintln!("       ja1-port <JA1.gog> <JA.EXE> --enter SAVE SEED OUT");
    eprintln!("       ja1-port <JA1.gog> <JA.EXE> --play SAVE [SEED] [--png OUT]");
    eprintln!("       ja1-port <JA1.gog> <JA.EXE> --walk SAVE SEED SLOT TILE TICKS OUT");
    eprintln!("       ja1-port <JA1.gog> <JA.EXE> --script SAVE|menu SEED SCRIPT OUT");
    eprintln!("       ja1-port <JA1.gog> <JA.EXE> --serve SAVE|menu SEED LOG OUT");
    eprintln!("       ja1-port <JA1.gog> <JA.EXE> --menu [SAVE_DIR]");
    eprintln!("       ja1-port <JA1.gog> <JA.EXE> --setup        (play settings in ja1.cfg, then the game)");
    eprintln!("       ja1-port                  (no arguments: the game found next to the program, main menu)");
    eprintln!("       ja1-port --setup          (the setup screen of that game)");
    exit(2)
}

fn fail(what: &str, code: i32) -> ! {
    eprintln!("ja1-port: {} failed (error {})", what, code);
    exit(1)
}

fn frame(iso: &str, exe: &str, sector: i32, c0: i32, r0: i32) -> (Vec<u8>, [u8; 768]) {
    let idx = render_view(iso, exe, sector, c0, r0, COLS, ROWS).unwrap_or_else(|e| fail("render", e));
    let ts = sector_tileset(iso, sector).unwrap_or_else(|e| fail("tileset", e));
    let pal = palette(iso, ts as i32).unwrap_or_else(|e| fail("palette", e));
    (idx, pal)
}

fn write_png(path: &str, idx: &[u8], pal: &[u8; 768]) -> std::io::Result<()> {
    write_png_wh(path, idx, pal, W, H)
}

fn write_png_wh(path: &str, idx: &[u8], pal: &[u8; 768], w: u32, h: u32) -> std::io::Result<()> {
    let f = std::io::BufWriter::new(std::fs::File::create(path)?);
    let mut enc = png::Encoder::new(f, w, h);
    enc.set_color(png::ColorType::Indexed);
    enc.set_depth(png::BitDepth::Eight);
    enc.set_palette(pal.to_vec());
    let mut w = enc.write_header().map_err(std::io::Error::other)?;
    w.write_image_data(idx).map_err(std::io::Error::other)?;
    Ok(())
}

/// `--world SAVE OUT`: load the world (no sector entry) and write its image.
fn world(iso: &str, exe: &str, save: &str, out: &str) -> ! {
    let w = ja1front::world::World::load(iso, exe, save).unwrap_or_else(|e| {
        eprintln!("ja1-port: world: {}", e);
        exit(1)
    });
    std::fs::write(out, w.image()).unwrap_or_else(|e| {
        eprintln!("ja1-port: {}: {}", out, e);
        exit(1)
    });
    exit(0)
}

/// `--save-copy SAVE OUT`: load the world (no sector entry) and write it as a day save (section 10).
fn save_copy(iso: &str, exe: &str, save: &str, out: &str) -> ! {
    let w = ja1front::world::World::load(iso, exe, save).unwrap_or_else(|e| die(format!("world: {}", e)));
    std::fs::write(out, w.save_bytes()).unwrap_or_else(|e| die(format!("{}: {}", out, e)));
    exit(0)
}

/// Where the day loop writes its day saves (section 10): `JA1_SAVE_DIR`, else the directory of the loaded save
/// (`None` in the play script: it writes only when `JA1_SAVE_DIR` is set).
fn save_dir(save: Option<&str>) -> Option<std::path::PathBuf> {
    save_dir_of(std::env::var("JA1_SAVE_DIR").ok(), save)
}

/// `save_dir` with the value of `JA1_SAVE_DIR` given.
fn save_dir_of(env: Option<String>, save: Option<&str>) -> Option<std::path::PathBuf> {
    match (env, save) {
        (Some(d), _) if !d.is_empty() => Some(d.into()),
        (_, Some(s)) => Some(std::path::Path::new(s).parent().map(|p| p.to_path_buf()).filter(|p| !p.as_os_str().is_empty()).unwrap_or(".".into())),
        _ => None,
    }
}

/// The game folder of a start with no paths. Searched in this order, each with its subfolders two levels down (so a
/// whole GOG folder dropped in works): JA1_GAME_DIR, the `game` folder beside the program, the program's folder, the
/// working folder; then the program's parent folder; then the usual GOG install folders (GOG Galaxy and the GOG
/// installer on Windows, Heroic and `~/GOG Games` elsewhere). A folder counts when it holds the CD image JA1.gog and
/// JA.EXE (any letter case), as a GOG install does.
fn find_game() -> Result<(String, String), String> {
    use std::path::PathBuf;
    let mut roots: Vec<(PathBuf, u32)> = Vec::new();
    if let Some(d) = std::env::var_os("JA1_GAME_DIR") {
        roots.push((d.into(), 2));
    }
    let me = std::env::current_exe().ok().and_then(|p| p.parent().map(|p| p.to_path_buf()));
    if let Some(d) = &me {
        roots.push((d.join("game"), 2));
        roots.push((d.clone(), 2));
    }
    if let Ok(d) = std::env::current_dir() {
        roots.push((d, 2));
    }
    if let Some(p) = me.as_ref().and_then(|d| d.parent()) {
        roots.push((p.to_path_buf(), 0));
    }
    for v in ["ProgramFiles(x86)", "ProgramFiles"] {
        if let Some(d) = std::env::var_os(v) {
            roots.push((PathBuf::from(d).join("GOG Galaxy").join("Games"), 1));
        }
    }
    let drive = std::env::var("SystemDrive").unwrap_or_else(|_| "C:".into());
    roots.push((PathBuf::from(format!("{}\\", drive)).join("GOG Games"), 1));
    if let Some(h) = std::env::var_os("HOME").or_else(|| std::env::var_os("USERPROFILE")) {
        let h = PathBuf::from(h);
        roots.push((h.join("GOG Games"), 1));
        roots.push((h.join("Games").join("Heroic"), 1));
    }
    let mut seen = std::collections::HashSet::new();
    roots.retain(|(d, _)| seen.insert(d.clone()));
    for (d, depth) in &roots {
        if let Some(found) = scan_game(d, *depth) {
            return Ok(found);
        }
    }
    let list: Vec<String> = roots.iter().filter(|(_, n)| *n == 2).map(|(d, _)| format!("  {}", d.display())).collect();
    Err(format!(
        "No Jagged Alliance game files found.\n\nCopy your GOG Jagged Alliance folder (the one with JA1.gog and JA.EXE) \
         into the \"game\" folder beside this program, then start it again.\n\nOther ways: set JA1_GAME_DIR to that \
         folder, or start\n  ja1front <JA1.gog> <JA.EXE> --menu\n\nFolders searched (with their subfolders):\n{}",
        list.join("\n")
    ))
}

/// `dir` or a subfolder down to `depth` levels (sorted by name) that holds JA1.gog and JA.EXE.
fn scan_game(dir: &std::path::Path, depth: u32) -> Option<(String, String)> {
    if let (Some(iso), Some(exe)) = (find_file(dir, "ja1.gog"), find_file(dir, "ja.exe")) {
        return Some((iso, exe));
    }
    if depth == 0 {
        return None;
    }
    let mut subs: Vec<std::path::PathBuf> = std::fs::read_dir(dir).ok()?.flatten().map(|e| e.path()).filter(|p| p.is_dir()).collect();
    subs.sort();
    subs.iter().find_map(|s| scan_game(s, depth - 1))
}

/// The path of the file `name` (lower case) in `dir`, in any letter case.
fn find_file(dir: &std::path::Path, name: &str) -> Option<String> {
    std::fs::read_dir(dir).ok()?.flatten().find_map(|e| {
        let p = e.path();
        (p.is_file() && e.file_name().to_string_lossy().to_lowercase() == name).then(|| p.to_string_lossy().into_owned())
    })
}

/// No game files: the message on stderr and, with a window, in a message box (a double-click has no terminal).
fn no_game(msg: String) -> ! {
    eprintln!("ja1-port: {}", msg);
    #[cfg(feature = "gui")]
    {
        use sdl2::messagebox::{show_simple_message_box, MessageBoxFlag};
        let _ = show_simple_message_box(MessageBoxFlag::ERROR, "JA1 port", &msg, None);
    }
    exit(1)
}

fn die(msg: String) -> ! {
    eprintln!("ja1-port: {}", msg);
    exit(1)
}

fn parse_seed(s: &str) -> u32 {
    s.parse().unwrap_or_else(|_| usage())
}

/// `--enter SAVE SEED OUT`: world_load.md section 7 "Test command".
fn enter(iso: &str, exe: &str, save: &str, seed: u32, out: &str) -> ! {
    let e = ja1front::enter::enter(iso, exe, save, seed).unwrap_or_else(|m| die(m));
    std::fs::write(out, e.world.image()).unwrap_or_else(|m| die(format!("{}: {}", out, m)));
    println!("seed {} draws {}", e.seed, e.draws);
    exit(0)
}

/// `--play SAVE [SEED] [--png OUT]`: section 7 "Viewer", in the tactical screen (panel.md: the 8 merc slots and the
/// map view at (48, 14)).
fn play(iso: &str, exe: &str, save: &str, seed: u32, png_out: Option<String>) -> ! {
    ja1front::record::start(exe, &format!("save {}", save));
    let mut e = ja1front::enter::enter(iso, exe, save, seed).unwrap_or_else(|m| die(m));
    let (c0, r0) = ja1front::enter::view_tile(&e.world);
    let ts = e.tileset;
    let title = format!("ja1front: sector {} (cur_sector {})", e.world.cur_sector as i32 + 1, e.world.cur_sector);
    let pal = palette(iso, ts as i32).unwrap_or_else(|c| fail("palette", c));
    let mut screen = ja1front::screen::Screen::new(iso, exe, &mut e.world).unwrap_or_else(|c| fail("faces", c));
    if let Some(out) = png_out {
        // Check aid: JA1_HOLD=K shows the screen while the left button is held on the bar area of slot K.
        let mut hold = None;
        if let Ok(k) = std::env::var("JA1_HOLD") {
            use ja1front::screen::{popup_press, slot_origin, spot, Spot};
            let k: usize = k.parse().unwrap_or_else(|_| usage());
            let (sx, sy) = slot_origin(k.min(7));
            let (bx, by) = if e.world.soldiers[k.min(7) * ja1front::soldier::REC + 0x180] == 0 { (3, 4) } else { (35, 4) };
            let at = spot(&e.world.soldiers, sx + bx, sy + by, W as i32, H as i32, e.world.opt_onscreen);
            screen.hover = if let Spot::Bars(h) = at { h } else { 0xFF };
            hold = Some(popup_press(&e.world.soldiers, screen.hover, at).unwrap_or_else(|| die(format!("no popup for slot {}", k))));
        }
        let mut idx = screen_frame(iso, exe, &mut e, &mut screen, c0, r0, &mut 0, true, None);
        if let Some(k) = hold {
            screen.popup(iso, exe, &e.world, k, &mut idx).unwrap_or_else(|c| fail("popup", c));
        }
        write_png_wh(&out, &idx, &pal, SW, SH).unwrap_or_else(|m| die(format!("{}: {}", out, m)));
        exit(0)
    }
    #[cfg(feature = "gui")]
    {
        let (to_menu, g) = play_window(iso, exe, &title, &mut e, &mut screen, &pal, c0, r0, save_dir(Some(save)), gfx(&title, iso, exe), false, false);
        if to_menu {
            // ending.md "Game end" step 4 (and Leave): the main menu opens.
            menu_loop(iso, exe, save_dir(Some(save)), g, Some(e.world), ja1front::world::start_seed());
        }
    }
    #[cfg(not(feature = "gui"))]
    play_window(iso, exe, &title, &mut e, &mut screen, &pal, c0, r0, save_dir(Some(save)));
    exit(0)
}

/// screen_frame with the play loop's blink byte, banner and overview cursor.
#[allow(clippy::too_many_arguments)]
fn play_frame(iso: &str, exe: &str, e: &mut ja1front::enter::Entered, screen: &mut ja1front::screen::Screen, c0: i32, r0: i32,
              play: &mut ja1front::play::Play, blink_step: bool) -> Vec<u8> {
    screen.ov_cursor = Some(play.ov_cursor(&e.world));
    // cursor.md "Drag to an edge": the drag's arrow (or "no") cursor shows while the left button is held.
    let value = screen.cursor.value;
    let tile = screen.cursor.tile;
    if let (Some(v), Some(d)) = (play.drag_value, play.drag) {
        screen.cursor.value = v;
        screen.cursor.tile = d.loop_.cursor_tile;
    }
    screen.cont = play.cont;
    let mut img = screen_frame(iso, exe, e, screen, c0, r0, &mut play.inv_blink, blink_step, play.host.banner.map(|b| (b.text, b.colour)));
    screen.cursor.value = value;
    screen.cursor.tile = tile;
    // #20 B: panel.md "Clicks on the bottom strip" step 3: a held item is the pointer (its picture and the hand).
    // HideMouse? 0x67368 takes the held item away in a hold (`held_gone`).
    if !play.held_off {
        ja1front::pointer::held_on_screen(iso, exe, &e.world, &mut img, play.mouse_px.0, play.mouse_px.1)
            .unwrap_or_else(|c| fail("pointer", c));
    }
    img
}

/// The start or the end of a hold for the held item: a held item is the pointer, so HideMouse? 0x67368 (mouse off the map)
/// restores the picture under it, and the frames of the hold do not show it. stderr `held gone N` with an item on the cursor.
#[cfg(feature = "gui")]
fn held_gone(play: &mut ja1front::play::Play, w: &ja1front::world::World, gone: bool) {
    if ja1front::pointer::held_is_pointer(w.held[0]) && play.held_off != gone {
        eprintln!("held gone {}", gone as u8);
    }
    play.held_off = gone;
}

/// cursor.md "Tile popups": the popup's screens over the screen as it is now (the cursor's 16 x 14 block from the
/// same screen with no tile cursor) and the index of the end wait's screen.
#[allow(clippy::too_many_arguments)]
fn popup_screens(iso: &str, exe: &str, e: &mut ja1front::enter::Entered, screen: &mut ja1front::screen::Screen, c0: i32, r0: i32,
                 play: &mut ja1front::play::Play, p: &ja1front::tilepopup::Popup) -> (Vec<Vec<u8>>, usize) {
    let base = play_frame(iso, exe, e, screen, c0, r0, play, false);
    let over = screen.cursor.over_map;
    screen.cursor.over_map = 0;
    let clean = play_frame(iso, exe, e, screen, c0, r0, play, false);
    screen.cursor.over_map = over;
    popup_on(iso, exe, &base, &clean, p)
}

/// The 16 x 14 block of `clean` at (sx, sy) under the popup; a pixel off the 320 x 200 screen saves 0.
fn popup_save(clean: &[u8], sx: i32, sy: i32) -> Vec<u8> {
    let mut save = vec![0u8; 224];
    for r in 0..14 {
        for c in 0..16 {
            let (x, y) = (sx + c, sy + r);
            if (0..320).contains(&x) && (0..200).contains(&y) {
                save[16 * r as usize + c as usize] = clean[(320 * y + x) as usize];
            }
        }
    }
    save
}

/// The popup's screens over `base`, the saved block taken from `clean` at the tile's screen point.
fn popup_on(iso: &str, exe: &str, base: &[u8], clean: &[u8], p: &ja1front::tilepopup::Popup) -> (Vec<Vec<u8>>, usize) {
    let (sx, sy) = match *p {
        ja1front::tilepopup::Popup::Item { sx, sy, .. } | ja1front::tilepopup::Popup::Sign { sx, sy, .. } => (sx, sy),
    };
    let save = popup_save(clean, sx, sy);
    ja1front::tilepopup::screens(iso, exe, p, base, &save).unwrap_or_else(|c| fail("tile popup", c))
}

/// One picture of the tactical screen: the map view at (c0, r0) and the 8 slots.
#[allow(clippy::too_many_arguments)]
fn screen_frame(iso: &str, exe: &str, e: &mut ja1front::enter::Entered, screen: &mut ja1front::screen::Screen, c0: i32, r0: i32,
                blink: &mut u8, blink_step: bool, banner: Option<(&str, u8)>) -> Vec<u8> {
    // panel.md "View modes": the mode's window (mode 0: the overview picture, 224 x 168 like the 14 x 12 view).
    let (cols, rows, _, _) = ja1front::viewmode::world_window(&e.world);
    // The faces first: the portraits over the full view use them (TacticalRedraw 0x17759).
    screen.update_faces(iso, exe, &e.world).unwrap_or_else(|c| fail("faces", c));
    ja1front::enter::view_faces(&screen.faces);
    let view = ja1front::enter::render_world_cursor(iso, exe, &mut e.world, e.tileset, c0, r0, cols, rows, Some(&mut screen.cursor), banner,
                                                         screen.ov_cursor)
        .unwrap_or_else(|c| fail("render", c));
    screen.draw(iso, exe, &mut e.world, e.tileset, &view, cols * 16, rows * 14, blink, blink_step).unwrap_or_else(|c| fail("panel", c))
}

/// The view's size in pixels in the world's view mode (panel.md "View modes").
fn vw_now(w: &ja1front::world::World) -> (i32, i32) {
    let (cols, rows, _, _) = ja1front::viewmode::world_window(w);
    (cols * 16, rows * 14)
}

fn parse_i32(s: &str) -> i32 {
    s.parse().unwrap_or_else(|_| usage())
}

/// `--walk SAVE SEED SLOT TILE TICKS OUT`: section 8 "Test command".
fn walk(iso: &str, exe: &str, save: &str, seed: u32, slot: i32, tile: i32, ticks: u32, out: &str) -> ! {
    let mut e = ja1front::enter::enter(iso, exe, save, seed).unwrap_or_else(|m| die(m));
    if !(0..48).contains(&slot) {
        // SPEC GAP: the spec does not limit SLOT; a slot outside the 48 records is refused.
        die(format!("slot {} is not 0..47", slot));
    }
    // SPEC GAP: the random numbers of play go on from the end seed of the entry (one game, one seed).
    let mut host = ja1front::play::PlayHost::new(e.seed);
    let r = ja1front::play::walk(&mut e.world, &mut host, slot, tile, ticks);
    std::fs::write(out, e.world.image()).unwrap_or_else(|m| die(format!("{}: {}", out, m)));
    println!("walk {} at {}", r, ja1front::play::soldier_tile(&e.world, slot as usize));
    exit(0)
}

/// `--script SAVE SEED SCRIPT OUT`: section 8 "Play script".
/// `--serve` (docs/serve.md): the script lines come from stdin, one chunk per `sync` line; each chunk is appended to
/// LOG. Returns false at the end of stdin.
fn serve_chunk(text: &mut String, log: &str) -> bool {
    use std::io::{BufRead, Write};
    let mut chunk = String::new();
    let mut got = false;
    for l in std::io::stdin().lock().lines() {
        let l = l.unwrap_or_else(|m| die(format!("stdin: {}", m)));
        got = true;
        if l.trim() == "sync" {
            break;
        }
        chunk += &l;
        chunk.push('\n');
    }
    let mut f = std::fs::OpenOptions::new().create(true).append(true).open(log).unwrap_or_else(|m| die(format!("{}: {}", log, m)));
    f.write_all(chunk.as_bytes()).unwrap_or_else(|m| die(format!("{}: {}", log, m)));
    *text += &chunk;
    got
}

/// `--serve` at a `sync` (docs/serve.md "Output at a sync"): what `--script` prints at its end for the lines so far;
/// the day lines only from `printed` on (the program prints the others at an earlier sync).
fn serve_out(w: &ja1front::world::World, play: &ja1front::play::Play, printed: &mut usize, out: &str, end: bool) {
    use std::io::Write;
    println!("\x01day");
    for l in &play.day.lines[*printed..] {
        println!("{}", l);
    }
    *printed = play.day.lines.len();
    std::fs::write(out, w.image()).unwrap_or_else(|m| die(format!("{}: {}", out, m)));
    // The open shots and walks close as at a script end, on a copy: the run goes on with them open.
    eprintln!("\x01sync");
    play.host.ev.clone().flush();
    eprintln!("\x01end");
    println!("\x01report");
    print!("{}", ja1front::play::script_report(w, play));
    println!("\x01{}", if end { "exit" } else { "end" });
    std::io::stdout().flush().ok();
}

fn script(iso: &str, exe: &str, save: &str, seed: u32, path: &str, out: &str, serve: bool) -> ! {
    let mut text = String::new();
    if serve {
        let _ = std::fs::remove_file(path);
        serve_chunk(&mut text, path);
    } else {
        text = std::fs::read_to_string(path).unwrap_or_else(|m| die(format!("{}: {}", path, m)));
    }
    let mut cmds = ja1front::play::parse_script(&text).unwrap_or_else(|m| die(m));
    // SAVE "menu": the run starts at the main menu (world_load.md section 12).
    let mut menu = (save == "menu").then(|| ja1front::menu::Menu::new(iso, exe, save_dir(None)).unwrap_or_else(|c| fail("menu", c)));
    if menu.is_some() && std::env::var("JA1_TITLE").is_ok_and(|v| v == "1") {
        // ending.md "The title part (program start)" (JA1_TITLE=1; JA1_TITLE_PNG = the last film frame).
        let (lines, (scr, pal)) = ja1front::ending::script_title(iso, exe);
        for l in lines {
            println!("{}", l);
        }
        if let Ok(png) = std::env::var("JA1_TITLE_PNG") {
            write_png_wh(&png, &scr, &pal, SW, SH).unwrap_or_else(|m| die(format!("{}: {}", png, m)));
        }
    }
    let mut keys = std::collections::VecDeque::new();
    let menu_png = std::env::var("JA1_MENU_PNG").ok();
    let mut menu_stage = |menu: &mut ja1front::menu::Menu, cmds: &[ja1front::play::Cmd]| -> (Option<ja1front::menu::Outcome>, usize) {
        let mut r = (None, cmds.len());
        for (i, c) in cmds.iter().enumerate() {
            if *c == ja1front::play::Cmd::Music {
                menu.lines.push(ja1front::music::with(|m| m.line()));
            }
            if let ja1front::play::Cmd::Menu(x, y, b, k) = c {
                keys.extend(k.iter().copied());
                let o = menu.poll(*x, *y, b & 1 != 0, &mut keys);
                if std::mem::take(&mut menu.intro) && std::env::var("JA1_INTRO").is_ok_and(|v| v == "1") {
                    // ending.md "The new-game intro" (JA1_INTRO=1: the script runs take ~5 s without it otherwise).
                    let dir = std::path::Path::new(exe).parent().map(|p| p.to_path_buf()).unwrap_or_default();
                    let mut io = ja1front::sab::LogIo { dir: dir.clone(), iso: iso.to_string(), lines: Vec::new(), subtitles: false, draw: None, bright: None, films: false };
                    match ja1front::ending::play_intro(&mut io, &dir) {
                        Some((s, r)) => {
                            menu.lines.push(format!("intro island.SAB {} steps {}", r, io.lines.len()));
                            if let Ok(png) = std::env::var("JA1_INTRO_PNG") {
                                let pal: Vec<u8> = (0..256).flat_map(|i| s.dac[4 * i..4 * i + 3].to_vec()).collect();
                                let pal = ja1front::eod::scale_palette(&pal.try_into().unwrap());
                                write_png_wh(&png, &s.screen, &pal, SW, SH).unwrap_or_else(|m| die(format!("{}: {}", png, m)));
                            }
                        }
                        None => menu.lines.push("intro none".into()),
                    }
                }
                if let Some(o) = o {
                    r = (Some(o), i + 1);
                    break;
                }
            }
        }
        if let Some(png) = menu_png.as_ref() {
            let pal = ja1front::eod::scale_palette(&menu.pal);
            write_png_wh(png, &menu.buf, &pal, SW, SH).unwrap_or_else(|m| die(format!("{}: {}", png, m)));
        }
        r
    };
    // The world of a menu choice: a new game, or Continue (the world in memory; at the program start the world of
    // world_load.md section 12, World::program_start).
    let new_world = |d: u8, q: u8| ja1front::world::World::new_game(iso, exe, d, q).unwrap_or_else(|m| die(format!("world: {}", m)));
    let mut pos = 0;
    let mut e = match menu.as_mut() {
        None => {
            ja1front::record::start(exe, &format!("save {}", save));
            ja1front::enter::enter(iso, exe, save, seed).unwrap_or_else(|m| die(m))
        }
        Some(m) => {
            let (o, n) = menu_stage(m, &cmds);
            pos = n;
            let w = match o {
                Some(ja1front::menu::Outcome::NewGame(d, q)) => {
                    ja1front::record::start(exe, "new game");
                    let mut n = new_world(d, q);
                    n.sound_cfg_options(exe);
                    n
                }
                Some(ja1front::menu::Outcome::Continue) => {
                    ja1front::record::start(exe, "continue");
                    ja1front::world::World::program_start(iso, exe).unwrap_or_else(|m| die(format!("world: {}", m)))
                }
                _ if serve => die("serve: sync inside a menu".into()),
                _ => {
                    for l in &m.lines {
                        println!("{}", l);
                    }
                    exit(0)
                }
            };
            ja1front::enter::Entered { loaded: Box::new(w.clone()), world: w, tileset: 0, seed, draws: 0 }
        }
    };
    let mut play = ja1front::play::Play::new(e.seed);
    play.day.save_dir = save_dir(None);
    // savegame.md "Which loads take Sound FX and Music from the file": at program start both are on (Music: unless
    // JA1_MUSIC=0); the loaded save's two bytes do not count.
    ja1front::ctlpanel::put_sound_switches(&mut e.world, [1, ja1front::music::with(|m| m.on) as u8]);
    if let Some(m) = menu.as_mut() {
        play.day.lines.append(&mut m.lines);
        play.left = true;
        play.day.from_office = true;
    }
    // End-of-day screen (section 8): no window, nothing waits; table 0 as the tactical screen starts with.
    let t0 = ja1front::common(iso).unwrap_or_else(|c| fail("common", c)).remap[0];
    play.host.eod = Some(ja1front::eod::EodHost::new(iso, exe, t0, None));
    // The sunrise screen (section 9 step 3): no window, nothing waits.
    play.host.sunrise = Some(ja1front::sunrise::Sunrise::new(iso, exe, t0));
    // The day loop (section 9): `start` enters the new day's sector from these files.
    play.files = Some((iso.to_string(), exe.to_string()));
    play.host.explode = ja1front::iso::Iso::open(iso).ok().and_then(|mut i| i.read("DAT\\EXPLODE.DAT").ok());
    play.set_exe(exe).unwrap_or_else(|c| fail("JA.EXE", c));
    play.day.tileset = e.tileset;
    // Before the first day start of the run, "Restart this day" goes back to the world as loaded.
    play.day.day_save = Some(e.loaded.clone());
    play.keep_frame = std::env::var("JA1_SCREEN_PNG").is_ok();
    // Section 9 step 1: JA1_PAY_PNG = the "Not enough funds" picture at its first script poll, JA1_PAY_ASK_PNG = the
    // question box of its first op 275 pd_ask.
    // Op 301 payroll_show: the payroll screen over the replay screen at its end, its sounds on the screen sound and
    // the line `payroll steps N sounds A B`; JA1_PAYROLL_PNG = its screen at the end. The pay pictures use it.
    let pmap = std::rc::Rc::new(std::cell::RefCell::new(None::<ja1front::mapui::MapUi>));
    {
        let (pm, iso, exe, seed) = (pmap.clone(), iso.to_string(), exe.to_string(), e.seed);
        let wp = &e.world as *const ja1front::world::World;
        let png = std::env::var("JA1_PAYROLL_PNG").ok();
        play.host.payroll_show = Some(Box::new(move |under: Option<&[u8]>| {
            // SAFETY: the world stays in place during run_script; the library writes only through its view.
            let w = unsafe { &*wp };
            let mut pm = pm.borrow_mut();
            let map = pm.get_or_insert_with(|| ja1front::mapui::MapUi::new(&iso, &exe, seed));
            map.redraw(w).unwrap_or_else(|c| fail("map", c));
            let steps = map.payroll_steps(w, under).unwrap_or_else(|c| fail("payroll", c));
            let mut names = Vec::new();
            ja1front::sound::screen_main(&iso, &exe);
            for (fx, rate, vol, n) in steps.iter().filter_map(|s| s.sound) {
                ja1front::sound::screen(|x| x.sfx(fx, rate, vol, n));
                names.push(ja1front::sound::main_name(fx));
            }
            if let (Some(p), Some(last)) = (png.as_ref(), steps.last()) {
                write_png_wh(p, &last.screen, &map.pal, SW, SH).unwrap_or_else(|m| die(format!("{}: {}", p, m)));
                // The first screen: the replay screen still above the flip pictures.
                let p0 = format!("{}_first.png", p.trim_end_matches(".png"));
                write_png_wh(&p0, &steps[0].screen, &map.pal, SW, SH).unwrap_or_else(|m| die(format!("{}: {}", p0, m)));
            }
            Some(format!("payroll steps {} sounds {}", steps.len(), names.join(" ")))
        }));
    }
    // Ops 282..287, the morning stat changes (strategic.md "The experience count"): the lines `xp count K STAT V D
    // shows N clicks C` and `xp end ...`; nothing waits. JA1_XP_ESC=1: Escape is pressed at the first op 282;
    // JA1_XP_PNG = the screen at op 287.
    {
        let (pm, iso, exe, seed) = (pmap.clone(), iso.to_string(), exe.to_string(), e.seed);
        let wp = &e.world as *const ja1front::world::World;
        let png = std::env::var("JA1_XP_PNG").ok();
        let mut frame_png = std::env::var("JA1_XP_FRAME_PNG").ok();
        // JA1_XP_MSG_PNG = the first op 285 message box (a level change).
        let mut msg_png = std::env::var("JA1_XP_MSG_PNG").ok();
        let mut esc_left = std::env::var("JA1_XP_ESC").map_or(false, |v| v == "1");
        let mut xv: Option<ja1front::xpcount::XpView> = None;
        play.host.xp_op = Some(Box::new(move |op: i32, a: &[i32]| {
            // SAFETY: as for payroll_show above.
            let w = unsafe { &*wp };
            let mut pm = pm.borrow_mut();
            let map = pm.get_or_insert_with(|| ja1front::mapui::MapUi::new(&iso, &exe, seed));
            if xv.is_none() {
                let faces = map.faces(w).unwrap_or_else(|c| fail("faces", c));
                let under = map.payroll.clone().unwrap_or_else(|| vec![0u8; 64000]);
                xv = Some(ja1front::xpcount::XpView::new(&iso, &exe, &w.soldiers[..9 * 0x200], &faces, &w.stat_changed,
                                                         &under).unwrap_or_else(|c| fail("xp", c)));
            }
            let v = xv.as_mut().unwrap();
            let mut esc = || std::mem::take(&mut esc_left);
            // JA1_XP_FRAME_PNG = the first screen of the first count that changes a number (the old number framed).
            let mut first = None;
            let l = v.op(&iso, &exe, op, a, &w.soldiers[..9 * 0x200], &mut esc, &mut |o| match o {
                ja1front::xpcount::Out::Screen(sc) => {
                    if first.is_none() {
                        first = Some(sc.to_vec());
                    }
                }
                ja1front::xpcount::Out::Message(sc, _) => {
                    if let Some(p) = msg_png.take() {
                        write_png_wh(&p, sc, &map.pal, SW, SH).unwrap_or_else(|m| die(format!("{}: {}", p, m)));
                    }
                }
                _ => {}
            });
            if let (Some(p), Some(sc), true) = (frame_png.as_ref(), first, op == 283 && a.get(3).map_or(false, |&d| d != 0)) {
                write_png_wh(p, &sc, &map.pal, SW, SH).unwrap_or_else(|m| die(format!("{}: {}", p, m)));
                frame_png = None;
            }
            if op == 287 {
                if let Some(p) = png.as_ref() {
                    write_png_wh(p, &v.screen, &map.pal, SW, SH).unwrap_or_else(|m| die(format!("{}: {}", p, m)));
                }
                xv = None;
            }
            l
        }));
    }
    let (pay_png, ask_png) = (std::env::var("JA1_PAY_PNG").ok(), std::env::var("JA1_PAY_ASK_PNG").ok());
    // JA1_MERC_SAY_PNG: each box of the morning's merc quotes as a picture (NAME.png, then NAME_<n>.png) and a `merc box` line with its text.
    if let Ok(png) = std::env::var("JA1_MERC_SAY_PNG") {
        let pm = pmap.clone();
        let (iso, exe, seed) = (iso.to_string(), exe.to_string(), e.seed);
        let wp = &e.world as *const ja1front::world::World;
        let mut n = 0;
        play.host.merc_say = Some(Box::new(move |s: ja1front::play::MercSay| {
            // SAFETY: the world stays in place during run_script.
            let w = unsafe { &*wp };
            let mut pm = pm.borrow_mut();
            let map = pm.get_or_insert_with(|| ja1front::mapui::MapUi::new(&iso, &exe, seed));
            if map.payroll.is_none() {
                map.redraw(w).unwrap_or_else(|c| fail("map", c));
            }
            let under = map.payday_img(w, None, &Default::default()).unwrap_or_else(|c| fail("map", c));
            let (boxes, nick) = merc_say_texts(&iso, &exe, s, w);
            for (q, text) in boxes {
                eprintln!("merc box {} quote {} {:?}", s.p, q, String::from_utf8_lossy(&text));
                let (v, _) = ja1front::balloon::text_screens(&iso, &exe, s.x0, s.p as u8, &nick, s.sub, text, &under)
                    .unwrap_or_else(|c| fail("speech balloon", c));
                let f = if n == 0 { png.clone() } else { format!("{}_{}.png", png.trim_end_matches(".png"), n) };
                write_png_wh(&f, &v[if s.sub { 0 } else { 4 }], &map.pal, SW, SH).unwrap_or_else(|m| die(format!("{}: {}", f, m)));
                n += 1;
            }
        }));
    }
    let (blink_png, end_png, said_png) = (std::env::var("JA1_PAY_BLINK_PNG").ok(), std::env::var("JA1_PAY_END_PNG").ok(),
                                          std::env::var("JA1_PAY_BALLOON_PNG").ok());
    if blink_png.is_some() || end_png.is_some() || said_png.is_some() {
        let pm = pmap.clone();
        let (iso, exe, seed) = (iso.to_string(), exe.to_string(), e.seed);
        let wp = &e.world as *const ja1front::world::World;
        let (mut end_png, mut said_png) = (end_png, said_png);
        let shot = move |png: &str, view: &ja1front::dayloop::PayView, said: Option<(i32, i32, i32, bool)>| {
            // SAFETY: the world stays in place during run_script; the library writes only through its view.
            let w = unsafe { &*wp };
            let mut pm = pm.borrow_mut();
            let map = pm.get_or_insert_with(|| ja1front::mapui::MapUi::new(&iso, &exe, seed));
            if map.payroll.is_none() {
                map.redraw(w).unwrap_or_else(|c| fail("map", c));
            }
            let mut img = map.payday_img(w, None, view).unwrap_or_else(|c| fail("map", c));
            if let Some((_, x0, p, sub)) = said {
                // The balloon at its full size (frame 4 of the slide), as it shows while the merc speaks.
                let nick = w.profiles.get(p as usize * 0x5A..p as usize * 0x5A + 10).unwrap_or(&[]);
                let (v, _) = ja1front::balloon::pay_screens(&map.iso, &map.exe, x0, p as u8, nick, sub, &img)
                    .unwrap_or_else(|c| fail("speech balloon", c));
                img = v[if sub { 0 } else { 4 }].clone();
            }
            write_png_wh(png, &img, &map.pal, SW, SH).unwrap_or_else(|m| die(format!("{}: {}", png, m)));
        };
        let shot = std::rc::Rc::new(shot);
        let s2 = shot.clone();
        play.host.pay_end_shot = Some(Box::new(move |view: &ja1front::dayloop::PayView, said: Option<(i32, i32, i32, bool)>| {
            let png = if said.is_some() { said_png.take() } else { end_png.take() };
            if let Some(p) = png {
                s2(&p, view, said);
            }
        }));
        if let Some(p) = blink_png {
            let mut p = Some(p);
            let s3 = shot.clone();
            play.host.pay_blink_shot = Some(Box::new(move |view: &ja1front::dayloop::PayView| {
                if view.blink {
                    if let Some(p) = p.take() {
                        s3(&p, view, None);
                    }
                }
            }));
        }
    }
    if pay_png.is_some() || ask_png.is_some() {
        let pm = pmap.clone();
        let (iso, exe, seed) = (iso.to_string(), exe.to_string(), e.seed);
        let wp = &e.world as *const ja1front::world::World;
        let (mut pay_png, mut ask_png) = (pay_png, ask_png);
        play.host.pay_shot = Some(Box::new(move |msgs: &[i32], view: &ja1front::dayloop::PayView, ask: Option<(i32, i32)>| {
            // SAFETY: the world stays in place during run_script; the library writes only through its view.
            let w = unsafe { &*wp };
            let mut pm = pm.borrow_mut();
            let map = pm.get_or_insert_with(|| ja1front::mapui::MapUi::new(&iso, &exe, seed));
            if map.payroll.is_none() {
                // The palette of the island map (the assignment screen's).
                map.redraw(w).unwrap_or_else(|c| fail("map", c));
            }
            let text = ja1front::dayloop::pay_box_text(msgs);
            let (png, img) = match ask {
                None => match pay_png.take() {
                    Some(p) => (p, map.payday_img(w, text.as_deref(), view)),
                    None => return,
                },
                Some((k, family)) => match ask_png.take() {
                    Some(p) => (p, map.payday_img(w, None, view)
                        .and_then(|u| map.payday_ask_img(w, &u, k.clamp(0, 7) as usize, family, None))),
                    None => return,
                },
            };
            let img = img.unwrap_or_else(|c| fail("map", c));
            write_png_wh(&png, &img, &map.pal, SW, SH).unwrap_or_else(|m| die(format!("{}: {}", png, m)));
        }));
    }
    // Op 303 day_ask (panel.md "The travel animation"): JA1_ASK_PNG = the box of the first question with the wait
    // row's frame 1 (frame 0 for kind 0's picture); <JA1_ASK_PNG>_go.png = the box after yes with the go row's last
    // shown frame (kind 1 only).
    if let Ok(png) = std::env::var("JA1_ASK_PNG") {
        let (pm, iso, exe, seed) = (pmap.clone(), iso.to_string(), exe.to_string(), e.seed);
        let wp = &e.world as *const ja1front::world::World;
        let mut png = Some(png);
        play.host.ask_show = Some(Box::new(move |v: [i32; 4]| {
            let Some(png) = png.take() else { return };
            // SAFETY: as pay_shot above.
            let w = unsafe { &*wp };
            let mut pm = pm.borrow_mut();
            let map = pm.get_or_insert_with(|| ja1front::mapui::MapUi::new(&iso, &exe, seed));
            let (text, pic) = ja1front::dayloop::ask_question(v[0]);
            let mut img = map.question_img(w, text, pic, None).unwrap_or_else(|c| fail("map", c));
            let base = map.img.clone();
            if v[0] == 1 {
                let wait = ja1front::sanim::load(&iso, &exe, ja1front::sanim::DAY_ARC, v[1], None).unwrap_or_else(|c| fail("sanim", c));
                let go = ja1front::sanim::load(&iso, &exe, ja1front::sanim::DAY_ARC, v[2], Some(v[3])).unwrap_or_else(|c| fail("sanim", c));
                wait.put(&mut img, 1);
                let mut g = ja1front::question::go_screen(&iso, &exe, &base, &go).unwrap_or_else(|c| fail("sanim", c));
                go.put(&mut g, go.shown().saturating_sub(1));
                let gp = format!("{}_go.png", png.trim_end_matches(".png"));
                write_png_wh(&gp, &g, &map.pal, SW, SH).unwrap_or_else(|m| die(format!("{}: {}", gp, m)));
            }
            write_png_wh(&png, &img, &map.pal, SW, SH).unwrap_or_else(|m| die(format!("{}: {}", png, m)));
        }));
    }
    // Section 9 step 2: JA1_OFFICE_PNG = the office picture after the last script poll.
    if let Ok(png) = std::env::var("JA1_OFFICE_PNG") {
        play.host.office_shot = Some(Box::new(move |img: &[u8], pal: &[u8; 768]| {
            let pal = ja1front::eod::scale_palette(pal);
            write_png_wh(&png, img, &pal, SW, SH).unwrap_or_else(|m| die(format!("{}: {}", png, m)));
        }));
    }
    // JA1_LEDGER_PNG = the team ledger page when a script poll opens it (strategic.md "The team ledger").
    if let Ok(png) = std::env::var("JA1_LEDGER_PNG") {
        play.host.ledger_shot = Some(Box::new(move |img: &[u8], pal: &[u8; 768]| {
            let pal = ja1front::eod::scale_palette(pal);
            write_png_wh(&png, img, &pal, SW, SH).unwrap_or_else(|m| die(format!("{}: {}", png, m)));
        }));
    }
    // JA1_SWITCHBOX_PNG = the switch box after each of its script polls (strategic.md "The switch box").
    if let Ok(png) = std::env::var("JA1_SWITCHBOX_PNG") {
        play.host.switchbox_shot = Some(Box::new(move |img: &[u8], pal: &[u8; 768]| {
            let pal = ja1front::eod::scale_palette(pal);
            write_png_wh(&png, img, &pal, SW, SH).unwrap_or_else(|m| die(format!("{}: {}", png, m)));
        }));
    }
    // JA1_SLOTS_PNG = the slot list after each of its script polls (strategic.md "Save Game and Restore Game").
    if let Ok(png) = std::env::var("JA1_SLOTS_PNG") {
        play.host.slots_shot = Some(Box::new(move |img: &[u8], pal: &[u8; 768]| {
            let pal = ja1front::eod::scale_palette(pal);
            write_png_wh(&png, img, &pal, SW, SH).unwrap_or_else(|m| die(format!("{}: {}", png, m)));
        }));
    }
    // JA1_AIM_PNG = the A.I.M. screen after each of its script polls that does not end it (strategic.md "The A.I.M.
    // screen").
    if let Ok(png) = std::env::var("JA1_AIM_PNG") {
        play.host.aim_shot = Some(Box::new(move |img: &[u8], pal: &[u8; 768]| {
            let pal = ja1front::eod::scale_palette(pal);
            write_png_wh(&png, img, &pal, SW, SH).unwrap_or_else(|m| die(format!("{}: {}", png, m)));
        }));
    }
    // Key I lines: the tactical screen's faces and table 0 (made at the first one), the polls from the script.
    let mut screen: Option<ja1front::screen::Screen> = None;
    let mut inv = |w: &mut ja1front::world::World, play: &mut ja1front::play::Play, polls: &[ja1front::invscreen::InvPoll]| {
        if screen.is_none() {
            screen = Some(ja1front::screen::Screen::new(iso, exe, w).unwrap_or_else(|c| fail("faces", c)));
        }
        let sc = screen.as_mut().unwrap();
        let mut input = ja1front::invscreen::ScriptInput { polls: polls.iter().cloned().collect() };
        let ts = play.day.tileset;
        play.inventory(w, iso, exe, ts, &sc.faces, &mut sc.table0, &mut input);
    };
    let mut r = Ok(());
    let mut printed = 0;
    loop {
        match ja1front::play::run_script_part(&mut e.world, &mut play, &cmds[pos..], &mut inv) {
            Err(m) => {
                r = Err(m);
                break;
            }
            Ok(n) => pos += n,
        }
        // panel.md "Exit to DOS": the program ends (no menu).
        if std::mem::take(&mut play.host.office_quit) {
            break;
        }
        // docs/serve.md: the lines ran out where `--script` could end; the next chunk goes on from here. Its lines
        // are parsed with all lines before them: the commands that ran must stay the same (a `key I` takes the
        // `inv` lines after it).
        if serve && pos == cmds.len() && !play.day.to_menu {
            serve_out(&e.world, &play, &mut printed, out, false);
            if !serve_chunk(&mut text, path) {
                exit(0);
            }
            let next = ja1front::play::parse_script(&text).unwrap_or_else(|m| die(m));
            if next.len() < pos || next[..pos] != cmds[..] {
                die("serve: the new lines change a command that already ran (sync after a `key I` line?)".into());
            }
            cmds = next;
            continue;
        }
        let Some(m) = menu.as_mut().filter(|_| std::mem::take(&mut play.day.to_menu)) else { break };
        // Section 12: Leave; the menu opens again (C = 1) with the row of the last choice.
        m.reopen();
        let (o, n) = menu_stage(m, &cmds[pos..]);
        pos += n;
        if serve && o.is_none() {
            die("serve: sync inside a menu".into());
        }
        play.day.lines.append(&mut m.lines);
        match o {
            Some(ja1front::menu::Outcome::NewGame(d, q)) => {
                ja1front::record::start(exe, "new game");
                // world_load.md section 11: the save name, options, sector order and notes of the game before stay.
                let mut n = ja1front::world::World::new_game_after(iso, exe, d, q, Some(&e.world))
                    .unwrap_or_else(|m| die(format!("world: {}", m)));
                n.replay_carry = e.world.replay_carry;
                e.world = n;
            }
            Some(ja1front::menu::Outcome::Continue) => ja1front::record::start(exe, "continue"),
            _ => break,
        }
        play.left = true;
        play.day.from_office = true;
        play.day.map_open = false;
        play.day.game_over = None;
    }
    if serve {
        if r.is_ok() {
            serve_out(&e.world, &play, &mut printed, out, true);
        }
        r.unwrap_or_else(|m| die(m));
        exit(0);
    }
    for l in &play.day.lines {
        println!("{}", l);
    }
    if let Err(m) = r {
        die(m);
    }
    e.tileset = play.day.tileset;
    let ts = e.tileset;
    // ending.md "Game end": JA1_END_PNG = the last screen of the end show, JA1_GAMEOVER_PNG = the game-over screen.
    for (var, pic) in [("JA1_END_PNG", &play.day.end_show), ("JA1_GAMEOVER_PNG", &play.day.over_screen)] {
        if let (Ok(png), Some((buf, pal))) = (std::env::var(var), pic.as_ref()) {
            write_png_wh(&png, buf, pal, SW, SH).unwrap_or_else(|m| die(format!("{}: {}", png, m)));
        }
    }
    if let (Ok(png), Some((s, pal))) = (std::env::var("JA1_AMAP_PNG"), play.amap_last.as_ref()) {
        write_png_wh(&png, s, pal, SW, SH).unwrap_or_else(|m| die(format!("{}: {}", png, m)));
    }
    // panel.md "The guide's and Brenda's balloons": JA1_BALLOON_PNG = the island map at the wait of the script's last
    // guide or Brenda balloon (when no merc or Santino balloon came after it).
    // Subtitles on: the box of the first part.
    if let (Ok(png), Some((op, a, sub))) = (std::env::var("JA1_BALLOON_PNG"), play.last_guide.as_ref()) {
        let mut map = ja1front::mapui::MapUi::new(iso, exe, e.seed);
        map.redraw(&e.world).unwrap_or_else(|c| fail("map", c));
        let shot = if *sub {
            let parts = ja1front::balloon::npc_sub_screens(iso, exe, *op, a, &map.img, "PANEL_P.PCC");
            parts.unwrap_or_else(|c| fail("speech balloon", c)).swap_remove(0).1
        } else {
            let (v, _) = ja1front::balloon::npc_screens(iso, exe, *op, a, &map.img).unwrap_or_else(|c| fail("speech balloon", c));
            v[4].clone()
        };
        write_png_wh(&png, &shot, &map.pal, SW, SH).unwrap_or_else(|m| die(format!("{}: {}", png, m)));
    }
    if let (Ok(png), Some(f)) = (std::env::var("JA1_INV_PNG"), play.inv_last.as_ref()) {
        let pal = palette(iso, ts as i32).unwrap_or_else(|c| fail("palette", c));
        write_png_wh(&png, f, &pal, SW, SH).unwrap_or_else(|m| die(format!("{}: {}", png, m)));
    }
    // strategic.md "The replay pictures": the middle picture of the last day replay.
    if let (Ok(png), Some((s, pal))) = (std::env::var("JA1_REPLAY_PNG"), play.host.replay_mid.as_ref()) {
        write_png_wh(&png, s, pal, SW, SH).unwrap_or_else(|m| die(format!("{}: {}", png, m)));
    }
    // strategic.md "The day's verdict" step 5: the last replay picture with the rating word.
    if let (Ok(png), Some((s, pal))) = (std::env::var("JA1_RATING_PNG"), play.host.replay_last.as_ref()) {
        write_png_wh(&png, s, pal, SW, SH).unwrap_or_else(|m| die(format!("{}: {}", png, m)));
    }
    if let (Ok(png), Some((s, pal))) = (std::env::var("JA1_EOD_PNG"), play.host.eod.as_ref().and_then(|x| x.last.as_ref())) {
        write_png_wh(&png, s, pal, SW, SH).unwrap_or_else(|m| die(format!("{}: {}", png, m)));
    }
    if let (Ok(png), Some((s, pal))) = (std::env::var("JA1_SUNRISE_PNG"), play.host.sunrise.as_ref().and_then(|x| x.last.as_ref())) {
        write_png_wh(&png, s, pal, SW, SH).unwrap_or_else(|m| die(format!("{}: {}", png, m)));
    }
    // sound.md "Output": JA1_WAV = the mix of the whole script run.
    if let (Ok(wp), Some(pcm)) = (std::env::var("JA1_WAV"), play.host.sound.wav.as_ref()) {
        std::fs::write(&wp, ja1front::sound::wav_bytes(pcm)).unwrap_or_else(|m| die(format!("{}: {}", wp, m)));
    }
    std::fs::write(out, e.world.image()).unwrap_or_else(|m| die(format!("{}: {}", out, m)));
    play.host.ev.flush();
    print!("{}", ja1front::play::script_report(&e.world, &play));
    // JA1_SCREEN_PNG (section 8 "Play script"): the tactical screen of the last pass, drawn from a copy of the world
    // as the last pass left it (after a leave: the world before the leaving pass).
    if let Ok(png) = std::env::var("JA1_SCREEN_PNG") {
        let (mut wc, ts) = match (play.left, play.frame_world.take()) {
            (true, Some((w, t))) => (*w, t),
            _ => (e.world.clone(), e.tileset),
        };
        // SPEC GAP: the faces come from the copy (a merc who died in the sector shows the dead face, not the last
        // frame of the death morph the window shows).
        let mut sc = ja1front::screen::Screen::new(iso, exe, &mut wc).unwrap_or_else(|c| fail("faces", c));
        let (c0, r0) = ja1front::enter::view_tile(&wc);
        // The script's cursor state (section 8 "Hover"); check aid: JA1_CURSOR=TILE puts the mouse over that map
        // tile (the tile cursor, cursor.md "Drawing").
        sc.cursor = play.cursor;
        sc.cont = play.cont;
        if let (Some(v), Some(d)) = (play.drag_value, play.drag) {
            sc.cursor.value = v;
            sc.cursor.tile = d.loop_.cursor_tile;
        }
        if let Ok(t) = std::env::var("JA1_CURSOR") {
            sc.cursor.over_map = 1;
            sc.cursor.tile = t.parse().unwrap_or_else(|_| usage());
        }
        let (cols, rows, _, _) = ja1front::viewmode::world_window(&wc);
        let ovc = play.ov_cursor(&wc);
        ja1front::enter::view_faces(&sc.faces);
        let view = ja1front::enter::render_world_cursor(iso, exe, &mut wc, ts, c0, r0, cols, rows, Some(&mut sc.cursor), play.host.banner.map(|b| (b.text, b.colour)),
                                                             Some(ovc))
            .unwrap_or_else(|c| fail("render", c));
        let mut blink = play.inv_blink;
        let mut idx = sc.draw(iso, exe, &mut wc, ts, &view, cols * 16, rows * 14, &mut blink, true).unwrap_or_else(|c| fail("panel", c));
        // #20 B: the held item at the mouse (the last strip click's mouse_to, else the script's mouse)
        let (mx, my) = play.host.mouse_to.unwrap_or(play.mouse_px);
        if ja1front::pointer::held_on_screen(iso, exe, &wc, &mut idx, mx, my).unwrap_or_else(|c| fail("pointer", c)) {
            eprintln!("screen held item {} at {} {}", wc.held[0], mx, my);
        }
        // tactical_map.md "Soldier texts": the hit numbers on the screen (record +0x1D2 set, 5 text bytes per slot).
        for k in 0..48usize {
            let r = &wc.soldiers[k * 512..(k + 1) * 512];
            if r[0x1D2] != 0 && r[0x138] != 0 {
                let t: Vec<u8> = wc.dmg_text.get(5 * k..5 * k + 5).unwrap_or(&[]).iter().copied().take_while(|&b| b != 0).collect();
                let s16 = |o: usize| i16::from_le_bytes([r[o], r[o + 1]]);
                eprintln!("screen hit number {} {:?} at {} {}", k, String::from_utf8_lossy(&t), s16(0xD8), s16(0xDA));
            }
            // DrawProjectilePass? 0x17548: a projectile in flight (+0x1C7) and its shadow sprite +0x1C5.
            if (k < 16 || k >= 40) && r[0x1C7] != 0 {
                eprintln!("screen projectile {} sprite {} at {} {}", k, r[0x1C5], i16::from_le_bytes([r[0xF6], r[0xF7]]), i16::from_le_bytes([r[0x9E], r[0x9F]]));
            }
        }
        // The effect records in use: count, kind (0 = a bullet puff / splash, its look's frame) and look.
        for k in 1..16usize {
            let e = &wc.fx_anims[k * 0x12..(k + 1) * 0x12];
            if e[0] != 0 {
                eprintln!("screen effect {} count {} kind {} look {} at {} {}", k, e[0] as i8, e[0xB], e[0xC],
                          i16::from_le_bytes([e[5], e[6]]), i16::from_le_bytes([e[7], e[8]]));
            }
        }
        let mut pal = palette(iso, ts as i32).unwrap_or_else(|c| fail("palette", c));
        // Op 206 pal_cycle (Pal Cycling): the palette after every rotate of the script.
        if !play.host.pal_rot.is_empty() {
            eprintln!("screen pal cycles {}", play.host.pal_rot.len());
        }
        for (f, l) in play.host.pal_rot.drain(..) {
            ja1front::eod::rotate(&mut pal, f, l);
        }
        write_png_wh(&png, &idx, &pal, SW, SH).unwrap_or_else(|m| die(format!("{}: {}", png, m)));
        // JA1_MSG_PNG (with JA1_SCREEN_PNG): the same screen with the script's last message box on it.
        if let (Ok(mp), Some(t)) = (std::env::var("JA1_MSG_PNG"), play.last_box.as_deref()) {
            let mut boxed = idx.clone();
            ja1front::msgbox::draw_tactical(iso, exe, ts, &ja1front::msgbox::game_bytes(t), &mut boxed).unwrap_or_else(|c| fail("message box", c));
            write_png_wh(&mp, &boxed, &pal, SW, SH).unwrap_or_else(|m| die(format!("{}: {}", mp, m)));
        }
        // JA1_PAGES_PNG (with JA1_SCREEN_PNG): the script's last debug pages over the same screen, one PNG per page
        // (page 1 = JA1_PAGES_PNG=NAME.png, pages 2..4 = NAME_2.png .. NAME_4.png).
        if let (Ok(pp), Some(k)) = (std::env::var("JA1_PAGES_PNG"), play.last_pages) {
            for (n, page) in ja1front::debugpage::pages_of(&wc, k).iter().enumerate() {
                let mut img = idx.clone();
                ja1front::debugpage::draw_on(iso, exe, page, &mut img).unwrap_or_else(|c| fail("debug page", c));
                let f = if n == 0 { pp.clone() } else { format!("{}_{}.png", pp.trim_end_matches(".png"), n + 1) };
                write_png_wh(&f, &img, &pal, SW, SH).unwrap_or_else(|m| die(format!("{}: {}", f, m)));
            }
        }
        // JA1_AGAIN_PNG (with JA1_SCREEN_PNG): the same screen with the box of the script's last key L.
        if let (Ok(ap), true) = (std::env::var("JA1_AGAIN_PNG"), play.last_again) {
            if let Some((img, _)) = ja1front::balloon::say_again(iso, exe, &wc, "PANEL_P.PCC", &idx).unwrap_or_else(|c| fail("say again", c)) {
                write_png_wh(&ap, &img, &pal, SW, SH).unwrap_or_else(|m| die(format!("{}: {}", ap, m)));
            }
        }
        // JA1_POPUP_PNG (with JA1_SCREEN_PNG): the same screen at the end wait of the script's last tile popup.
        if let (Ok(pp), Some(p)) = (std::env::var("JA1_POPUP_PNG"), play.last_popup) {
            sc.cursor.over_map = 0;
            ja1front::enter::view_faces(&sc.faces);
            let view = ja1front::enter::render_world_cursor(iso, exe, &mut wc, ts, c0, r0, cols, rows, Some(&mut sc.cursor), play.host.banner.map(|b| (b.text, b.colour)),
                                                                 Some(ovc))
                .unwrap_or_else(|c| fail("render", c));
            let mut blink = play.inv_blink;
            let clean = sc.draw(iso, exe, &mut wc, ts, &view, cols * 16, rows * 14, &mut blink, true).unwrap_or_else(|c| fail("panel", c));
            let (v, wait) = popup_on(iso, exe, &idx, &clean, &p);
            write_png_wh(&pp, &v[wait], &pal, SW, SH).unwrap_or_else(|m| die(format!("{}: {}", pp, m)));
        }
        // JA1_STATS_PNG (with JA1_SCREEN_PNG): the same screen with the stat popups of a last `statkey` line on it.
        if let (Ok(sp), Some(ks)) = (std::env::var("JA1_STATS_PNG"), play.stat_shown.as_ref()) {
            let mut img = idx.clone();
            for &k in ks {
                sc.popup(iso, exe, &wc, k, &mut img).unwrap_or_else(|c| fail("popup", c));
            }
            write_png_wh(&sp, &img, &pal, SW, SH).unwrap_or_else(|m| die(format!("{}: {}", sp, m)));
        }
        // JA1_BATTLE_PNG (with JA1_SCREEN_PNG): the same screen with the script's last battle box on it.
        if let (Ok(bp), Some((s, n, k))) = (std::env::var("JA1_BATTLE_PNG"), play.last_battle) {
            let mut boxed = idx.clone();
            ja1front::battlebox::draw_tactical(iso, exe, ts, s, n, k, &mut boxed).unwrap_or_else(|c| fail("battle box", c));
            // After the Leave path's fade out the palette is black (strategic.md "The battle show").
            let black = [0u8; 768];
            write_png_wh(&bp, &boxed, if play.last_battle_dark { &black } else { &pal }, SW, SH).unwrap_or_else(|m| die(format!("{}: {}", bp, m)));
        }
        // JA1_PANEL_PNG (with JA1_SCREEN_PNG): the same screen with the script's last Control Panel as its `opt`
        // lines left it (panel.md "The Control Panel").
        if let (Ok(cp), Some(log)) = (std::env::var("JA1_PANEL_PNG"), play.ctl_log.as_ref()) {
            let (c, d) = (ja1front::common(iso).unwrap_or_else(|c| fail("palette", c)),
                          ja1front::panel::panel_data(iso, exe).unwrap_or_else(|c| fail("panel", c)));
            // Opening step 2: the panel lies over the dark view.
            ja1front::enter::darken_next();
            ja1front::enter::view_faces(&sc.faces);
            let dv = ja1front::enter::render_world_cursor(iso, exe, &mut wc, ts, c0, r0, cols, rows, Some(&mut sc.cursor),
                                                          play.host.banner.map(|b| (b.text, b.colour)), Some(ovc))
                .unwrap_or_else(|c| fail("render", c));
            let mut blink = play.inv_blink;
            let under = sc.draw(iso, exe, &mut wc, ts, &dv, cols * 16, rows * 14, &mut blink, true).unwrap_or_else(|c| fail("panel", c));
            let mut img = under.clone();
            let mut table0 = c.remap[0];
            let mut p = ja1front::panel::Painter { d: &d, buf: &mut img, table0: &mut table0, table29: ja1front::panel::table29(&c) };
            ja1front::ctlpanel::paint(&mut p, &under, log);
            write_png_wh(&cp, &img, &pal, SW, SH).unwrap_or_else(|m| die(format!("{}: {}", cp, m)));
        }
        // JA1_DISARM_PNG (with JA1_SCREEN_PNG): the same screen with the script's last op 345 disarm_ask question
        // (the merc-face question box, PANEL_P.PCC).
        if let (Ok(dp), Some((t, p, n))) = (std::env::var("JA1_DISARM_PNG"), play.host.disarm_last.as_ref()) {
            let v = ja1front::question::face_screens(iso, exe, t, *p, n, "PANEL_P.PCC", &idx)
                .unwrap_or_else(|c| fail("question box", c));
            write_png_wh(&dp, &v[0], &pal, SW, SH).unwrap_or_else(|m| die(format!("{}: {}", dp, m)));
        }
        // JA1_DOOR_PNG (with JA1_SCREEN_PNG): the same screen with every frame of the script's last door film drawn
        // at its place (the film's last frame).
        if let (Ok(dp), Some((_, x, y, data))) = (std::env::var("JA1_DOOR_PNG"), play.last_door_film.as_ref()) {
            let mut img = idx.clone();
            if let Some(mut f) = ja1front::flc::Flc::new(data) {
                while f.next_frame(&mut img, *x, *y) {}
            }
            write_png_wh(&dp, &img, &pal, SW, SH).unwrap_or_else(|m| die(format!("{}: {}", dp, m)));
        }
        // JA1_ANIMPOP_PNG (with JA1_SCREEN_PNG): the same screen with the last frame of the script's last drown /
        // injection popup on it.
        if let (Ok(ap), Some((slot, flag, selected))) = (std::env::var("JA1_ANIMPOP_PNG"), play.last_anim_popup) {
            if let Ok(f) = ja1front::popup::frames(exe, slot, selected, flag) {
                let mut img = idx.clone();
                if let Some(fr) = f.last() {
                    ja1front::popup::blit(fr, &mut img);
                }
                write_png_wh(&ap, &img, &pal, SW, SH).unwrap_or_else(|m| die(format!("{}: {}", ap, m)));
            }
        }
        // JA1_BALLOON_PNG (with JA1_SCREEN_PNG): the same screen at the wait of the script's last speech balloon.
        if let (Ok(bp), true) = (std::env::var("JA1_BALLOON_PNG"), play.last_santino) {
            let (v, _) = ja1front::balloon::santino_screens(iso, exe, &ja1front::balloon::santino_text(iso, exe, 0), &idx)
                .unwrap_or_else(|c| fail("speech balloon", c));
            write_png_wh(&bp, &v[4], &pal, SW, SH).unwrap_or_else(|m| die(format!("{}: {}", bp, m)));
        } else if let (Ok(bp), Some((rec, q, t, nat))) = (std::env::var("JA1_BALLOON_PNG"), play.last_balloon.as_ref()) {
            let (v, _) = ja1front::balloon::reaction_screens(iso, exe, rec, *q, *nat, ja1front::ctlpanel::sub_byte(wc.update.opt_step_text), &idx)
                .unwrap_or_else(|c| fail("speech balloon", c));
            write_png_wh(&bp, &v[4], &pal, SW, SH).unwrap_or_else(|m| die(format!("{}: {}", bp, m)));
            // PNG_text.png: the same wait with the mouse on the face (panel.md "Speech balloon" step 7: the weapon
            // text over the face) when the merc message has one.
            if let (false, Some(spot)) = (t.is_empty(), ja1front::balloon::last_face()) {
                let mut img = v[4].clone();
                ja1front::balloon::wait_text(iso, exe, &mut img, &spot, t.as_bytes()).unwrap_or_else(|c| fail("speech balloon", c));
                let tp = format!("{}_text.png", bp.trim_end_matches(".png"));
                write_png_wh(&tp, &img, &pal, SW, SH).unwrap_or_else(|m| die(format!("{}: {}", tp, m)));
            }
        }
    }
    // JA1_CORPSE_PNG: the script's last op 292 corpse_ask question over the island map as the script left it.
    // With the wait row's frame 0 in the box; PNG_go.png = the box of the answer's row (its last shown frame).
    if let (Ok(png), Some((cost, can, yes))) = (std::env::var("JA1_CORPSE_PNG"), play.host.corpse_last) {
        let mut map = ja1front::mapui::MapUi::new(iso, exe, e.seed);
        map.redraw(&e.world).unwrap_or_else(|c| fail("map", c));
        let mut v = ja1front::question::screens(iso, exe, &ja1front::dayloop::corpse_text(cost), None, &map.img)
            .unwrap_or_else(|c| fail("question box", c));
        let (wr, yr, nr) = ja1front::dayloop::corpse_rows(can);
        let arc = ja1front::dayloop::CORPSE_ARC;
        let wait = ja1front::sanim::load(iso, exe, arc, wr, None).unwrap_or_else(|c| fail("sanim", c));
        wait.put(&mut v[0], 0);
        write_png_wh(&png, &v[0], &map.pal, SW, SH).unwrap_or_else(|m| die(format!("{}: {}", png, m)));
        let go = ja1front::sanim::load(iso, exe, arc, if yes { yr } else { nr }, None).unwrap_or_else(|c| fail("sanim", c));
        let mut g = ja1front::question::go_screen(iso, exe, &map.img, &go).unwrap_or_else(|c| fail("sanim", c));
        go.put(&mut g, go.shown().saturating_sub(1));
        let gp = format!("{}_go.png", png.trim_end_matches(".png"));
        write_png_wh(&gp, &g, &map.pal, SW, SH).unwrap_or_else(|m| die(format!("{}: {}", gp, m)));
    }
    // JA1_RAISE_PNG: the script's last op 297 salary box over the island map as the script left it, after every
    // press of its line that did not end the box.
    if let (Ok(png), Some((prof, nick, start, keys))) = (std::env::var("JA1_RAISE_PNG"), play.host.raise_last.as_ref()) {
        let mut map = ja1front::mapui::MapUi::new(iso, exe, e.seed);
        map.redraw(&e.world).unwrap_or_else(|c| fail("map", c));
        let mut b = ja1front::salary::Screen::new(iso, exe, *prof, nick, *start, &map.img)
            .unwrap_or_else(|c| fail("salary box", c));
        for &k in keys {
            let mut o = b.offer();
            if matches!(o.press(k), ja1front::salary::Press::End(_)) {
                break;
            }
            b.press(k);
        }
        write_png_wh(&png, &b.img, &map.pal, SW, SH).unwrap_or_else(|m| die(format!("{}: {}", png, m)));
    }
    // Debug picture of the end state: JA1_SCRIPT_PNG=OUT [JA1_VIEW="COL ROW COLS ROWS"] (default: the whole map).
    if let Ok(png) = std::env::var("JA1_SCRIPT_PNG") {
        let v: Vec<i32> = std::env::var("JA1_VIEW").unwrap_or("0 0 56 56".into()).split_whitespace()
            .map(parse_i32).collect();
        if v.len() != 4 {
            usage();
        }
        let ts = e.tileset;
        let pal = palette(iso, ts as i32).unwrap_or_else(|c| fail("palette", c));
        let idx = ja1front::enter::render_world(iso, exe, &mut e.world, ts, v[0], v[1], v[2], v[3])
            .unwrap_or_else(|c| fail("render", c));
        write_png_wh(&png, &idx, &pal, 16 * v[2] as u32, 14 * v[3] as u32).unwrap_or_else(|m| die(format!("{}: {}", png, m)));
    }
    exit(0)
}

/// The tile under view pixel (vx, vy): 16 x 14 px tiles from view column / row (c0, r0); in mode 0 the overview's
/// 4 x 3 px tiles of the whole map (panel.md "The overview mouse step").
fn map_tile(w: &ja1front::world::World, c0: i32, r0: i32, vx: i32, vy: i32) -> i32 {
    if w.opt_onscreen == 0 {
        ja1front::viewmode::overview_tile(vx + 48, vy + 14)
    } else {
        ja1front::viewmode::tactical_tile(c0, r0, vx, vy)
    }
}

fn main() {
    let mut pos: Vec<String> = Vec::new();
    let mut words = 0;
    let mut png_out: Option<String> = None;
    let mut world_args: Option<(String, String)> = None;
    let mut copy_args: Option<(String, String)> = None;
    let mut new_args: Option<(u8, u8, String)> = None;
    let mut enter_args: Option<(String, u32, String)> = None;
    let mut play_args: Option<(String, u32)> = None;
    let mut walk_args: Option<(String, u32, i32, i32, u32, String)> = None;
    let mut script_args: Option<(String, u32, String, String, bool)> = None;
    let mut menu_dir: Option<Option<String>> = None;
    // config.rs: `ISO EXE --setup` = the settings screen; every start reads ja1.cfg (an environment variable wins).
    let mut argv: Vec<String> = std::env::args().skip(1).collect();
    // No game paths (a double-click, `ja1-port --setup` or `ja1-port --menu`): find the game folder, work in the
    // program's folder. Insert #13506: a plain start (no arguments) is the game: title films, then the main menu
    // (`--menu saves`). `ja1-port --setup` opens the setup screen.
    if argv.first().map_or(true, |a| a.starts_with("--")) {
        let (iso, exe) = find_game().unwrap_or_else(|m| no_game(m));
        if let Some(d) = std::env::current_exe().ok().and_then(|p| p.parent().map(|p| p.to_path_buf())) {
            let _ = std::env::set_current_dir(&d);
        }
        let _ = std::fs::create_dir_all("saves");
        if argv.is_empty() {
            argv.extend(["--menu".into(), "saves".into()]);
        }
        argv.splice(0..0, [iso, exe]);
    }
    if argv.iter().any(|a| a == "--setup") {
        let p: Vec<&String> = argv.iter().filter(|a| *a != "--setup").collect();
        if p.len() != 2 {
            usage();
        }
        setup(p[0], p[1]);
    }
    ja1front::config::load_into_env();
    // #71: fixes (ja1.cfg) / JA1_FIXES = off (1.13b) | 1.15m (the #70 fan fixes) | all (+ the port fixes); old
    // fan_fixes = 1 reads as 1.15m. JA1_FAN_FIXES (F ids, tests) replaces the F part when set.
    let fixes = ja1front::config::read::fixes(std::env::var("JA1_FIXES").ok().as_deref());
    let (mut f, p) = ja1logic::fanfix::parse_fixes(&fixes).unwrap_or_else(|e| die(e));
    if let Ok(v) = std::env::var("JA1_FAN_FIXES") {
        f = ja1logic::fanfix::parse(&v).unwrap_or_else(|e| die(e));
    }
    ja1logic::fanfix::set(f);
    ja1logic::fanfix::port_set(p);
    ja1front::sound::start_volumes();
    if f != 0 || std::env::var("JA1_FAN_FIXES").is_ok_and(|v| v == "1" || v.eq_ignore_ascii_case("on")) {
        eprintln!("fan fixes on (mask {:#x})", f);
    }
    if p != 0 {
        eprintln!("port fixes on (mask {:#x})", p);
    }
    let mut args = argv.into_iter();
    while let Some(a) = args.next() {
        if a == "--png" {
            png_out = Some(args.next().unwrap_or_else(|| usage()));
        } else if a == "--world" {
            let save = args.next().unwrap_or_else(|| usage());
            let out = args.next().unwrap_or_else(|| usage());
            world_args = Some((save, out));
        } else if a == "--new-game" {
            let d = args.next().and_then(|v| v.parse().ok()).unwrap_or_else(|| usage());
            let q = args.next().and_then(|v| v.parse().ok()).unwrap_or_else(|| usage());
            let out = args.next().unwrap_or_else(|| usage());
            new_args = Some((d, q, out));
        } else if a == "--save-copy" {
            let save = args.next().unwrap_or_else(|| usage());
            let out = args.next().unwrap_or_else(|| usage());
            copy_args = Some((save, out));
        } else if a == "--enter" {
            let save = args.next().unwrap_or_else(|| usage());
            let seed = parse_seed(&args.next().unwrap_or_else(|| usage()));
            let out = args.next().unwrap_or_else(|| usage());
            enter_args = Some((save, seed, out));
        } else if a == "--walk" {
            let mut next = || args.next().unwrap_or_else(|| usage());
            let save = next();
            let seed = parse_seed(&next());
            let slot = parse_i32(&next());
            let tile = parse_i32(&next());
            let ticks = parse_seed(&next());
            let out = next();
            walk_args = Some((save, seed, slot, tile, ticks, out));
        } else if a == "--sab" {
            // `--sab ARCHIVE MEMBER` (ending.md): play one SAB script and print the test log of sab_diff.py.
            let arc = args.next().unwrap_or_else(|| usage());
            let mem = args.next().unwrap_or_else(|| usage());
            if pos.len() != 2 {
                usage();
            }
            let (iso, exe): (String, String) = (pos[0].clone(), pos[1].clone());
            let dir = std::path::Path::new(&exe).parent().map(|p| p.to_path_buf()).unwrap_or_default();
            let subtitles = std::env::var("JA1_SAB_SUBTITLES").is_ok_and(|v| v == "1");
            let (i2, e2) = (iso.clone(), exe.clone());
            let draw: Box<dyn FnMut(&mut [u8], &[u8], u8, u8)> = Box::new(move |scr, rec, fg, sh| {
                let (Ok(c), Ok(d)) = (ja1front::common(&i2), ja1front::panel::panel_data(&i2, &e2)) else { return };
                let mut t0 = c.remap[0];
                let mut p = ja1front::panel::Painter { d: &d, buf: scr, table0: &mut t0, table29: ja1front::panel::table29(&c) };
                ja1front::sab::draw_subtitle(&mut p, rec, fg, sh);
            });
            let mut io = ja1front::sab::LogIo { dir, iso, lines: Vec::new(), subtitles, draw: Some(draw), bright: None, films: false };
            let mut s = ja1front::sab::Sab::new();
            if let Ok(p) = std::env::var("JA1_SAB_DAC") {
                if let Ok(d) = std::fs::read(&p) {
                    s.set_dac_copy(&d);
                }
            }
            let r = s.play_script(&mut io, &arc, &mem);
            if std::env::var("JA1_SAB_DAC").is_ok() {
                io.lines.push(format!("dac {:08x}", ja1front::sab::crc32(0, &s.dac)));
            }
            io.lines.push(format!("end {}", r));
            println!("{}", io.lines.join("\n"));
            exit(0);
        } else if a == "--effect-slot" {
            // `--effect-slot CASES` (sound.md op 65 effect_slot): per case line `K C TIMER B16C HP ENERGY B1E0` (the
            // fields of record K) print `sfx EFFECT RATE VOL` or `none`, then `timer T` (u32 +0xC0 of record K).
            let path = args.next().unwrap_or_else(|| usage());
            let text = std::fs::read_to_string(&path).unwrap_or_else(|m| die(format!("{}: {}", path, m)));
            for l in text.lines() {
                let v: Vec<i64> = l.split_whitespace().map(|x| x.parse().unwrap_or(0)).collect();
                let mut sol = vec![0u8; 48 * 512];
                let r = (v[0] as usize) * 512;
                sol[r + 0xC0..r + 0xC4].copy_from_slice(&(v[2] as u32).to_le_bytes());
                sol[r + 0x16C] = v[3] as u8;
                sol[r + 0x183] = v[4] as u8;
                sol[r + 0x18A] = v[5] as u8;
                sol[r + 0x1E0] = v[6] as u8;
                match ja1front::sound::effect_slot(&mut sol, v[0] as i32, v[1] as i32) {
                    Some((e, rate, vol)) => println!("sfx {} {} {}", e, rate, vol),
                    None => println!("none"),
                }
                println!("timer {}", u32::from_le_bytes(sol[r + 0xC0..r + 0xC4].try_into().unwrap()) as i32);
            }
            return;
        } else if a == "--face-anim" {
            // `--face-anim CASES` (panel.md "Face animation"): per line `eye E D B V..` / `mouth M S V..` (V = the
            // random numbers in order; R(n) = V mod n) print the new state and the count of numbers used;
            // `face E M HEX` (the 148 x 69 sheet) prints the 56 x 50 face in hex; `scan K HEX` (the samples) prints
            // the silences and the ticks (of K) at which the lip-sync clock changes; `parts S E HEX` prints the
            // silences of the lip view of a voice whose cry sample part is bytes S..E.
            let path = args.next().unwrap_or_else(|| usage());
            let text = std::fs::read_to_string(&path).unwrap_or_else(|m| die(format!("{}: {}", path, m)));
            let hex = |h: &str| (0..h.len() / 2).map(|i| u8::from_str_radix(&h[2 * i..2 * i + 2], 16).unwrap_or(0)).collect::<Vec<u8>>();
            for l in text.lines() {
                let w: Vec<&str> = l.split_whitespace().collect();
                let num = |i: usize| w[i].parse::<i64>().unwrap_or(0);
                match w[0] {
                    "eye" | "mouth" => {
                        let first = if w[0] == "eye" { 4 } else { 3 };
                        let vals: Vec<i64> = (first..w.len()).map(|i| num(i)).collect();
                        let mut k = 0usize;
                        let mut r = |n: i32| {
                            let v = vals.get(k).copied().unwrap_or(0);
                            k += 1;
                            (v.rem_euclid(n.max(1) as i64)) as i32
                        };
                        let out = if w[0] == "eye" {
                            ja1front::faceanim::eye_step(num(1) as i32, num(2) != 0, num(3) as i32, &mut r)
                        } else {
                            ja1front::faceanim::mouth_step(num(1) as i32, num(2) != 0, &mut r)
                        };
                        println!("{} {} {}", w[0], out, k);
                    }
                    "face" => {
                        let f = ja1front::faceanim::face(&hex(w[3]), num(1) as i32, num(2) as i32);
                        println!("face {}", f.iter().map(|b| format!("{:02x}", b)).collect::<String>());
                    }
                    "scan" => {
                        let sil = ja1front::faceanim::silences(&hex(w[2]));
                        let mut c = ja1front::faceanim::LipClock::default();
                        let mut ch = Vec::new();
                        for t in 0..num(1) {
                            let was = c.silent;
                            c.tick(&sil);
                            if c.silent != was {
                                ch.push(t.to_string());
                            }
                        }
                        let s: Vec<String> = sil.iter().map(|(a, b)| format!("{},{}", a, b)).collect();
                        println!("scan {} | {}", s.join(" "), ch.join(" "));
                    }
                    "parts" => {
                        // `parts S E HEX`: a quote voice with the cry sample part at bytes S..E (PlayQuote modes 1..3):
                        // the silences of the port's lip view, in ms from the voice start.
                        let v = ja1front::sound::lip_view(&hex(w[3]), num(1) as usize..num(2) as usize);
                        let s: Vec<String> = ja1front::faceanim::silences(&v).iter().map(|(a, b)| format!("{},{}", a, b)).collect();
                        println!("parts {}", s.join(" "));
                    }
                    _ => println!("?"),
                }
            }
            return;
        } else if a == "--film-cue" {
            // `--film-cue CASES` (ending.md "Film cues"): per case line `RATE VOL HEX320 C,C,..` (C = -1: the cue
            // index goes back to 0, as after a ring frame) print the cue steps for filmcue_diff.py.
            let path = args.next().unwrap_or_else(|| usage());
            if pos.len() != 2 {
                usage();
            }
            let (iso, exe): (String, String) = (pos[0].clone(), pos[1].clone());
            let dir = std::path::Path::new(&exe).parent().map(|p| p.to_path_buf()).unwrap_or_default();
            let text = std::fs::read_to_string(&path).unwrap_or_else(|m| die(format!("{}: {}", path, m)));
            let mut io = ja1front::sab::LogIo { dir, iso, lines: Vec::new(), subtitles: false, draw: None, bright: None, films: false };
            let snd = ja1front::sab::SabIo::file(&mut io, "INTRO\\INTROSP");
            io.lines.clear();
            for l in text.lines() {
                let w: Vec<&str> = l.split_whitespace().collect();
                let mut s = ja1front::sab::Sab::new();
                s.set_sound_archive(snd.clone());
                s.cue_rate = w[0].parse().unwrap_or(0);
                s.cue_vol = w[1].parse().unwrap_or(0);
                for (k, b) in s.cues.iter_mut().enumerate() {
                    *b = u8::from_str_radix(&w[2][2 * k..2 * k + 2], 16).unwrap_or(0);
                }
                for c in w[3].split(',').map(|c| c.parse::<i32>().unwrap_or(0)) {
                    if c == -1 {
                        s.cue_i = 0;
                        continue;
                    }
                    let cue = s.film_cue(&mut io, c);
                    let what = match &cue {
                        ja1front::sab::Cue::Clear => "clear 48636 0 320 35 320".to_string(),
                        ja1front::sab::Cue::Voice(n, _) => format!("voice {}", n),
                        ja1front::sab::Cue::Subtitle(q, fg) => format!("subtitle {} {} 0", q, fg),
                        _ => String::new(),
                    };
                    if !what.is_empty() {
                        io.lines.insert(io.lines.len() - matches!(cue, ja1front::sab::Cue::Voice(_, true)) as usize, what);
                    }
                    io.lines.push(format!("cue {} {} {}", c, s.cue_i, s.cue_j));
                }
                io.lines.push("case".into());
            }
            println!("{}", io.lines.join("\n"));
            exit(0);
        } else if a == "--credits" {
            // `--credits` (ending.md "The credits (after the win)"): print the test log of credits_diff.py.
            if pos.len() != 2 {
                usage();
            }
            let (iso, exe): (String, String) = (pos[0].clone(), pos[1].clone());
            let dir = std::path::Path::new(&exe).parent().map(|p| p.to_path_buf()).unwrap_or_default();
            let mut io = ja1front::sab::LogIo { dir, iso: iso.clone(), lines: Vec::new(), subtitles: false, draw: None, bright: None, films: false };
            let mut s = ja1front::sab::Sab::new();
            let stop = ja1front::ending::credits(&mut io, &mut s, false, &mut ja1front::ending::credits_draw(&iso, &exe));
            io.lines.push(format!("end {}", stop as i32));
            println!("{}", io.lines.join("\n"));
            exit(0);
        } else if a == "--title" {
            // `--title` (ending.md "The title part (program start)"): print the test log of credits_diff.py --title.
            if pos.len() != 2 {
                usage();
            }
            let (iso, exe): (String, String) = (pos[0].clone(), pos[1].clone());
            let dir = std::path::Path::new(&exe).parent().map(|p| p.to_path_buf()).unwrap_or_default();
            let mut io = ja1front::sab::LogIo { dir, iso: iso.clone(), lines: Vec::new(), subtitles: false, draw: None, bright: None, films: false };
            let mut s = ja1front::sab::Sab::new();
            let stop = ja1front::ending::title(&mut io, &mut s, &mut ja1front::ending::credits_draw(&iso, &exe));
            io.lines.push(format!("end {}", stop as i32));
            io.lines.push(format!("cue_state {} {} {:08x}", s.cue_rate, s.cue_vol, ja1front::sab::crc32(0, &s.cues[..200])));
            println!("{}", io.lines.join("\n"));
            exit(0);
        } else if a == "--menu" {
            // `--menu [SAVE_DIR]` (world_load.md section 12): the window starts at the main menu.
            menu_dir = Some(None);
        } else if a == "--script" || a == "--serve" {
            let mut next = || args.next().unwrap_or_else(|| usage());
            let save = next();
            let seed = parse_seed(&next());
            let path = next();
            let out = next();
            script_args = Some((save, seed, path, out, a == "--serve"));
        } else if a == "--play" {
            let save = args.next().unwrap_or_else(|| usage());
            let mut rest: Vec<String> = args.by_ref().collect();
            // No seed on the command line: the clock, as main_ 0x10150 (SrandTime? 0x53564).
            let seed = if rest.first().is_some_and(|s| !s.starts_with("--")) {
                parse_seed(&rest.remove(0))
            } else {
                ja1front::world::start_seed()
            };
            let mut it = rest.into_iter();
            while let Some(b) = it.next() {
                if b == "--png" {
                    png_out = Some(it.next().unwrap_or_else(|| usage()));
                } else {
                    usage();
                }
            }
            play_args = Some((save, seed));
        } else {
            // main_ 0x10150: the first 3 words go to ParseCmdArg 0x1008C. The port's own arguments are paths: a word
            // is a known word, or letters only that name no file behind the two paths.
            let w = ja1front::world::cmd_word(a.as_bytes());
            let unknown = matches!(w, ja1front::world::CmdWord::Unknown(_));
            let letters = a.bytes().all(|c| c.is_ascii_alphabetic());
            if takes_cmd_word(unknown, pos.len(), letters, || std::path::Path::new(&a).exists()) {
                if ja1front::world::first_word(&mut words) {
                    ja1front::world::run_cmd_word(&w);
                }
            } else {
                pos.push(a);
            }
        }
    }
    // world_load.md "The program option allow save": the notice at program start (the front-end has no text
    // screen and does not wait for a key).
    if ja1front::world::allow_save() {
        eprint!("{}", ja1front::world::ALLOW_SAVE_NOTICE);
    }
    if pos.len() < 2 || pos.len() > 3 {
        usage();
    }
    if menu_dir.is_some() && pos.len() == 3 {
        menu_dir = Some(pos.pop());
    }
    let (iso, exe) = (pos[0].clone(), pos[1].clone());
    if let Some(d) = menu_dir {
        menu_window(&iso, &exe, d.map(|d| d.into()).or_else(|| save_dir(None)).or(Some(".".into())));
    }
    if let Some((save, out)) = world_args {
        if pos.len() != 2 || png_out.is_some() {
            usage();
        }
        world(&iso, &exe, &save, &out);
    }
    if let Some((d, q, out)) = new_args {
        if pos.len() != 2 || png_out.is_some() {
            usage();
        }
        // world_load.md section 11 test command: the new game's day save. JA1_NEW_GAME_OLD=SAVE: the new game after
        // the game in that day save (new_game_after; newgame_diff --fan 31).
        let old = std::env::var("JA1_NEW_GAME_OLD").ok().filter(|v| !v.is_empty()).map(|v| {
            ja1front::world::World::load(&iso, &exe, &v).unwrap_or_else(|e| die(format!("{}: {}", v, e)))
        });
        let w = ja1front::world::World::new_game_after(&iso, &exe, d, q, old.as_ref())
            .unwrap_or_else(|e| die(format!("world: {}", e)));
        std::fs::write(&out, w.save_bytes()).unwrap_or_else(|e| die(format!("{}: {}", out, e)));
        exit(0);
    }
    if let Some((save, out)) = copy_args {
        if pos.len() != 2 || png_out.is_some() {
            usage();
        }
        save_copy(&iso, &exe, &save, &out);
    }
    if let Some((save, seed, out)) = enter_args {
        if pos.len() != 2 || png_out.is_some() || play_args.is_some() {
            usage();
        }
        enter(&iso, &exe, &save, seed, &out);
    }
    if let Some((save, seed, slot, tile, ticks, out)) = walk_args {
        if pos.len() != 2 || png_out.is_some() || play_args.is_some() {
            usage();
        }
        walk(&iso, &exe, &save, seed, slot, tile, ticks, &out);
    }
    if let Some((save, seed, path, out, serve)) = script_args {
        if pos.len() != 2 || png_out.is_some() || play_args.is_some() {
            usage();
        }
        script(&iso, &exe, &save, seed, &path, &out, serve);
    }
    if let Some((save, seed)) = play_args {
        if pos.len() != 2 {
            usage();
        }
        play(&iso, &exe, &save, seed, png_out);
    }
    let sector: i32 = pos.get(2).map(|s| s.parse().unwrap_or_else(|_| usage())).unwrap_or(1);
    if !(1..=60).contains(&sector) {
        usage();
    }
    if let Some(out) = png_out {
        let (idx, pal) = frame(&iso, &exe, sector, 0, 0);
        write_png(&out, &idx, &pal).unwrap_or_else(|e| {
            eprintln!("ja1-port: {}: {}", out, e);
            exit(1)
        });
        return;
    }
    let mut draw = |s: i32, c0: i32, r0: i32| frame(&iso, &exe, s, c0, r0);
    window("ja1front", &mut draw, sector, 0, 0, true);
}

#[cfg(feature = "gui")]
fn setup(iso: &str, exe: &str) -> ! {
    setupwin::run(iso, exe)
}

#[cfg(not(feature = "gui"))]
fn setup(_iso: &str, _exe: &str) -> ! {
    eprintln!("ja1-port: built without the gui feature: no --setup");
    exit(2)
}

type Draw<'a> = dyn FnMut(i32, i32, i32) -> (Vec<u8>, [u8; 768]) + 'a;

#[cfg(not(feature = "gui"))]
#[allow(clippy::too_many_arguments)]
fn play_window(_iso: &str, _exe: &str, _title: &str, _e: &mut ja1front::enter::Entered, _screen: &mut ja1front::screen::Screen, _pal: &[u8; 768], _c0: i32, _r0: i32, _save_dir: Option<std::path::PathBuf>) {
    eprintln!("ja1-port: built without the gui feature: use --play SAVE [SEED] --png OUT, --walk or --script");
    exit(2)
}

#[cfg(not(feature = "gui"))]
fn window(_title: &str, _draw: &mut Draw, _sector: i32, _c0: i32, _r0: i32, _sectors: bool) {
    eprintln!("ja1-port: built without the gui feature: use --png OUT, --world or --enter");
    exit(2)
}

#[cfg(feature = "gui")]
/// The 4x window: `draw(sector, c0, r0)` gives the view; PageUp / PageDown change the sector when `sectors`.
fn window(title: &str, draw: &mut Draw, mut sector: i32, mut c0: i32, mut r0: i32, sectors: bool) {

    let sdl = sdl2::init().unwrap_or_else(|e| {
        eprintln!("ja1-port: SDL: {}", e);
        exit(1)
    });
    let video = sdl.video().expect("SDL video");
    let mut canvas = Frame::new(video.window("ja1front", W * SCALE, H * SCALE).position_centered(), W, H);
    let tc = canvas.texture_creator();
    let mut tex = tc.create_texture_streaming(PixelFormatEnum::RGB24, W, H).expect("texture");
    frame::drop_lone_modifiers();
    let mut events = sdl.event_pump().expect("events");
    let mut dirty = true;
    loop {
        if dirty {
            let (idx, pal) = draw(sector, c0, r0);
            tex.with_lock(None, |buf: &mut [u8], pitch: usize| {
                for y in 0..H as usize {
                    for x in 0..W as usize {
                        let v = idx[y * W as usize + x] as usize * 3;
                        let d = y * pitch + x * 3;
                        buf[d..d + 3].copy_from_slice(&pal[v..v + 3]);
                    }
                }
            })
            .expect("texture lock");
            canvas.window_mut().set_title(&if sectors {
                format!("{}: sector {} at ({}, {})", title, sector, c0, r0)
            } else {
                format!("{} at ({}, {})", title, c0, r0)
            }).ok();
            dirty = false;
        }
        canvas.copy(&tex, None, None).expect("copy");
        canvas.present();
        let ev = events.wait_event();
        let (oc, or, os) = (c0, r0, sector);
        match ev {
            Event::Quit { .. } | Event::KeyDown { keycode: Some(Keycode::Escape), .. } => break,
            Event::KeyDown { keycode: Some(k), .. } => match k {
                Keycode::Left => c0 = (c0 - 1).max(0),
                Keycode::Right => c0 = (c0 + 1).min(56 - COLS),
                Keycode::Up => r0 = (r0 - 1).max(0),
                Keycode::Down => r0 = (r0 + 1).min(56 - ROWS),
                Keycode::PageUp if sectors => sector = (sector - 1).max(1),
                Keycode::PageDown if sectors => sector = (sector + 1).min(60),
                _ => {}
            },
            _ => {}
        }
        dirty |= (oc, or, os) != (c0, r0, sector);
    }
}

/// A question box in the window (panel.md "Question box" input): `imgs` = the screen with no button, NO, OK
/// pressed. Y / N / Enter / Escape (question::key_answer) or a press and release on a button answers; true = yes.
#[cfg(feature = "gui")]
/// panel.md "The Control Panel": passes until it closes; (result, Show Paths turned off). A left press acts when
/// the button comes up, at the place of the press.
#[allow(clippy::too_many_arguments)]
fn control_panel_loop(cv: &mut Frame, tx: &mut sdl2::render::Texture,
                      ev: &mut sdl2::EventPump, iso: &str, exe: &str, base: &[u8], pal: &[u8; 768],
                      w: &mut ja1front::world::World, host: &mut ja1front::play::PlayHost, restore: bool,
                      mouse: &sdl2::mouse::MouseUtil) -> (i32, bool) {
    pointer_top(false);
    pointer_left(false);
    use sdl2::event::Event;
    use sdl2::keyboard::Keycode;
    use sdl2::mouse::MouseButton;
    let (c, d) = (ja1front::common(iso).unwrap_or_else(|c| fail("palette", c)),
                  ja1front::panel::panel_data(iso, exe).unwrap_or_else(|c| fail("panel", c)));
    let quick = w.base.get(21285).is_some_and(|&b| b != 0);
    // The window always has sound and music (SDL audio), so both drivers count as installed.
    let mut cp = ja1front::ctlpanel::Panel::open(w, quick, restore, w.sound_setup.sound_driver, w.sound_setup.music_driver);
    let mut img = base.to_vec();
    let mut table0 = c.remap[0];
    let mut hide = false;
    let mut press: Option<(i32, i32)> = None;
    loop {
        let (mut left, mut esc) = (None, false);
        for e in ev.poll_iter() {
            match e {
                Event::KeyDown { keycode: Some(Keycode::Escape), .. } => esc = true,
                Event::MouseButtonDown { mouse_btn: MouseButton::Left, x, y, .. } => press = Some(screen_pixel(cv, x, y)),
                Event::MouseButtonUp { mouse_btn: MouseButton::Left, .. } => left = press.take(),
                _ => {}
            }
        }
        let ms = ev.mouse_state();
        // panel.md "The Control Panel" Opening step 2: the mouse stays inside the map view while the panel is open.
        let (px, py) = screen_pixel(cv, ms.x(), ms.y());
        let (bx0, by0, bx1, by1) = ja1front::ctlpanel::mouse_box(w.view_w, w.view_h, w.opt_onscreen as i32);
        let (cx, cy) = (px.clamp(bx0, bx1), py.clamp(by0, by1));
        if panel_warps((cx, cy), (px, py), cv.window().window_flags()) {
            warp_frame(&mouse, cv, cx, cy);
        }
        let (mx, my) = left.unwrap_or_else(|| press.unwrap_or((cx, cy)));
        let out = cp.pass(w, mx, my, left.is_some(), esc);
        for _ in 0..out.clicks {
            host.sound.sfx(0, 0x2B11, 20, 1);
        }
        host.sound.fx_off = ja1front::ctlpanel::fx_off(w);
        let fx_off = host.sound.fx_off;
        ja1front::sound::screen(|s| s.fx_off = fx_off);
        if let Some(on) = out.music {
            ja1front::music::with(|m| m.switch(on));
        }
        if out.hide_marks {
            let n = ja1front::play::show_paths_off(w, host);
            eprintln!("control panel marks {} -> {}", n, w.update.marks_n);
            hide = true;
        }
        if let Some(k) = out.toggled {
            eprintln!("control panel {} {}", k, ja1front::ctlpanel::get(w, k));
        }
        {
            let mut p = ja1front::panel::Painter { d: &d, buf: &mut img, table0: &mut table0, table29: ja1front::panel::table29(&c) };
            ja1front::ctlpanel::paint(&mut p, base, &cp.log);
        }
        cp.log.clear();
        blit(tx, &img, pal, SW, SH);
        cv.copy(tx, None, None).expect("copy");
        cv.present();
        if let Some(r) = out.close {
            return (r, hide);
        }
        std::thread::sleep(std::time::Duration::from_millis(10));
    }
}

/// QuestionBox? 0x279B1 in the window: the mouse place goes to MouseSavePos2?, a mouse outside the box moves to
/// (237, 102), then the wait. These boxes have the return flag 0 (0x27AF0): the mouse stays where the answer left it.
fn question_box(cv: &mut Frame, tx: &mut sdl2::render::Texture, ev: &mut sdl2::EventPump, imgs: &[Vec<u8>],
                pal: &[u8; 768]) -> bool {
    let ms = ev.mouse_state();
    let at = screen_pixel(cv, ms.x(), ms.y());
    ja1front::pointer::mouse_save(at);
    if let Some((x, y)) = ja1front::confirm::open_warp(at) {
        warp_frame(&cv.window().subsystem().sdl().mouse(), cv, x, y);
        eprintln!("question mouse {} {} from {} {}", x, y, at.0, at.1);
    }
    question_loop(cv, tx, ev, imgs, pal, None)
}

fn question_loop(cv: &mut Frame, tx: &mut sdl2::render::Texture,
                 ev: &mut sdl2::EventPump, imgs: &[Vec<u8>], pal: &[u8; 768],
                 anim: Option<&ja1front::sanim::Anim>) -> bool {
    pointer_top(false);
    pointer_left(false);
    use sdl2::event::Event;
    use sdl2::keyboard::Keycode;
    use sdl2::mouse::MouseButton;
    // panel.md "The travel animation": a wait row loops in the picture frame (frame k at each step, k = 0 .. n − 2).
    let mut frame: Option<usize> = None;
    let mut step = std::time::Instant::now();
    let mut show = |cv: &mut Frame, n: usize, frame: Option<usize>| {
        let mut img = imgs[n].clone();
        if let (Some(a), Some(k)) = (anim, frame) {
            a.put(&mut img, k);
        }
        blit(tx, &img, pal, SW, SH);
        cv.copy(tx, None, None).expect("copy");
        cv.present();
    };
    show(cv, 0, None);
    let mut down: Option<usize> = None;
    let mut hold = None;
    loop {
        if let Some(a) = anim {
            if frame.is_none() || step.elapsed().as_millis() as u32 >= a.ms {
                let k = frame.map_or(0, |k| if k + 1 >= a.shown() { 0 } else { k + 1 });
                frame = Some(k);
                step = std::time::Instant::now();
                show(cv, down.map_or(0, |k| 1 + k), frame);
            }
        }
        let evs: Vec<Event> = ev.poll_iter().collect();
        for e in evs {
            match e {
                Event::KeyDown { keycode: Some(k), .. } => {
                    if let Some(yes) = question_key(k, down.is_some()) {
                        return yes;
                    }
                }
                Event::MouseButtonDown { mouse_btn: MouseButton::Left, x, y, .. } if down.is_none() => {
                    let (sx, sy) = screen_pixel(cv, x, y);
                    down = ja1front::question::button_at(sx, sy);
                    if let Some(k) = down {
                        hold = Some(ja1front::pointer::ButtonHold::press((sx, sy)));
                        // sound.md "Button sounds": press = IN, release = OUT
                        ja1front::sound::screen_button_loaded(0);
                        show(cv, 1 + k, frame);
                    }
                }
                Event::MouseButtonUp { mouse_btn: MouseButton::Left, .. } => {
                    if let Some(k) = down {
                        hold_end(cv, hold);
                        ja1front::sound::screen_button_loaded(1);
                        return k == 1;
                    }
                }
                // SPEC GAP: closing the window during the question answers "no".
                Event::Quit { .. } => return false,
                _ => {}
            }
        }
        std::thread::sleep(std::time::Duration::from_millis(10));
    }
}

/// The salary box (op 297, 0x2573C): a left press on a tested button shows its pressed face (sound 0), the release
/// shows it up again (sound 1) and presses it; the box has no keys. Returns the offer (NO = 0).
#[cfg(feature = "gui")]
fn raise_loop(cv: &mut Frame, tx: &mut sdl2::render::Texture,
              ev: &mut sdl2::EventPump, b: &mut ja1front::salary::Screen, pal: &[u8; 768]) -> i16 {
    pointer_top(false);
    pointer_left(false);
    use sdl2::event::Event;
    use sdl2::mouse::MouseButton;
    let show = |cv: &mut Frame, tx: &mut sdl2::render::Texture, img: &[u8]| {
        blit(tx, img, pal, SW, SH);
        cv.copy(tx, None, None).expect("copy");
        cv.present();
    };
    show(cv, tx, &b.img);
    let mut down: Option<usize> = None;
    let mut hold = None;
    loop {
        let evs: Vec<Event> = ev.poll_iter().collect();
        for e in evs {
            match e {
                Event::MouseButtonDown { mouse_btn: MouseButton::Left, x, y, .. } if down.is_none() => {
                    let (sx, sy) = screen_pixel(cv, x, y);
                    down = b.button_at(sx, sy);
                    if let Some(k) = down {
                        hold = Some(ja1front::pointer::ButtonHold::press((sx, sy)));
                        ja1front::sound::screen_button_loaded(0);
                        b.face(k, true);
                        show(cv, tx, &b.img);
                    }
                }
                Event::MouseButtonUp { mouse_btn: MouseButton::Left, .. } => {
                    if let Some(k) = down.take() {
                        hold_end(cv, hold);
                        ja1front::sound::screen_button_loaded(1);
                        b.face(k, false);
                        let r = b.press(k);
                        let o = b.offer();
                        eprintln!("window raise press {} offer {}", k, o.cur);
                        show(cv, tx, &b.img);
                        if let Some(v) = r {
                            return v;
                        }
                    }
                }
                // SPEC GAP: closing the window during the box answers NO.
                Event::Quit { .. } => return 0,
                _ => {}
            }
        }
        std::thread::sleep(std::time::Duration::from_millis(10));
    }
}

/// ConfirmDialog 0x275E4 in the window: `ja1front::confirm::run` with SDL events, the mouse and the sounds.
#[cfg(feature = "gui")]
struct WinConfirm<'a, 'b> {
    cv: &'a mut Frame,
    tx: &'a mut sdl2::render::Texture<'b>,
    ev: &'a mut sdl2::EventPump,
    mouse: &'a sdl2::mouse::MouseUtil,
    pal: &'a [u8; 768],
    iso: &'a str,
    exe: &'a str,
    step: Option<std::time::Instant>,
    /// Some: the keys typed while the answer row waits for the release stay queued for the next reader (KeyOrClick
    /// 0x67E44 reads no key in its release loop, and the caller flushes none: the corpse export question).
    keep: Option<&'a sdl2::EventSubsystem>,
}

#[cfg(feature = "gui")]
impl ja1front::confirm::Host for WinConfirm<'_, '_> {
    fn sound(&mut self, slot: i32, sound_loop: i32) {
        ja1front::sound::screen_sfx(self.iso, self.exe, (slot, 0x2B11, 20, sound_loop));
    }

    fn warp(&mut self, at: (i32, i32)) {
        warp_frame(&self.mouse, self.cv, at.0, at.1);
    }

    fn ask(&mut self, imgs: &[Vec<u8>], wait: Option<&ja1front::sanim::Anim>) -> bool {
        question_loop(self.cv, self.tx, self.ev, imgs, self.pal, wait)
    }

    fn due(&mut self, ms: u32) -> bool {
        let due = self.step.map_or(true, |t| t.elapsed().as_millis() as u32 >= ms);
        if due {
            self.step = Some(std::time::Instant::now());
        }
        due
    }

    fn show(&mut self, img: &[u8]) {
        blit(self.tx, img, self.pal, SW, SH);
        self.cv.copy(self.tx, None, None).expect("copy");
        self.cv.present();
    }

    fn stopped(&mut self) -> bool {
        use sdl2::event::Event;
        // KeyOrClick 0x67E44: a key, or a button when it comes up; the original stays inside the call until then. The
        // start flags are the globals 0x284F1A / 0x284F1C as the last KeyOrClick left them (ConfirmDialog sets none).
        let mut wait = ja1front::invextra::KeyOrClick::new(ja1front::invextra::latch());
        let mut kept = Vec::new();
        let keep = self.keep;
        let give_back = |kept: Vec<Event>| {
            if let Some(es) = keep {
                for e in kept {
                    if let Event::KeyDown { keycode, .. } = &e {
                        eprintln!("confirm kept key {}", keycode.map_or(0, |k| k.into_i32()));
                    }
                    let _ = es.push_event(e);
                }
            }
        };
        loop {
            let (mut key, mut quit) = (false, false);
            let in_wait = wait.in_release();
            for e in self.ev.poll_iter() {
                note_last_key(&e);
                key |= matches!(e, Event::KeyDown { .. });
                quit |= matches!(e, Event::Quit { .. });
                if in_wait && matches!(e, Event::KeyDown { .. }) {
                    kept.push(e);
                }
            }
            let m = self.ev.mouse_state();
            let buttons = m.left() as u8 | (m.right() as u8) << 1;
            let before = wait.in_release();
            if quit || wait.step(buttons, key) {
                give_back(kept);
                return true;
            }
            std::thread::sleep(std::time::Duration::from_millis(5));
            if !wait.in_release() {
                give_back(kept);
                return false;
            }
            if !before {
                eprintln!("confirm release wait");
            }
        }
    }

    fn stop_sounds(&mut self) {
        ja1front::sound::screen(|s| s.stop_all());
    }
}

/// The map keys that waited in the queue during a release wait of the wage question's yes row, read now as the map
/// loop reads them after the box. True when one of them starts the day; a kept Alt-X sets `quit` when the map asks.
#[cfg(feature = "gui")]
fn map_kept_keys(map: &mut ja1front::mapui::MapUi, w: &mut ja1front::world::World, quit: &mut bool) -> bool {
    let mut start = false;
    for c in map.take_kept() {
        eprintln!("map kept key {}", c);
        if c == ja1front::mapui::KEPT_ALT_X {
            *quit |= map.alt_x(w).unwrap_or_else(|c| fail("map", c));
        } else {
            start |= map.key(w, c).unwrap_or_else(|c| fail("map key", c)) == ja1front::mapui::Key::Start;
        }
    }
    start
}

/// One ConfirmDialog box in the window over the screen `under`, flags 1, 1, 1 (the island map's callers). `rows` =
/// wait, yes, no row of archive `arc`; `number` goes into the yes row's text. Returns the answer.
#[cfg(feature = "gui")]
#[allow(clippy::too_many_arguments)]
fn confirm_win(cv: &mut Frame, tx: &mut sdl2::render::Texture, ev: &mut sdl2::EventPump,
               mouse: &sdl2::mouse::MouseUtil, pal: &[u8; 768], iso: &str, exe: &str, text: &[u8], arc: &str,
               rows: (i32, i32, Option<i32>), number: Option<i32>, under: &[u8],
               keep: Option<&sdl2::EventSubsystem>) -> bool {
    use ja1front::sanim::load;
    let ms = ev.mouse_state();
    let at = screen_pixel(cv, ms.x(), ms.y());
    let c = ja1front::confirm::Confirm {
        iso: iso.to_string(), exe: exe.to_string(), text: text.to_vec(), panel: "PANEL_P.PCC",
        wait: load(iso, exe, arc, rows.0, None).ok(), yes: load(iso, exe, arc, rows.1, number).ok(),
        no: rows.2.and_then(|r| load(iso, exe, arc, r, None).ok()),
        flags: ja1front::confirm::MAP, under: under.to_vec(), mouse: at,
    };
    let mut h = WinConfirm { cv, tx, ev, mouse, pal, iso, exe, step: None, keep };
    match ja1front::confirm::run(&c, &mut h) {
        Ok((yes, end)) => {
            use ja1front::confirm::Host;
            h.show(&end);
            yes
        }
        Err(code) => fail("question box", code),
    }
}

#[cfg(feature = "gui")]
fn blit(tex: &mut sdl2::render::Texture, idx: &[u8], pal: &[u8; 768], w: u32, h: u32) {
    if w * h == SW * SH {
        record_frame(idx, pal);
        // #64 rig: the DOS pointer is drawn into the picture, so it takes this palette's colours.
        frame::dos_palette(pal);
    }
    tex.with_lock(None, |buf: &mut [u8], pitch: usize| {
        for y in 0..h as usize {
            for x in 0..w as usize {
                let v = idx[y * w as usize + x] as usize * 3;
                let d = y * pitch + x * 3;
                buf[d..d + 3].copy_from_slice(&pal[v..v + 3]);
            }
        }
    })
    .expect("texture lock");
}

/// #31 step 4: JA1_FRAMES=FILE records every DISTINCT frame that the window shows (tools/effects.py reads it):
/// the header "JA1F", then per frame u32 ms since the first frame (real time), the 320x200 colour indexes and the
/// 768-byte palette as blit got it (8-bit values).
fn record_frame(idx: &[u8], pal: &[u8; 768]) {
    use std::io::Write;
    type Rec = (std::fs::File, Vec<u8>);
    static REC: std::sync::Mutex<Option<Option<Rec>>> = std::sync::Mutex::new(None);
    let mut g = REC.lock().unwrap();
    let rec = g.get_or_insert_with(|| {
        let path = std::env::var("JA1_FRAMES").ok()?;
        let mut f = std::fs::File::create(&path).unwrap_or_else(|e| die(format!("{}: {}", path, e)));
        f.write_all(b"JA1F").expect("JA1_FRAMES write");
        Some((f, Vec::new()))
    });
    let Some((f, last)) = rec else { return };
    if last.len() == idx.len() + 768 && last[..idx.len()] == *idx && last[idx.len()..] == pal[..] {
        return;
    }
    last.clear();
    last.extend_from_slice(idx);
    last.extend_from_slice(pal);
    let ms = frame::record_ms();
    f.write_all(&ms.to_le_bytes()).and_then(|_| f.write_all(last)).expect("JA1_FRAMES write");
}

#[cfg(feature = "gui")]
#[allow(clippy::too_many_arguments)]
/// Section 8: the window runs the sector. Each pass (`Play::pass`, the same code as `--script`): the inputs, the due
/// 10 ms ticks (at most 10), `ja1_frame`, `ja1_update_end`, `ja1_fuses`; then the draw. Left click on a tile =
/// shoot or walk, keys 1..8 = choose a merc, D = end turn, arrow keys move the view.
#[cfg(not(feature = "gui"))]
fn menu_window(_iso: &str, _exe: &str, _dir: Option<std::path::PathBuf>) -> ! {
    eprintln!("ja1-port: built without the gui feature: use --script menu SEED SCRIPT OUT");
    exit(2)
}

/// ContinueDayPrompt 0x4FF00 in the window (quicksave::continue_prompt); true = go on from QUICK.SAV.
#[cfg(feature = "gui")]
fn continue_prompt(iso: &str, exe: &str, dir: Option<&std::path::PathBuf>, g: &mut Gfx) -> bool {
    struct Win<'a> {
        iso: &'a str,
        exe: &'a str,
        dir: Option<&'a std::path::PathBuf>,
        g: &'a mut Gfx,
        tex: sdl2::render::Texture<'a>,
        pal8: [u8; 768],
    }
    impl ja1front::quicksave::ContinueIo for Win<'_> {
        fn fade(&mut self, pal: &[u8; 768], n: i32, from: i32, to: i32) {
            fade_show(&mut self.g.canvas, &mut self.tex, &[0u8; (SW * SH) as usize], pal, n, from, to);
        }
        fn flush(&mut self) {
            if self.g.events.poll_iter().fold(false, |q, e| q || matches!(e, sdl2::event::Event::Quit { .. })) {
                exit(0);
            }
        }
        fn show_pointer(&mut self) {
            pointer_shape(0);
        }
        fn ask(&mut self, q: (&[u8], &str)) -> bool {
            let imgs = ja1front::quicksave::continue_box(self.iso, self.exe, q).unwrap_or_else(|c| fail("question box", c));
            question_box(&mut self.g.canvas, &mut self.tex, &mut self.g.events, &imgs, &self.pal8)
        }
        fn hide_pointer(&mut self) {
            pointer_visible(false);
        }
        fn delete_day_files(&mut self) {
            if let Some(d) = self.dir {
                ja1front::quicksave::delete_day_files(d);
            }
        }
    }
    let pal = ja1front::quicksave::start_palette(iso, exe).unwrap_or_else(|c| fail("AIMSCRN.PCX", c));
    let tc = g.canvas.texture_creator();
    let tex = tc.create_texture_streaming(PixelFormatEnum::RGB24, SW, SH).expect("texture");
    // The original's pointer stays off until the fade in ends.
    pointer_visible(false);
    let mut w = Win { iso, exe, dir, g, tex, pal8: ja1front::eod::scale_palette(&pal) };
    let yes = ja1front::quicksave::continue_prompt(&pal, &mut w);
    // The menu and the map do not switch the pointer on themselves.
    pointer_visible(true);
    eprintln!("{}", if yes { "continue day yes" } else { "continue day no, day files deleted" });
    yes
}

/// The window from the main menu (world_load.md section 12): the menu screens, then the game from the office; Leave
/// opens the menu again.
#[cfg(feature = "gui")]
fn menu_window(iso: &str, exe: &str, dir: Option<std::path::PathBuf>) -> ! {
    let seed = ja1front::world::start_seed();
    menu_loop(iso, exe, dir, gfx("ja1front: main menu", iso, exe), None, seed)
}

/// The menu loop of `menu_window`; with `world` (a game ended in the window: ending.md "Game end" step 4) the menu
/// opens again as after a game, with that world for Continue.
#[cfg(feature = "gui")]
/// `seed`: main_ 0x10150 and InitProgramWorld? 0x377B0 seed the numbers from the clock once, and the stream goes on over
/// every game of the run. The port seeds each next game from the clock again (JA1_SEED: tests).
/// One window event on the main menu: Up and Down queue the extended key bytes (0, 0x48) and (0, 0x50), Enter the
/// byte 13 (MainMenu? reads them through GetKey); Some((x, y, press)) = the mouse at window place (x, y), press = a
/// left press there.
#[cfg(feature = "gui")]
fn menu_event(ev: &Event, keys: &mut std::collections::VecDeque<u8>) -> Option<(i32, i32, bool)> {
    match *ev {
        Event::MouseMotion { x, y, .. } => Some((x, y, false)),
        Event::MouseButtonDown { mouse_btn: sdl2::mouse::MouseButton::Left, x, y, .. } => Some((x, y, true)),
        Event::KeyDown { keycode: Some(Keycode::Up), .. } => {
            keys.extend([0, 0x48]);
            None
        }
        Event::KeyDown { keycode: Some(Keycode::Down), .. } => {
            keys.extend([0, 0x50]);
            None
        }
        Event::KeyDown { keycode: Some(Keycode::Return | Keycode::KpEnter), .. } => {
            keys.push_back(13);
            None
        }
        _ => None,
    }
}

/// The Control Panel puts the mouse back into its box only when the mouse is out of the box and the window has
/// the input focus (a window in the background does not take the desktop pointer).
#[cfg(feature = "gui")]
fn panel_warps(clamped: (i32, i32), at: (i32, i32), window_flags: u32) -> bool {
    clamped != at && (window_flags & sdl2::sys::SDL_WindowFlags::SDL_WINDOW_INPUT_FOCUS as u32) != 0
}

fn menu_loop(iso: &str, exe: &str, dir: Option<std::path::PathBuf>, g: Gfx, world: Option<ja1front::world::World>, seed: u32) -> ! {
    use std::time::Duration;
    let mut g = g;
    let mut m = ja1front::menu::Menu::new(iso, exe, dir.clone()).unwrap_or_else(|c| fail("menu", c));
    if world.is_some() {
        m.reopen();
    } else if std::env::var("JA1_TITLE").map_or(true, |v| v != "0") {
        // The program start: the title part, then the main menu (JA1_TITLE=0 skips it: the window tests).
        let tc = g.canvas.texture_creator();
        let mut tex = tc.create_texture_streaming(PixelFormatEnum::RGB24, SW, SH).expect("texture");
        g.canvas.window_mut().set_title("ja1front: title").ok();
        // InitMainDatMouse 0x36F90: the pointer stays off through the title part; then the mouse goes to the middle of the
        // screen with the arrow shape.
        let start = ja1front::startup::START;
        pointer_visible(!start.pointer_off);
        // config.rs title_films = off (JA1_TITLE_FILMS=0, port option): no films; the mouse start stays.
        if ja1front::config::read::title_films(std::env::var("JA1_TITLE_FILMS").ok().as_deref()) {
            title_window(iso, exe, ja1front::ctlpanel::sub_byte(ja1front::world::World::program_start_subtitles(exe) as u8), &mut g.canvas, &mut tex, &mut g.events);
        } else {
            eprintln!("title films off");
        }
        eprintln!("start mouse {} {}", start.mouse.0, start.mouse.1);
        warp_frame(&g.sdl.mouse(), &g.canvas, start.mouse.0, start.mouse.1);
        pointer_visible(true);
    }
    let mut world = world;
    let mut keys = std::collections::VecDeque::new();
    let mut mouse = if std::env::var("JA1_TITLE").map_or(true, |v| v != "0") { ja1front::startup::START.mouse } else { (0, 0) };
    // world_load.md section 12 "Continue last day in progress?": at the program start with QUICK.SAV the day goes on
    // from it (the menu does not show first).
    let mut seed = seed;
    let mut quick_start = world.is_none() && dir.as_ref().is_some_and(|d| d.join("QUICK.SAV").exists()) && continue_prompt(iso, exe, dir.as_ref(), &mut g);
    loop {
        if quick_start {
            quick_start = false;
            ja1front::record::start(exe, "quick continue");
            let w = ja1front::world::World::new_game(iso, exe, 0, 0).unwrap_or_else(|e| die(format!("world: {}", e)));
            let mut e = ja1front::enter::Entered { loaded: Box::new(w.clone()), world: w, tileset: 0, seed, draws: 0 };
            let mut screen = ja1front::screen::Screen::new(iso, exe, &mut e.world).unwrap_or_else(|c| fail("faces", c));
            let pal = palette(iso, 0).unwrap_or_else(|c| fail("palette", c));
            let (to_menu, g2) = play_window(iso, exe, "ja1front", &mut e, &mut screen, &pal, 0, 0, dir.clone(), g, false, true);
            g = g2;
            if !to_menu {
                exit(0);
            }
            world = Some(e.world);
            seed = ja1front::world::start_seed();
            m.reopen();
        }
        let o = {
            let tc = g.canvas.texture_creator();
            let mut tex = tc.create_texture_streaming(PixelFormatEnum::RGB24, SW, SH).expect("texture");
            g.canvas.window_mut().set_title("ja1front: main menu").ok();
            // panel.md "The mouse pointer": the main menu shows the shape set last (0 at the program start and after a game)
            pointer_shape(0);
            loop {
                std::thread::sleep(Duration::from_millis(10));
                let mut down = false;
                for evt in g.events.poll_iter() {
                    match evt {
                        Event::Quit { .. } => exit(0),
                        e => {
                            if let Some((x, y, press)) = menu_event(&e, &mut keys) {
                                mouse = screen_pixel(&g.canvas, x, y);
                                down |= press;
                            }
                        }
                    }
                }
                let left = seen_or_held(down, g.events.mouse_state().left());
                let o = m.poll(mouse.0, mouse.1, left, &mut keys);
                for l in m.lines.drain(..) {
                    eprintln!("{}", l);
                }
                // Insert #13553: config.rs intro = off (JA1_INTRO=0) skips the film; START NEW GAME goes on to Set
                // Difficulty with the same fades and pointer shape 202.
                let film = std::mem::take(&mut m.intro);
                if film && !ja1front::config::read::intro(std::env::var("JA1_INTRO").ok().as_deref()) {
                    eprintln!("intro off");
                    pointer_shape(202);
                } else if film {
                    let sub = world.as_ref().map_or_else(|| ja1front::ctlpanel::sub_byte(ja1front::world::World::program_start_subtitles(exe) as u8), |w| ja1front::ctlpanel::sub_byte(w.update.opt_step_text));
                    intro_window(iso, exe, sub, &mut g.canvas, &mut tex, &mut g.events);
                    keys.clear();
                    // panel.md "The mouse pointer": shape 202 after the intro (Set Difficulty, Set Quick-Save Mode)
                    pointer_shape(202);
                }
                // world_load.md "Fades of the menu screens": one level per vertical retrace.
                for (img, pal, n, from, to) in std::mem::take(&mut m.fades) {
                    for p in ja1front::office::fade(&pal, n, from, to) {
                        blit(&mut tex, &img, &ja1front::eod::scale_palette(&p), SW, SH);
                        g.canvas.copy(&tex, None, None).expect("copy");
                        g.canvas.present();
                        std::thread::sleep(Duration::from_micros(14_286));
                    }
                }
                if let Some((x, y)) = m.warp.take() {
                    mouse = (x, y);
                    warp_frame(&g.sdl.mouse(), &g.canvas, x, y);
                }
                if let Some(o) = o {
                    // Every choice ends with a fade out (above): the screen stays black.
                    break o;
                }
                blit(&mut tex, &m.buf, &ja1front::eod::scale_palette(&m.pal), SW, SH);
                g.canvas.copy(&tex, None, None).expect("copy");
                g.canvas.present();
            }
        };
        let w = match o {
            ja1front::menu::Outcome::NewGame(d, q) => {
                ja1front::record::start(exe, "new game");
                let mut n = ja1front::world::World::new_game_after(iso, exe, d, q, world.as_ref())
                    .unwrap_or_else(|e| die(format!("world: {}", e)));
                // strategic.md "The day replay": no new game writes replay_carry (the original's byte is not in a save).
                match world.as_ref() {
                    Some(old) => n.replay_carry = old.replay_carry,
                    None => n.sound_cfg_options(exe),
                }
                n
            }
            // Section 12: Continue at the program start uses the program-start world.
            ja1front::menu::Outcome::Continue => match {
                ja1front::record::start(exe, "continue");
                world.take()
            } {
                Some(w) => w,
                None => ja1front::world::World::program_start(iso, exe).unwrap_or_else(|e| die(format!("world: {}", e))),
            },
            ja1front::menu::Outcome::Exit => exit(0),
        };
        let mut e = ja1front::enter::Entered { loaded: Box::new(w.clone()), world: w, tileset: 0, seed, draws: 0 };
        let mut screen = ja1front::screen::Screen::new(iso, exe, &mut e.world).unwrap_or_else(|c| fail("faces", c));
        let pal = palette(iso, 0).unwrap_or_else(|c| fail("palette", c));
        let (to_menu, g2) = play_window(iso, exe, "ja1front", &mut e, &mut screen, &pal, 0, 0, dir.clone(), g, true, false);
        g = g2;
        if !to_menu {
            exit(0);
        }
        world = Some(e.world);
        seed = ja1front::world::start_seed();
        m.reopen();
    }
}

/// A fade (ending.md op 24 / 25 rule; office::fade) of `img` with the colours `pal` (6-bit): n steps from level
/// `from` to `to`, one level per vertical retrace (1/70 s).
#[cfg(feature = "gui")]
fn fade_show(cv: &mut Frame, tx: &mut sdl2::render::Texture<'_>, img: &[u8],
             pal: &[u8; 768], n: i32, from: i32, to: i32) {
    // FadePalette? 0x52908 writes one level per vertical retrace (70 Hz); the draw time is part of that step.
    let t0 = std::time::Instant::now();
    for (k, p) in ja1front::office::fade(pal, n, from, to).iter().enumerate() {
        blit(tx, img, &ja1front::eod::scale_palette(p), SW, SH);
        cv.copy(tx, None, None).expect("copy");
        cv.present();
        std::thread::sleep(ja1front::dostime::retrace_due(k).saturating_sub(t0.elapsed()));
    }
}

/// `fade_show` for a palette in 8-bit window form (the tactical palette, the island map's): the fade runs on the 6-bit
/// values of the original's DAC.
#[cfg(feature = "gui")]
fn fade_show8(cv: &mut Frame, tx: &mut sdl2::render::Texture<'_>, img: &[u8],
              pal8: &[u8; 768], n: i32, from: i32, to: i32) {
    fade_show(cv, tx, img, &pal8.map(|v| v >> 2), n, from, to);
}

/// The SDL window of a run (the main menu, the office and play share it).
#[cfg(feature = "gui")]
struct Gfx {
    sdl: sdl2::Sdl,
    canvas: Frame,
    events: sdl2::EventPump,
    /// music.md "Output": the music's own audio device (None: no audio device).
    music: Option<sdl2::audio::AudioDevice<MusicOut>>,
}

/// Not in the original: with `JA1_MUTE_UNFOCUSED` = 1 the window is silent while another window has the keyboard
/// focus (the maintainer, to compare the port with the original side by side). The sound still runs, only the output is muted.
#[cfg(feature = "gui")]
fn muted() -> bool {
    static ON: std::sync::OnceLock<bool> = std::sync::OnceLock::new();
    *ON.get_or_init(|| ja1front::config::read::mute_when_away(std::env::var("JA1_MUTE_UNFOCUSED").ok().as_deref()))
        && unsafe { sdl2::sys::SDL_GetKeyboardFocus() }.is_null()
}

/// music.md "Output": the window renders the music from the audio callback.
#[cfg(feature = "gui")]
struct MusicOut {
    buf: Vec<i32>,
}

#[cfg(feature = "gui")]
impl sdl2::audio::AudioCallback for MusicOut {
    type Channel = i16;
    fn callback(&mut self, out: &mut [i16]) {
        self.buf.clear();
        self.buf.resize(out.len(), 0);
        ja1front::music::with(|m| m.render(&mut self.buf));
        ja1front::sound::screen(|s| s.render_add(&mut self.buf));
        if muted() {
            self.buf.fill(0);
        }
        for (o, v) in out.iter_mut().zip(self.buf.iter()) {
            *o = (*v).clamp(-32768, 32767) as i16;
        }
    }
}

#[cfg(feature = "gui")]
fn gfx(title: &str, iso: &str, exe: &str) -> Gfx {
    let sdl = sdl2::init().unwrap_or_else(|m| die(format!("SDL: {}", m)));
    let video = sdl.video().expect("SDL video");
    // config.rs window options (port options): size, full screen, smooth scaling (the hint holds for the textures
    // made after it).
    let look = frame::look();
    if look.smooth {
        sdl2::hint::set("SDL_RENDER_SCALE_QUALITY", "linear");
    }
    let mut b = video.window(title, SW * look.scale, SH * look.scale);
    b.position_centered();
    if look.fullscreen {
        b.fullscreen_desktop();
    }
    let canvas = Frame::new(&mut b, SW, SH);
    pointer_install(iso, exe);
    frame::drop_lone_modifiers();
    let events = sdl.event_pump().expect("events");
    let want = sdl2::audio::AudioSpecDesired { freq: Some(ja1front::sound::RATE as i32), channels: Some(1), samples: Some(1024) };
    // SDL's dummy audio driver (tests) has one device only: the sound queue takes it, the music has none.
    // JA1_DUMMY_SCREEN_AUDIO=1 (tests): the screen sound takes it, so a voice on the office and A.I.M. screens plays to its end.
    let screen_audio = std::env::var_os("JA1_DUMMY_SCREEN_AUDIO").is_some();
    let music = sdl.audio().ok().filter(|a| ja1front::music::opens_device(a.current_audio_driver(), screen_audio))
        .and_then(|a| a.open_playback(None, &want, |_| MusicOut { buf: vec![] }).ok());
    if let Some(d) = music.as_ref() {
        ja1front::music::DEVICE.store(true, std::sync::atomic::Ordering::Relaxed);
        d.resume();
    }
    Gfx { sdl, canvas, events, music }
}

#[cfg(feature = "gui")]
thread_local! {
    /// panel.md "The mouse pointer": the window's pointers for shapes 0, 200, 202, 203, 201, and the shape shown while
    /// no button is held (0, 201 or 202).
    static POINTERS: std::cell::RefCell<Vec<sdl2::mouse::Cursor>> = const { std::cell::RefCell::new(Vec::new()) };
    static POINTER_BASE: std::cell::Cell<i32> = const { std::cell::Cell::new(0) };
    /// A map flip hides the pointer now (stderr `pointer flip 0|1` at each change).
    static POINTER_FLIP: std::cell::Cell<bool> = const { std::cell::Cell::new(false) };
    /// The pointer is off over the island map or the guard map (stderr `pointer map 0|1` at each change).
    static POINTER_MAP: std::cell::Cell<bool> = const { std::cell::Cell::new(false) };
    /// The system pointer shown (None = not set yet).
    static POINTER_ON: std::cell::Cell<Option<bool>> = const { std::cell::Cell::new(None) };
    static POINTER_WANT: std::cell::Cell<bool> = const { std::cell::Cell::new(true) };
    static POINTER_TOP: std::cell::Cell<bool> = const { std::cell::Cell::new(false) };
    static POINTER_BOX: std::cell::Cell<bool> = const { std::cell::Cell::new(false) };
    static POINTER_LEFT: std::cell::Cell<bool> = const { std::cell::Cell::new(false) };
    static POINTER_LEFT_VIEW: std::cell::Cell<(u8, bool)> = const { std::cell::Cell::new((1, false)) };
}

/// The system pointer on or off (off over the tactical map, where the tile cursor shows the mouse, and while the
/// screen draws a held item at the mouse); stderr `pointer visible 0|1` at each change.
#[cfg(feature = "gui")]
fn pointer_visible(on: bool) {
    POINTER_WANT.with(|v| v.set(on));
    pointer_apply();
}

/// HideMouse? 0x67368 in the window: the start of a hold in which the game draws by itself (a view slide, a speech balloon,
/// a death morph). Returns the off byte for `pointer_hold_end`. stderr `pointer hold off N` (not over the map: 1).
#[cfg(feature = "gui")]
fn pointer_hold_start(over_map: u8) -> bool {
    let off = ja1front::pointer::hide_mouse(!POINTER_WANT.with(|v| v.get()), over_map);
    eprintln!("pointer hold off {}", (over_map == 0) as u8);
    pointer_visible(!off);
    off
}

/// MouseRedraw? 0x67398 in the window, the end of the hold. A held item stays the frame's picture, not the system pointer.
#[cfg(feature = "gui")]
/// Byte 0x92DCC (`screen_drawn`): the tactical screen is drawn; ShowMouse? does nothing while it is 0.
fn tactical_ready(screen_drawn: u8) -> bool {
    screen_drawn != 0
}

/// The button bits of the mouse driver: bit 0 left, bit 1 right, or-ed with the bits of the presses seen since the
/// last poll (`down`).
/// Keys stay queued while a drag, a map/strip press, a right press or a hold wait runs (WaitRightUp at the end of
/// the right arm of UseItemAt 0x1D9B8: keys act after the release too, live, issue #31). Ledger row 0b4d529:1058.
/// The island map eats every other mouse button (middle, extra) while it is open: none reaches the tactical arms
/// below it (a press there ends a pause or time compression). Ledger row d057aca:1143.
fn map_eats_button(ev: &sdl2::event::Event, map_open: bool) -> bool {
    use sdl2::event::Event;
    map_open && matches!(ev, Event::MouseButtonDown { .. } | Event::MouseButtonUp { .. })
}

fn keys_held(drag: bool, map_press: bool, right_press: bool, hold_wait: bool) -> bool {
    drag || map_press || right_press || hold_wait
}

fn mouse_bits(left: bool, right: bool, down: i32) -> i32 {
    left as i32 | (right as i32) << 1 | down
}

/// One event of the box wait: a key, the window close, a left (bit 0) or right (bit 1) press.
#[cfg(feature = "gui")]
fn box_event(ev: &Event, key: &mut bool, quit: &mut bool, down: &mut u8) {
    match ev {
        Event::KeyDown { .. } => *key = true,
        Event::Quit { .. } => *quit = true,
        Event::MouseButtonDown { mouse_btn: sdl2::mouse::MouseButton::Left, .. } => *down |= 1,
        Event::MouseButtonDown { mouse_btn: sdl2::mouse::MouseButton::Right, .. } => *down |= 2,
        _ => {}
    }
}

/// A key in the question box: Return 13, Escape 27, else its code; no answer while a button is held down.
#[cfg(feature = "gui")]
fn question_key(k: Keycode, held: bool) -> Option<bool> {
    if held {
        return None;
    }
    let c = match k {
        Keycode::Return => 13,
        Keycode::Escape => 27,
        k => k.into_i32().clamp(0, 255) as u8,
    };
    ja1front::question::key_answer(c)
}

/// A button of a wait: pressed since the last poll (an event) or held now.
fn seen_or_held(seen: bool, held: bool) -> bool {
    seen | held
}

/// A wait without face rules ends at any key or button.
fn any_input(key: bool, left: bool, right: bool) -> bool {
    key || left || right
}

/// A Locked balloon inside a pass owes its ticks to the pass timers (`balloon_owed`); outside a pass the game's
/// locked ticks run.
fn pass_owes_ticks(locked: bool, in_pass: bool) -> bool {
    locked && in_pass
}

fn pointer_hold_end(off: bool, over_map: u8, ready: bool, held: bool) {
    let off = ja1front::pointer::mouse_redraw(off, over_map, ready);
    eprintln!("pointer hold end {}", off as u8);
    if !off {
        POINTER_LEFT.with(|v| v.set(false));
    }
    pointer_visible(!off && !held);
}

/// The wait of ShowMessage? 0x27C69 .. 0x27C82 in a box on the tactical screen: MousePoll?, KeyOrClick 0x67E44, then the timer. A
/// press ends the box at its release, and neither a key nor the timer counts until then; a button that was down at the start of the
/// box counts only after it came up. stderr `box release wait` at a press. True when the window closed.
#[cfg(feature = "gui")]
fn box_wait(events: &mut sdl2::EventPump, ms: u64, shown: &Instant) -> bool {
    let level = |m: sdl2::mouse::MouseState| mouse_bits(m.left(), m.right(), 0) as u8;
    let mut wait = ja1front::invextra::KeyOrClick::new(level(events.mouse_state()));
    loop {
        let (mut key, mut quit, mut down) = (false, false, 0u8);
        for ev in events.poll_iter() {
            note_last_key(&ev);
            box_event(&ev, &mut key, &mut quit, &mut down);
        }
        if quit {
            return true;
        }
        // (a press and its release between two polls: the driver's queue 0x68470 keeps both)
        let was = wait.in_release();
        let m = events.mouse_state();
        let mut end = wait.step(mouse_bits(m.left(), m.right(), down as i32) as u8, key && !was);
        if !was && wait.in_release() {
            eprintln!("box release wait");
            end = wait.step(level(events.mouse_state()), false);
        }
        if end {
            return false;
        }
        if !wait.in_release() && shown.elapsed() >= Duration::from_millis(ms) {
            return false;
        }
        std::thread::sleep(Duration::from_millis(10));
    }
}

/// ja1front::pointer::top_row_hidden in the window: the pointer is off on the top rows of the plain tactical screen.
/// The window's tactical pass decides it per frame, and every box loop lets it go. stderr `pointer top N` at each change.
#[cfg(feature = "gui")]
fn pointer_top(hide: bool) {
    if POINTER_TOP.with(|v| v.replace(hide)) != hide {
        eprintln!("pointer top {}", hide as u8);
    }
    pointer_apply();
}

/// ja1front::pointer::leave_hides in the window: rows 196..199 of the view with no panel show no pointer. stderr `pointer left N`
/// at each change.
#[cfg(feature = "gui")]
fn pointer_left(off: bool) {
    if POINTER_LEFT.with(|v| v.replace(off)) != off {
        eprintln!("pointer left {}", off as u8);
    }
    pointer_apply();
}

/// ShowMessage? 0x27B98 in the window: HidePointer? 0x67BF0 at the start (0x27BBE), ShowMouse? 0x673AC at the end
/// (0x27CC0), so the pointer is off while the box shows. stderr `pointer box N` at each change.
#[cfg(feature = "gui")]
fn pointer_box(on: bool) {
    if POINTER_BOX.with(|v| v.replace(on)) != on {
        eprintln!("pointer box {}", on as u8);
    }
    pointer_apply();
}

#[cfg(feature = "gui")]
fn pointer_apply() {
    let on = POINTER_WANT.with(|v| v.get()) && !POINTER_TOP.with(|v| v.get()) && !POINTER_BOX.with(|v| v.get())
        && !POINTER_LEFT.with(|v| v.get());
    // The DOS mouse mode draws the pointer into the picture; the desktop pointer shows only with the mouse released.
    unsafe {
        sdl2::sys::SDL_ShowCursor((on && !frame::dos_captured()) as i32);
    }
    frame::dos_pointer(None, Some(on));
    if POINTER_ON.with(|v| v.replace(Some(on))) != Some(on) {
        eprintln!("pointer visible {}", on as u8);
    }
}

/// panel.md "The switch view" in the window.
struct SwitchUi {
    k: u8,
    step: SwStep,
    t: Instant,
}

#[derive(Clone, Copy, PartialEq)]
enum SwStep {
    /// the open frames (20 ms each), then Up
    Open,
    /// open; a left press on a switch presses it, a right press ends it
    Up,
    /// switch i held down
    Press(usize),
    /// the right button went down: the close starts when it comes up
    WaitRight,
    /// the close frames, then the slot
    Close,
}

/// The index in POINTERS of shape `base` (0, 201 or 202), turned when a button is held (200 / 203; 201 stays).
#[cfg(feature = "gui")]
fn pointer_index(base: i32, buttons: u32) -> usize {
    ja1front::pointer::window_index(base, buttons)
}

/// panel.md "The mouse pointer": the window shows shape `base` (0, 201 or 202) from now on; stderr `pointer shape N`.
#[cfg(feature = "gui")]
fn pointer_shape(base: i32) {
    // #20 C: a screen with a pointer shape shows the pointer (the tactical map may have hidden it before the day end)
    pointer_visible(true);
    if POINTER_BASE.with(|b| b.replace(base)) != base {
        let held = unsafe { sdl2::sys::SDL_GetMouseState(std::ptr::null_mut(), std::ptr::null_mut()) };
        POINTERS.with(|p| {
            if let Some(c) = p.borrow().get(pointer_index(base, held)) {
                c.set();
            }
        });
        frame::dos_pointer(Some(pointer_index(base, held)), None);
        eprintln!("pointer shape {}", base);
    }
}

/// Shape 202 while the office is up (InitOfficeScreen? 0xCA), shape 0 again when it ends.
#[cfg(feature = "gui")]
struct OfficePointer;

#[cfg(feature = "gui")]
impl OfficePointer {
    fn new() -> OfficePointer {
        pointer_shape(202);
        // The tactical pass can leave the pointer off (the mouse over the map, key A); the office always shows it.
        pointer_visible(true);
        OfficePointer
    }
}

#[cfg(feature = "gui")]
impl Drop for OfficePointer {
    fn drop(&mut self) {
        pointer_shape(0);
    }
}

/// panel.md "The mouse pointer": the window's pointer is shape 0 of the panel sheet (tileset 0 colours, SCALE times
/// as big), shape 200 while a mouse button is held (202 / 203 in the office). stderr `pointer shapes 0 200 202 203`.
#[cfg(feature = "gui")]
fn pointer_install(iso: &str, exe: &str) {
    let (Ok(d), Ok(pal)) = (ja1front::panel::panel_data(iso, exe), ja1front::palette(iso, 0)) else { return };
    let mut set = Vec::new();
    let mut dos = Vec::new();
    for s in [0, 200, 202, 203, 201] {
        let Some(sh) = ja1front::pointer::shape(&d, s) else { return };
        let (dw, dh) = ja1front::pointer::rgba_size(&sh, 1);
        dos.push((sh.pix.clone(), dw, dh, sh.hx, sh.hy));
        let k = SCALE as i32;
        let mut px = ja1front::pointer::rgba(&sh, &pal, k);
        let (w, h) = ja1front::pointer::rgba_size(&sh, k);
        let Ok(surf) = sdl2::surface::Surface::from_data(&mut px, w, h, w * 4, sdl2::pixels::PixelFormatEnum::RGBA32) else { return };
        let Ok(c) = sdl2::mouse::Cursor::from_surface(surf, sh.hx * k, sh.hy * k) else { return };
        set.push(c);
    }
    set[0].set();
    POINTERS.with(|p| *p.borrow_mut() = set);
    if frame::dos_mode() {
        frame::dos_pointers(dos);
        frame::dos_palette(&pal);
        frame::dos_start();
    }
    unsafe extern "C" fn watch(_: *mut std::ffi::c_void, ev: *mut sdl2::sys::SDL_Event) -> i32 {
        let t = (*ev).type_;
        if t == sdl2::sys::SDL_EventType::SDL_MOUSEBUTTONDOWN as u32 || t == sdl2::sys::SDL_EventType::SDL_MOUSEBUTTONUP as u32 {
            let held = sdl2::sys::SDL_GetMouseState(std::ptr::null_mut(), std::ptr::null_mut());
            POINTERS.with(|p| {
                if let Some(c) = p.borrow().get(pointer_index(POINTER_BASE.with(|b| b.get()), held)) {
                    c.set();
                }
            });
            frame::dos_pointer(Some(pointer_index(POINTER_BASE.with(|b| b.get()), held)), None);
        }
        1
    }
    unsafe { sdl2::sys::SDL_AddEventWatch(Some(watch), std::ptr::null_mut()) };
    eprintln!("pointer shapes 0 200 202 203 201");
}

/// The button number of the message box rule (0 = a button the game does not read).
#[cfg(feature = "gui")]
fn box_button(b: sdl2::mouse::MouseButton) -> u8 {
    match b {
        sdl2::mouse::MouseButton::Left => 1,
        sdl2::mouse::MouseButton::Right => 2,
        _ => 0,
    }
}

/// sound.md "Output": the samples mixed since the last call go to the window's audio queue.
#[cfg(feature = "gui")]
fn feed_audio(q: Option<&sdl2::audio::AudioQueue<i16>>, s: &mut ja1front::sound::Sound) {
    let p = std::mem::take(&mut s.pending);
    if let Some(q) = q {
        queue_part(q, &p);
    }
}

/// Late sound waiting in the queue (the window was held up) goes, so the new part plays at once. stderr `audio
/// cleared N ms` for each clear and `audio queue max N ms` every 5 s (the most sound that waited, after the part).
#[cfg(feature = "gui")]
fn queue_part(q: &sdl2::audio::AudioQueue<i16>, p: &[i16]) {
    use std::sync::Mutex;
    static STAT: Mutex<Option<(std::time::Instant, u32)>> = Mutex::new(None);
    let ms = |b: u32| b as u64 * 1000 / (2 * ja1front::sound::RATE as u64);
    if ja1front::sound::queue_stale(q.size()) {
        eprintln!("audio cleared {} ms", ms(q.size()));
        q.clear();
    }
    if muted() {
        // the part is dropped, so nothing old plays when the focus comes back
        q.clear();
        return;
    }
    q.queue_audio(p).ok();
    let mut st = STAT.lock().unwrap();
    let (t0, max) = st.get_or_insert((std::time::Instant::now(), 0));
    *max = (*max).max(q.size());
    if t0.elapsed().as_secs() >= 5 {
        eprintln!("audio queue max {} ms", ms(*max));
        *st = Some((std::time::Instant::now(), 0));
    }
}

/// panel.md "Speech balloon": the face slides in, the text shows until the wait ends, the face slides out; no game
/// time passes meanwhile. `locked` runs the Locked time of each wait as timer ticks (the balloons of a finished
/// pass); a balloon shown inside a pass (the trap bark before the disarm box, #36 B) must not run them again.
/// Returns false when the window closed.
#[cfg(feature = "gui")]
fn world_view(w: &ja1front::world::World) -> ja1front::sighting::ViewBytes {
    let u = &w.update;
    ja1front::sighting::ViewBytes { view_x: w.view_x, view_y: w.view_y, show_timer: u.show_timer, view_tile: u.view_tile, spotted: u.spotted,
                                    focus_slot: u.focus_slot, focus_last: u.focus_last, shown_slot: u.shown_slot }
}

#[cfg(feature = "gui")]
fn set_world_view(w: &mut ja1front::world::World, b: &ja1front::sighting::ViewBytes) {
    (w.view_x, w.view_y) = (b.view_x, b.view_y);
    let u = &mut w.update;
    (u.show_timer, u.view_tile, u.spotted) = (b.show_timer, b.view_tile, b.spotted);
    (u.focus_slot, u.focus_last, u.shown_slot) = (b.focus_slot, b.focus_last, b.shown_slot);
}

/// WaitLeftUp 0x67C54 and WaitRightUp 0x67C68 of the speech roads: the poll goes on until the buttons are up, with no
/// new picture. The other events stay in the queue. Returns the time of the wait in ms, 0 = no wait.
#[cfg(feature = "gui")]
fn speech_buttons_up(canvas: &Frame, events: &mut impl ButtonRead, left: bool, right: bool, mut tick: impl FnMut((i32, i32))) -> u128 {
    let begin = Instant::now();
    let mut waited = false;
    loop {
        let (left_down, right_down, x, y) = events.buttons();
        if wanted_buttons_up(left, right, left_down, right_down) {
            break;
        }
        waited = true;
        let at = screen_pixel(canvas, x, y);
        ja1front::pointer::mouse_poll(at);
        tick(at);
        std::thread::sleep(Duration::from_millis(5));
    }
    if waited { begin.elapsed().as_millis().max(1) } else { 0 }
}

/// One window event of the A.I.M. screen's poll: the mouse place (`to_px` maps window to screen pixels), the presses
/// of this batch (bit 0 left, bit 1 right) and the key bytes. True = the window closes: the program ends.
#[cfg(feature = "gui")]
fn aim_event(evt: &Event, to_px: impl Fn(i32, i32) -> (i32, i32), mouse: &mut (i32, i32), down: &mut i32, keys: &mut Vec<u8>) -> bool {
    match *evt {
        Event::Quit { .. } => return true,
        Event::MouseMotion { x, y, .. } => *mouse = to_px(x, y),
        Event::MouseButtonDown { mouse_btn, x, y, .. } => {
            *mouse = to_px(x, y);
            *down |= match mouse_btn {
                sdl2::mouse::MouseButton::Left => 1,
                sdl2::mouse::MouseButton::Right => 2,
                _ => 0,
            };
        }
        Event::KeyDown { keycode: Some(k), keymod, .. } => keys.extend(aim_key_bytes(k, keymod)),
        _ => {}
    }
    false
}

/// The mouse buttons and place now (left down, right down, window x, y): the speech button-up waits read them.
#[cfg(feature = "gui")]
trait ButtonRead {
    fn buttons(&mut self) -> (bool, bool, i32, i32);
}

#[cfg(feature = "gui")]
impl ButtonRead for sdl2::EventPump {
    fn buttons(&mut self) -> (bool, bool, i32, i32) {
        self.pump_events();
        let ms = self.mouse_state();
        (ms.left(), ms.right(), ms.x(), ms.y())
    }
}

/// A balloon of `show_balloons`: the soldier record, quote, weapon text, sector line flag, the view move and the path marks of the old picture. None = Santino's speech.
#[cfg(feature = "gui")]
type BalloonShow = Option<(Vec<u8>, i8, String, bool, Option<ja1front::sighting::ViewUndo>, Vec<(usize, u8)>)>;

#[cfg(feature = "gui")]
#[allow(clippy::too_many_arguments)]
fn show_balloons(iso: &str, exe: &str, e: &mut ja1front::enter::Entered, screen: &mut ja1front::screen::Screen,
                 _c0: i32, _r0: i32, play: &mut ja1front::play::Play,
                 canvas: &mut Frame, tex: &mut sdl2::render::Texture,
                 events: &mut sdl2::EventPump, pal: &[u8; 768], audio: Option<&sdl2::audio::AudioQueue<i16>>,
                 start: Instant, ticks_done: &mut u64,
                 balloons: Vec<BalloonShow>, locked: bool, in_pass: bool) -> bool {
    // A pass with no balloon leaves the top-row rule alone: this function runs on every pass.
    if balloons.iter().any(|b| b.is_some()) {
        pointer_top(false);
        pointer_left(false);
    }
    for (bn, who) in balloons.into_iter().enumerate() {
        // TimeCompress(0) 0x11DB3 at the top of ShowMercMessage? 0x299B8 and SectorLine? 0x297D4: WaitLeftUp + WaitRightUp
        // come before the voice, the Lock and the picture (live, #31 comment 11937). Only the timer runs in this wait.
        let mut release_ms = 0;
        if who.is_some() {
            let ms = events.mouse_state();
            if ms.left() || ms.right() {
                if bn == 0 {
                    play.host.sound.speech_stop();
                }
                let begin = ja1front::dostime::game_ticks(start.elapsed());
                let mut mixed = begin;
                let (pl, cur) = (&mut *play, &mut *screen);
                release_ms = speech_buttons_up(canvas, events, true, true, |at| {
                    let now = ja1front::dostime::game_ticks(start.elapsed());
                    pl.mouse_px = at;
                    pl.cursor = cur.cursor;
                    if in_pass {
                        // The library's timer runs now, in the wait; no session inside a pass: the window runs the
                        // front-end's timers of these ticks after it (`balloon_owed`).
                        let n = now.saturating_sub(*ticks_done);
                        pl.host.timer_in_pass(n, false);
                        pl.host.balloon_owed.0 += n;
                    } else {
                        pl.drag_ticks(&mut e.world, now.saturating_sub(*ticks_done));
                    }
                    cur.cursor = pl.cursor;
                    *ticks_done = now;
                    if now > mixed {
                        pl.host.sound.pass((now - mixed) as u32);
                        mixed = now;
                        feed_audio(audio, &mut pl.host.sound);
                    }
                });
                if release_ms > 0 {
                    eprintln!("balloon release start {}", release_ms);
                }
            }
        }
        // PlayHost::say voices only the first balloon of a pass; the others speak when they show.
        if let (true, Some((rec, q, _, nat, _, _))) = (ja1front::balloon::voice_when_shown(bn, release_ms), &who) {
            play.host.reaction_voice(rec[0x179], *q as i32, *nat);
        }
        // SectorVictory? 0x3A0B0 stops the combat only after this wait, and no frame comes in the wait (live, #2): the
        // picture under the quote of the last enemy's death still has DONE. The port runs that stop inside the pass.
        let won = ja1front::balloon::victory_quote_holds_combat(who.as_ref().map(|w| w.1 as i32), e.world.combat, e.world.update.turn_clock);
        if won {
            e.world.combat = 1;
        }
        // The view move of the balloon before this one can change the view.
        let mut right = false;
        // ShowMercMessage? 0x299B8: the balloon shows over the old view; the game state already holds the view move.
        let moved = world_view(&e.world);
        if let Some((_, _, _, _, Some(u), _)) = &who {
            set_world_view(&mut e.world, &u.before);
        }
        let (c0, r0) = ja1front::enter::view_tile(&e.world);
        held_gone(play, &e.world, ja1front::pointer::held_gone(screen.cursor.over_map));
        // 0x299B8 hides the path marks after its wait: the picture under the balloon still has them.
        let marks: &[(usize, u8)] = if let Some((_, _, _, _, _, m)) = &who { m } else { &[] };
        ja1front::sighting::show_marks(&mut e.world, marks, true);
        if !marks.is_empty() {
            eprintln!("balloon marks {}", marks.len());
        }
        // The frames get a held item one by one (`held_in_balloon_frame`): the picture under them has none.
        let held_was = play.held_off;
        play.held_off = true;
        let idx = play_frame(iso, exe, e, screen, c0, r0, play, false);
        play.held_off = held_was;
        ja1front::sighting::show_marks(&mut e.world, marks, false);
        if won {
            e.world.combat = 0;
        }
        let (mut v, wait) = match &who {
            Some((rec, q, _, nat, _, _)) => {
                eprintln!("balloon {} {}{}", rec[0x17A], if *nat { "nat " } else { "" }, q);
                ja1front::balloon::reaction_screens(iso, exe, rec, *q as i32, *nat, ja1front::ctlpanel::sub_byte(e.world.update.opt_step_text), &idx)
            }
            None => {
                eprintln!("balloon santino");
                ja1front::balloon::santino_screens(iso, exe, &ja1front::balloon::santino_text(iso, exe, 0), &idx)
            }
        }
        .unwrap_or_else(|c| fail("speech balloon", c));
        // SpeechBalloon? 0x29589: HideMouse?. SpeechWait? 0x283A8 (argument 1): ShowMouse?, also over the map. 0x29765:
        // HidePointer? before the face slides out. 0x297BB: MouseRedraw? at the end.
        let held_item = ja1front::pointer::held_is_pointer(e.world.held[0]);
        pointer_hold_start(screen.cursor.over_map);
        if held_item {
            let over = screen.cursor.over_map;
            let shown: Vec<usize> = (0..v.len()).filter(|&n| ja1front::pointer::held_in_balloon_frame(n, over)).collect();
            eprintln!("balloon held frames {:?} of {}", shown, v.len());
            for &n in &shown {
                ja1front::pointer::held_on_screen(iso, exe, &e.world, &mut v[n], play.mouse_px.0, play.mouse_px.1)
                    .unwrap_or_else(|c| fail("pointer", c));
            }
        }
        for (n, f) in v.iter().enumerate() {
            pointer_visible(n == 4 && !held_item);
            blit(tex, f, pal, SW, SH);
            canvas.copy(tex, None, None).expect("copy");
            canvas.present();
            // SpeechBalloon? 0x296DA: WaitLeftUp between the face's way in and SpeechWait?.
            if n == 4 {
                let mut mixed = ja1front::dostime::game_ticks(start.elapsed());
                let sound = &mut play.host.sound;
                let ms = speech_buttons_up(canvas, events, true, false, |_| {
                    let now = ja1front::dostime::game_ticks(start.elapsed());
                    if now > mixed {
                        sound.pass((now - mixed) as u32);
                        mixed = now;
                        feed_audio(audio, sound);
                    }
                });
                if ms > 0 {
                    eprintln!("balloon release middle {}", ms);
                }
            }
            let shown = Instant::now();
            let ms = if n == 4 { wait as u64 } else { ja1front::balloon::FRAME_MS as u64 };
            // panel.md step 7 / sound.md "Merc quotes": with a voice the wait ends when the voice ends.
            let voiced = ja1front::balloon::held_voiced(n, 4, play.host.sound.speaking());
            let mut mixed = 0u128;
            // panel.md "Face animation": the face blinks and talks during the wait.
            let mut face = if n == 4 {
                let lip = play.host.sound.speech_lip();
                ja1front::balloon::last_face().map(|sp| ja1front::faceanim::WaitFace::new(sp, lip.as_ref().map(|l| l.0.as_slice()), start.elapsed().as_nanos() as u32 ^ 0x5EED))
            } else {
                None
            };
            // panel.md "Speech balloon" step 7: the weapon text shows over the face while the mouse is on it.
            let wtext = match &who { Some((_, _, t, _, _, _)) if weapon_text_frame(n) => t.clone(), _ => String::new() };
            // SectorLine? 0x29925 and ShowMercMessage? 0x29BDC pass 800 as the last SpeechBalloon? argument: the wait holds
            // 800 ms after the voice (or the text time) ends (live, #2: voice end -> combat end 1.08 s at the last enemy's
            // death). SantinoSpeech? passes 0.
            let hold = if who.is_some() { 800 } else { 0 };
            let rules = ja1front::faceanim::WaitRules { text_ms: ms as i64, voiced, hold, subtitles: ja1front::ctlpanel::sub_byte(e.world.update.opt_step_text),
                                                        has_text: !wtext.is_empty() };
            let mut wait_rules = face.take().map(|wf| ja1front::faceanim::SpeechWait::new(wf, rules));
            let mut cur = f.clone();
            'wait: while balloon_waits(wait_rules.is_some(), shown.elapsed(), ms) {
                let mut input = ja1front::faceanim::WaitInput::default();
                for ev in events.poll_iter() {
                    match ev {
                        Event::KeyDown { keycode: Some(Keycode::Return | Keycode::Escape | Keycode::Space), .. } if n == 4 => input.key = true,
                        Event::MouseButtonDown { mouse_btn, .. } if n == 4 => match mouse_btn {
                            sdl2::mouse::MouseButton::Left => input.left = true,
                            _ => input.right = true,
                        },
                        Event::Quit { .. } => return false,
                        _ => {}
                    }
                }
                if let Some(sw) = wait_rules.as_mut() {
                    let ms_ = events.mouse_state();
                    (input.x, input.y) = screen_pixel(canvas, ms_.x(), ms_.y());
                    // SpeechWait? 0x2817D has its own MousePoll?, also with 0x92DCC at 0: the slide behind the balloon saves this place.
                    play.mouse_px = (input.x, input.y);
                    ja1front::pointer::mouse_poll(play.mouse_px);
                    input.left = seen_or_held(input.left, ms_.left());
                    input.right = seen_or_held(input.right, ms_.right());
                    input.now = shown.elapsed().as_millis() as i64;
                    if let Some((_, us)) = play.host.sound.speech_lip() {
                        input.played_us = us;
                    }
                    input.playing = play.host.sound.speaking();
                    let step = sw.pass(&input);
                    if step.redraw {
                        cur.clone_from(f);
                        sw.face.spot.paint(&mut cur, sw.face.anim.e, sw.face.anim.m);
                        if step.text {
                            eprintln!("balloon text {}", wtext);
                            ja1front::balloon::wait_text(iso, exe, &mut cur, &sw.face.spot, wtext.as_bytes()).unwrap_or_else(|c| fail("speech balloon", c));
                        } else if rules.has_text {
                            eprintln!("balloon text off");
                        }
                        blit(tex, &cur, pal, SW, SH);
                        canvas.copy(tex, None, None).expect("copy");
                        canvas.present();
                    }
                    if step.end.is_some() {
                        right = step.end == Some(ja1front::faceanim::WaitEnd::Right);
                        eprintln!("balloon wait end {:?} at {} quiet {:?} hold {}", step.end.unwrap(), input.now, sw.quiet_since(), hold);
                        break 'wait;
                    }
                } else if any_input(input.key, input.left, input.right) {
                    right = input.right;
                    break 'wait;
                }
                let due = ja1front::dostime::game_ticks(shown.elapsed()) as u128;
                if due > mixed {
                    play.host.sound.pass((due - mixed) as u32);
                    mixed = due;
                    feed_audio(audio, &mut play.host.sound);
                }
                std::thread::sleep(Duration::from_millis(5));
            }
            let face = wait_rules.map(|sw| sw.face);
            face_note(&face);
            // The end of SpeechWait? 0x28682: WaitLeftUp + WaitRightUp, and the picture stays until the release.
            if n == 4 {
                let mut mixed = ja1front::dostime::game_ticks(start.elapsed());
                let sound = &mut play.host.sound;
                let px = &mut play.mouse_px;
                let ms = speech_buttons_up(canvas, events, true, true, |at| {
                    *px = at;
                    let now = ja1front::dostime::game_ticks(start.elapsed());
                    if now > mixed {
                        sound.pass((now - mixed) as u32);
                        mixed = now;
                        feed_audio(audio, sound);
                    }
                });
                if ms > 0 {
                    eprintln!("balloon release end {}", ms);
                }
            }
            let at = ja1front::pointer::game_place();
            eprintln!("mouse at balloon end {} {}", at.0, at.1);
        }
        pointer_hold_end(true, screen.cursor.over_map, tactical_ready(e.world.update.screen_drawn), held_item);
        held_gone(play, &e.world, false);
        // panel.md "Other users of Lock": the balloon Locked the game while it showed.
        let now = ja1front::dostime::game_ticks(start.elapsed());
        if pass_owes_ticks(locked, in_pass) {
            let n = now.saturating_sub(*ticks_done);
            play.host.timer_in_pass(n, true);
            play.host.balloon_owed.1 += n;
        } else if locked {
            play.locked_ticks(&mut e.world, false, now.saturating_sub(*ticks_done), false);
        }
        *ticks_done = now;
        // The view moves after the wait, and byte 0x92F65 (the right button) skips that move.
        if let Some((_, _, _, _, Some(u), _)) = &who {
            let before = (u.before.view_x, u.before.view_y);
            if right_undoes_move(right, u.right_skips) {
                // A later focus of the same pass stays: only the message's own move goes back.
                if moved != u.after {
                    set_world_view(&mut e.world, &moved);
                }
            } else {
                set_world_view(&mut e.world, &moved);
                if let Some(sl) = u.slide {
                    ja1front::focus::push_slide(sl);
                }
            }
            eprintln!("balloon view {} {} {} {} {}", if right && u.right_skips { "stays" } else { "moves" }, before.0, before.1, e.world.view_x, e.world.view_y);
            // Inside a pass the slide waits in the list: the window plays it after the pass.
            if !ja1front::balloon::goes_on_after_view_move(in_pass, || window_slides(iso, exe, e, screen, play, canvas, tex, events, pal, ticks_done)) {
                return false;
            }
        }
    }
    true
}

/// The view slides of the last pass or balloon, one frame per step (SlideViewToTile? 0x3F683). False when the window closed.
#[cfg(feature = "gui")]
#[allow(clippy::too_many_arguments)]
fn window_slides(iso: &str, exe: &str, e: &mut ja1front::enter::Entered, screen: &mut ja1front::screen::Screen,
                 play: &mut ja1front::play::Play, canvas: &mut Frame, tex: &mut sdl2::render::Texture,
                 events: &mut sdl2::EventPump, pal: &[u8; 768], ticks_done: &mut u64) -> bool {
    let slides = ja1front::focus::take_slides();
    for sl in &slides {
        eprintln!("window slide {} steps {} ticks", sl.steps, sl.ticks());
        let end = (e.world.view_x, e.world.view_y);
        // SlideViewToTile? 0x3F683: HideMouse?, MouseSavePos2?; the pointer stays off while the view slides.
        let hold = pointer_hold_start(screen.cursor.over_map);
        held_gone(play, &e.world, ja1front::pointer::held_gone(screen.cursor.over_map));
        // MouseSavePos2? 0x67E18 saves the game's place 0x229D00: the last MousePoll?, not the driver's place (#31 evidence).
        let mouse_at = ja1front::pointer::game_place();
        ja1front::pointer::mouse_save(mouse_at);
        eprintln!("mouse saved {} {}", mouse_at.0, mouse_at.1);
        let first = sl.ticks() + 5 - 5 * sl.steps;
        let t0 = Instant::now();
        for i in 1..=sl.steps {
            let due = Duration::from_millis(10 * (first + 5 * (i - 1)) as u64);
            while t0.elapsed() < due {
                for ev in events.poll_iter() {
                    if let Event::Quit { .. } = ev {
                        return false;
                    }
                }
                std::thread::sleep(Duration::from_millis(2));
            }
            (e.world.view_x, e.world.view_y) = sl.at(i);
            let (c, r) = ja1front::enter::view_tile(&e.world);
            let idx = play_frame(iso, exe, e, screen, c, r, play, false);
            blit(tex, &idx, pal, SW, SH);
            canvas.copy(&tex, None, None).expect("copy");
            canvas.present();
        }
        (e.world.view_x, e.world.view_y) = end;
        *ticks_done += sl.ticks() as u64;
        // 0x3F70E, 0x3F713: MouseRestorePos2? puts the mouse back to its place at the start, then MouseRedraw?.
        let ms = events.mouse_state();
        if screen_pixel(canvas, ms.x(), ms.y()) != mouse_at && !unsafe { sdl2::sys::SDL_GetMouseFocus() }.is_null() {
            eprintln!("mouse back {} {}", mouse_at.0, mouse_at.1);
            play.mouse_px = mouse_at;
            play.mouse_warp = Some(mouse_at);
        }
        pointer_hold_end(hold, screen.cursor.over_map, e.world.update.screen_drawn != 0, ja1front::pointer::held_is_pointer(e.world.held[0]));
        held_gone(play, &e.world, false);
        eprintln!("window slide end {} ms", t0.elapsed().as_millis());
    }
    play.slide_ticks(&mut e.world, &slides);
    true
}

/// Play in the window `g`; returns true when the office's Leave ended the game (the main menu opens again), false
/// when the window closed.
#[cfg(feature = "gui")]
#[allow(clippy::too_many_arguments)]
fn play_window(iso: &str, exe: &str, title: &str, e: &mut ja1front::enter::Entered, screen: &mut ja1front::screen::Screen, pal0: &[u8; 768], mut c0: i32, mut r0: i32, save_dir: Option<std::path::PathBuf>, g: Gfx, from_office: bool, quick_start: bool) -> (bool, Gfx) {
    use ja1front::play::{Input, Play};
    // The palette and the title change when the day loop enters a new sector (world_load.md section 9).
    let mut pal: [u8; 768] = *pal0;
    let mut title: String = title.to_string();
    use ja1front::screen::{popup_press, spot, Spot};
    use sdl2::keyboard::Mod;
    use sdl2::mouse::MouseButton;
    use std::time::{Duration, Instant};

    let Gfx { sdl, mut canvas, mut events, music } = g;
    canvas.window_mut().set_title(&title).ok();
    // sound.md "Output": the mix goes to an SDL audio queue (22050 Hz, mono, 16 bits); no audio device: no sound.
    let audio = sdl.audio().and_then(|a| {
        let want = sdl2::audio::AudioSpecDesired { freq: Some(ja1front::sound::RATE as i32), channels: Some(1), samples: Some(512) };
        a.open_queue::<i16, _>(None, &want)
    });
    let audio = match audio {
        Ok(q) => {
            q.resume();
            eprintln!("audio on ({} Hz)", q.spec().freq);
            Some(q)
        }
        Err(e) => {
            eprintln!("audio off (no audio device: {})", e);
            None
        }
    };
    let tc = canvas.texture_creator();
    let mut tex = tc.create_texture_streaming(PixelFormatEnum::RGB24, SW, SH).expect("texture");
    let mut to_menu = false;
    // Random numbers of play go on from the seed at the end of the sector entry (section 8).
    let mut play = Play::new(e.seed);
    play.hover_per_tick = 20;
    play.host.voice_later = true;
    ja1front::ctlpanel::put_sound_switches(&mut e.world, [1, ja1front::music::with(|m| m.on) as u8]);
    play.day.save_dir = save_dir;
    // End-of-day screen (section 8): its frames go to this window (the sink is bound before each pass).
    play.host.eod = Some(ja1front::eod::EodHost::new(iso, exe, screen.table0, None));
    // The sunrise screen (section 9 step 3): its frames go to this window (the sink is bound for the day steps).
    play.host.sunrise = Some(ja1front::sunrise::Sunrise::new(iso, exe, screen.table0));
    let start = Instant::now();
    let mut drag_keys: Vec<Event> = Vec::new();
    let mut drag_drawn = None;
    let mut ticks_done: u64 = 0;
    let mut after_guard = false;
    // Op 345 disarm_ask (combat.md "Disarming an explosive"): the merc-face question box (PANEL_P.PCC) over the
    // tactical screen as last drawn; the game waits for the answer.
    let mut last_frame: Vec<u8> = vec![0; (SW * SH) as usize];
    {
        let cv = &mut canvas as *mut Frame;
        let tx = &mut tex as *mut sdl2::render::Texture<'_> as *mut sdl2::render::Texture<'static>;
        let ev = &mut events as *mut sdl2::EventPump;
        let (lf, pp) = (&mut last_frame as *mut Vec<u8>, &pal as *const [u8; 768]);
        let (ep, sp) = (e as *mut ja1front::enter::Entered, screen as *mut ja1front::screen::Screen);
        let (plp, tdp) = (&mut play as *mut Play, &mut ticks_done as *mut u64);
        let ap = &audio as *const Option<sdl2::audio::AudioQueue<i16>>;
        let (iso_s, exe_s) = (iso.to_string(), exe.to_string());
        play.host.fade_win = Some(Box::new(move |n: i32, from: i32, to: i32| {
            // SAFETY: as for disarm_win below: the pass runs inside this function's loop.
            unsafe { fade_show8(&mut *cv, &mut *tx, &*lf, &*pp, n, from, to) }
        }));
        let (iso_sc, exe_sc) = (iso.to_string(), exe.to_string());
        play.host.scene_win = Some(Box::new(move || {
            // SAFETY: as for disarm_win below: the pass runs inside this function's loop.
            unsafe {
                // The pass holds the state in the library's world: the pictures of the scene need its values.
                let cw = (*plp).host.world;
                (*cw).store(&mut (*ep).world);
                let pending: Vec<_> = std::mem::take(&mut (*plp).host.reactions).into_iter()
                    .filter_map(|r| r.balloon.map(|b| Some((b, r.how, r.text, r.nat, r.undo, r.marks)))).collect();
                let (c0, r0) = ja1front::enter::view_tile(&(*ep).world);
                if !pending.is_empty() && !show_balloons(&iso_sc, &exe_sc, &mut *ep, &mut *sp, c0, r0, &mut *plp, &mut *cv, &mut *tx,
                                                         &mut *ev, &*pp, (*ap).as_ref(), start, &mut *tdp, pending, false, false) {
                    (*plp).host.scene_closed = true;
                    return;
                }
                // A right click on a balloon keeps the old view: the pass goes on with it.
                let v = world_view(&(*ep).world);
                ((*cw).view_x, (*cw).view_y, (*cw).show_timer, (*cw).view_tile) = (v.view_x, v.view_y, v.show_timer, v.view_tile);
                ((*cw).spotted, (*cw).focus_slot, (*cw).focus_last, (*cw).shown_slot) = (v.spotted, v.focus_slot, v.focus_last, v.shown_slot);
                let (c0, r0) = ja1front::enter::view_tile(&(*ep).world);
                for (op, a) in std::mem::take(&mut (*plp).host.guide_said) {
                    let idx = play_frame(&iso_sc, &exe_sc, &mut *ep, &mut *sp, c0, r0, &mut *plp, false);
                    if !guide_balloon(&iso_sc, &exe_sc, op, &a, ja1front::ctlpanel::sub_byte((*ep).world.update.opt_step_text), &idx, &*pp, &mut *cv, &mut *tx, &mut *ev) {
                        (*plp).host.scene_closed = true;
                        return;
                    }
                }
                // RedrawChoice before FadePalette?: the fade shows the state of this moment.
                *lf = play_frame(&iso_sc, &exe_sc, &mut *ep, &mut *sp, c0, r0, &mut *plp, false);
                *tdp = ja1front::dostime::game_ticks(start.elapsed());
            }
        }));
        let (iso_b, exe_b) = (iso.to_string(), exe.to_string());
        play.host.balloon_win = Some(Box::new(move || {
            // SAFETY: as for disarm_win below: the pass runs inside this function's loop.
            unsafe {
                // ShowMercMessage? 0x299B8 waits inside the call: the picture shows the state of this moment, and a
                // right click keeps the old view for the rest of the pass.
                let cw = (*plp).host.world;
                (*cw).store(&mut (*ep).world);
                let pending: Vec<_> = std::mem::take(&mut (*plp).host.reactions).into_iter()
                    .filter_map(|r| r.balloon.map(|b| Some((b, r.how, r.text, r.nat, r.undo, r.marks)))).collect();
                // The slide of a balloon's view move plays after the balloon (and after the pass).
                for sl in pending.iter().flatten().filter_map(|b| b.4.as_ref().and_then(|u| u.slide)) {
                    ja1front::focus::hold_slide(&sl);
                }
                let (c0, r0) = ja1front::enter::view_tile(&(*ep).world);
                let mut td = ja1front::dostime::game_ticks(start.elapsed());
                if !show_balloons(&iso_b, &exe_b, &mut *ep, &mut *sp, c0, r0, &mut *plp, &mut *cv, &mut *tx,
                                  &mut *ev, &*pp, (*ap).as_ref(), start, &mut td, pending, true, true) {
                    (*plp).host.scene_closed = true;
                }
                let v = world_view(&(*ep).world);
                eprintln!("balloon in pass view {} {}", v.view_x, v.view_y);
                ((*cw).view_x, (*cw).view_y, (*cw).show_timer, (*cw).view_tile) = (v.view_x, v.view_y, v.show_timer, v.view_tile);
                ((*cw).spotted, (*cw).focus_slot, (*cw).focus_last, (*cw).shown_slot) = (v.spotted, v.focus_slot, v.focus_last, v.shown_slot);
                *lf = play_frame(&iso_b, &exe_b, &mut *ep, &mut *sp, c0, r0, &mut *plp, false);
            }
        }));
        play.host.disarm_win = Some(Box::new(move |text: &[u8], prof: u8, nick: &[u8]| -> bool {
            // SAFETY: the pass runs inside this function's loop; canvas, texture, events, frame and palette live
            // until it returns, and nothing else uses them during the pass.
            unsafe {
                // #36 B: the trap bark (op 54 merc_message 0x13) comes from combat.md "Disarming an explosive"
                // step 4, before the ask, and the original shows its balloon before the box. The front-end plays a
                // pass's balloons after the pass, so show the ones this pass queued before the box opens.
                let pending: Vec<_> = std::mem::take(&mut (*plp).host.reactions).into_iter()
                    .filter_map(|r| r.balloon.map(|b| Some((b, r.how, r.text, r.nat, r.undo, r.marks)))).collect();
                if !pending.is_empty() {
                    let (c0, r0) = ja1front::enter::view_tile(&(*ep).world);
                    if !show_balloons(&iso_s, &exe_s, &mut *ep, &mut *sp, c0, r0, &mut *plp, &mut *cv, &mut *tx,
                                      &mut *ev, &*pp, (*ap).as_ref(), start, &mut *tdp, pending, false, false) {
                        return false;
                    }
                    let (c0, r0) = ja1front::enter::view_tile(&(*ep).world);
                    // The box's saved background is the screen just after the balloon.
                    *lf = play_frame(&iso_s, &exe_s, &mut *ep, &mut *sp, c0, r0, &mut *plp, false);
                }
                let imgs = ja1front::question::face_screens(&iso_s, &exe_s, text, prof, nick, "PANEL_P.PCC", &*lf)
                    .unwrap_or_else(|c| fail("question box", c));
                eprintln!("window disarm asks (profile {})", prof);
                // #36 B: YesNoPrompt 0x28C84 calls ShowMouse (0x673AC) before its wait, so the game pointer shows
                // in the box; the tactical map hides the system pointer (the tile cursor shows the mouse there).
                pointer_visible(true);
                prompt_mouse(&*cv, &*ev, ja1front::ctlpanel::subtitles(&(*ep).world));
                question_loop(&mut *cv, &mut *tx, &mut *ev, &imgs, &*pp, None)
            }
        }));
    }
    let speed_log = std::env::var("JA1_SPEED_LOG").is_ok_and(|v| v == "1");
    let mut hover_seen: (i32, u128, bool) = (-1, 0, false);
    let mut last_blink: Option<u8> = None;
    let mut last_tile: Option<(usize, i16)> = None;
    let mut last_clock: Option<i32> = None;
    // Clicks and keys wait for the next pass with a tick; they run before that pass's ticks, in order.
    let mut inputs: Vec<Input> = Vec::new();
    let mut drawn_left = false;
    let mut last_step: Option<u32> = None;
    // The slot whose stat popup shows while the left button is held (the game stops meanwhile).
    let mut held: Option<u8> = None;
    // Key S (panel.md "Key S: all stat popups"): the slots whose popups show until a key or a mouse button.
    let mut stat_keys: Option<Vec<u8>> = None;
    let mut stat_wait = WindowWaitKey::new();
    // panel.md "The switch view": the slot whose switch view is up (the game is Locked meanwhile).
    let mut switch_ui: Option<SwitchUi> = None;
    let mut right_press: Option<(i32, i32)> = None;
    let mut left_held = false;
    let mut map_press = false;
    // StripClick 0x54F3C ends in WaitLeftUp or WaitRightUp: the button of that wait, 0 = none.
    let mut strip_wait = 0;
    // PressPanelButton 0x35DA4 stops the game until the release, once the pressed face shows.
    let mut strip_face_shown = false;
    // OverviewMouse 0x24630 reads the left button as a level and has no wait of its own: while the button stays down after a
    // press on the overview, each pass acts on the place under the mouse (live in the original, issue #31).
    let mut ov_press = false;
    // A left press in the enemies' turn: 0x141EC skips it, but the button is a level (live, issue #31).
    let mut turn_press = false;
    let mut mouse_reads = true;
    // Check aid (window_turn_win): the cursor tile at each change, `cursor tile T` on stderr.
    let mut tile_seen: Option<i32> = None;
    let tile_log = std::env::var_os("JA1_ENEMY_TURN_WIN").is_some();
    let mut shown_selected = e.world.selected;
    let mut shown_view = e.world.opt_onscreen;
    // The action of the top-strip button held down (done on release).
    let mut strip_action: Option<StripAction> = None;
    let mut strip_hold: Option<ja1front::pointer::ButtonHold> = None;
    // The island map (key M): the picture and its 8-bit palette while it shows.
    let mut map = ja1front::mapui::MapUi::new(iso, exe, e.seed);
    play.files = Some((iso.to_string(), exe.to_string()));
    play.host.explode = ja1front::iso::Iso::open(iso).ok().and_then(|mut i| i.read("DAT\\EXPLODE.DAT").ok());
    play.set_exe(exe).unwrap_or_else(|c| fail("JA.EXE", c));
    play.day.tileset = e.tileset;
    // Before the first day start of the run, "Restart this day" goes back to the world as loaded.
    play.day.day_save = Some(e.loaded.clone());
    if from_office {
        // world_load.md sections 11 and 12: a game from the main menu starts at the office (step 2).
        play.left = true;
        play.day.from_office = true;
    }
    if quick_start {
        // world_load.md section 12 "Continue last day in progress?": the day goes on from QUICK.SAV (Quick Restore
        // steps 2..7 with no question). Restart this day and Abort this day take the day files of the save folder.
        match play.continue_day(&mut e.world, iso, exe) {
            Ok(t) => eprintln!("quick restore day {} sector {} theme {}", e.world.day, e.world.cur_sector, t),
            Err(m) => die(m),
        }
    }
    // strategic.md "Start the day": the start button of the day loop's map is held down.
    let mut start_down = false;
    play.host.sound.live = audio.is_some();
    // MergeItemsPrompt 0x4AA26 on the inventory and assignment screens: asked inside the library's press.
    {
        let cv = &mut canvas as *mut Frame;
        let tx = &mut tex as *mut sdl2::render::Texture<'_> as *mut sdl2::render::Texture<'static>;
        let ev = &mut events as *mut sdl2::EventPump;
        let (miso, mexe) = (iso.to_string(), exe.to_string());
        let (tp, mp) = (&pal as *const [u8; 768], &map.pal as *const [u8; 768]);
        ja1front::question::set_merge_wait(Some(Box::new(move |base, items, on_map| -> bool {
            eprintln!("merge asks");
            let imgs = ja1front::question::merge_screens(&miso, &mexe, base, &items)
                .unwrap_or_else(|c| fail("question box", c));
            // SAFETY: the press runs inside play_window, and nothing else uses these meanwhile (as for guide_ask).
            unsafe {
                let p = if on_map { *mp } else { *tp };
                let yes = question_box(&mut *cv, &mut *tx, &mut *ev, &imgs, &p);
                eprintln!("merge answer {}", yes as i32);
                yes
            }
        })));
    }
    'outer: loop {
        feed_audio(audio.as_ref(), &mut play.host.sound);
        let mut open_inv = false;
        let mut open_map = false;
        let mut guard_leave = false;
        let mut start_day = false;
        // 0 = Enter, 1 = the start button (the argument of ja1_map_start).
        let mut start_button = 0;
        let mut ask_abort = false;
        let mut ask_quit = false;
        // Alt+S (true) / Alt+R (false): the quick save keys of the tactical screen.
        let mut quick_key: Option<bool> = None;
        // panel.md "Exit to DOS" (argument 1): yes to Alt-X's question; Quick Save first, then the quit line.
        let mut exit_dos = false;
        let mut exit_zero = false;
        let mut map_quit = false;
        // panel.md "The Control Panel" (F10, `opt`).
        let mut open_opt = false;
        // soldier_update.md "Traverse to a neighbour sector": the popup's corner (T key or the TRAV button).
        let mut ask_trav: Option<((i32, i32), bool)> = None;
        let mut polled = false;
        if map.open {
            frame::set_limit(map.limit);
        }
        // AllMoveTraverse? 0x1CFEC reads no key: while its loop runs, key presses stay in the BIOS queue of 15 keys, and
        // the game reads them after the release.
        // KeyOrClick 0x67E44 (a press in a message box, up to its release) and PlayInvExtra? 0x25AD8 (the rest of the sound)
        // read no key either.
        let keys_wait = |map: &ja1front::mapui::MapUi, play_drag: bool| play_drag || (map.open && (map.box_pressed() || map.sound_held()));
        // UseItemAt 0x1E..: every press on the map ends in WaitLeftUp 0x67C54, so the whole game stops until the release
        // (live in the original, issue #31). A press that starts an aim has its own wait.
        // A box in the click (box_wait: "doesn't have enough Action Points!") takes the release event, so read the
        // level, as WaitLeftUp does. Without this the pass stayed in WaitLeftUp after the box: no hover, the right
        // button ignored, and the next press clicked again (the maintainer play 2026-11-10, #36 A).
        {
            let ms = events.mouse_state();
            let (left_up, both_up) = release_net(ms.left(), ms.right());
            if left_up {
                left_held = false;
            }
            if both_up {
                play.mouse_held = false;
            }
        }
        if map_press && (!left_held || play.wait.is_some() || play.drag.is_some()) {
            map_press = false;
            eprintln!("map press hold 0");
        }
        // The button is a level: a box in the click can take the release event (live in the original, tac60).
        if std::mem::take(&mut play.face_press) {
            play.host.wait_up = 1;
        }
        if play.host.wait_up != 0 {
            strip_wait = std::mem::take(&mut play.host.wait_up);
            eprintln!("strip wait {}", strip_wait);
        }
        if strip_wait != 0 {
            let ms = events.mouse_state();
            if !(if strip_wait == 1 { ms.left() } else { ms.right() }) {
                strip_wait = 0;
                eprintln!("strip wait 0");
            }
        }
        let map_press_now = map_press || strip_wait != 0;
        // WaitRightUp at the end of the right arm of UseItemAt 0x1D9B8: keys act after the release too (live, issue #31).
        let mut evq: Vec<Event> = if keys_wait(&map, keys_held(play.drag.is_some(), map_press_now, right_press.is_some(), matches!(play.wait, Some(ja1front::cursor::ClickWait::Hold(_))))) { Vec::new() } else { std::mem::take(&mut drag_keys) };
        evq.extend(events.poll_iter());
        // A box in a right hold (the turn's "doesn't have enough Action Points!" at the left release) takes the right
        // release too. The hold polls the level (right step 3), so a hold whose button is up ends here: without this
        // every later mouse event stayed a poll of the hold (the LOOK cursor stood still, a right click did nothing).
        if right_press.is_some() && !events.mouse_state().right()
            && !evq.iter().any(|ev| matches!(ev, Event::MouseButtonUp { mouse_btn: MouseButton::Right, .. })) {
            let (ww, wh) = canvas.window().size();
            let (x, y) = frame::window_of(ww, wh, play.mouse_px.0, play.mouse_px.1);
            eprintln!("right up by level");
            evq.push(Event::MouseButtonUp { timestamp: 0, window_id: 0, which: 0, mouse_btn: MouseButton::Right, clicks: 1, x, y });
        }
        if turn_press {
            // The first pass of the mercs' turn reads the button that is still down, at the mouse place of the turn's end.
            if !events.mouse_state().left() {
                turn_press = false;
                left_held = false;
            } else if ja1front::slotmouse::pass_runs(e.world.update.screen_drawn) && !play.host.mouse_hold && !play.left && play.wait.is_none()
                // A queued reaction runs first and takes the release (ShowMercMessage? 0x299B8, live: #31 comment 11937).
                && play.host.sight.slot >= 8
                && !map.open && held.is_none() && stat_keys.is_none() && switch_ui.is_none() && right_press.is_none() {
                turn_press = false;
                let (ww, wh) = canvas.window().size();
                let (x, y) = frame::window_of(ww, wh, play.mouse_px.0, play.mouse_px.1);
                eprintln!("turn press at {} {}", play.mouse_px.0, play.mouse_px.1);
                evq.push(Event::MouseButtonDown { timestamp: 0, window_id: 0, which: 0, mouse_btn: MouseButton::Left, clicks: 1, x, y });
            }
        }
        if ov_press {
            // A box in this press takes its own release, so the event does not come here: read the level, as the original does.
            let down = events.mouse_state().left();
            if !down {
                left_held = false;
            }
            let up = evq.iter().any(|ev| matches!(ev, Event::MouseButtonUp { mouse_btn: MouseButton::Left, .. }));
            if up || !down || !left_held || e.world.opt_onscreen != 0 || map.open || play.wait.is_some() || play.drag.is_some() {
                ov_press = false;
            } else if ja1front::slotmouse::pass_runs(e.world.update.screen_drawn) && !play.left && !play.pause.paused
                && e.world.update.time_compress == 0 && held.is_none() && stat_keys.is_none() && switch_ui.is_none() && right_press.is_none() {
                ov_press = false;
                let (ww, wh) = canvas.window().size();
                let (x, y) = frame::window_of(ww, wh, play.mouse_px.0, play.mouse_px.1);
                evq.push(Event::MouseButtonDown { timestamp: 0, window_id: 0, which: 0, mouse_btn: MouseButton::Left, clicks: 1, x, y });
            }
        }
        for ev in evq {
            if keys_wait(&map, keys_held(play.drag.is_some(), map_press_now, right_press.is_some(), matches!(play.wait, Some(ja1front::cursor::ClickWait::Hold(_))))) && matches!(ev, Event::KeyDown { .. }) {
                if drag_keys.len() < 15 {
                    drag_keys.push(ev);
                }
                continue;
            }
            // PlayInvExtra? 0x25AD8: no hover during the rest of the sound; a button down ends the wait and counts.
            if map.open && map.sound_held() {
                match ev {
                    Event::MouseMotion { .. } => continue,
                    Event::MouseButtonDown { .. } => {
                        map.sound_hold_over(true);
                    }
                    _ => {}
                }
            }
            // Section 8 "Hover": no hover step while a mouse button is held.
            match ev {
                Event::MouseButtonDown { mouse_btn, x, y, .. } => {
                    if std::env::var_os("JA1_CLICK_LOG").is_some() {
                        eprintln!("press at {:?}", screen_pixel(&canvas, x, y));
                    }
                    play.mouse_held = true;
                    left_held |= mouse_btn == MouseButton::Left;
                }
                Event::MouseButtonUp { mouse_btn, .. } => {
                    play.mouse_held = false;
                    left_held &= mouse_btn != MouseButton::Left;
                }
                _ => {}
            }
            // panel.md "Key S: all stat popups": a key, or a mouse button when it comes up, ends them and does nothing
            // else; the game was Locked (clock switch 1) meanwhile.
            if stat_keys.is_some() {
                if stat_wait.event(&ev) {
                    stat_keys = None;
                    eprintln!("window stat popups end");
                    let now = ja1front::dostime::game_ticks(start.elapsed());
                    play.locked_ticks(&mut e.world, true, now.saturating_sub(ticks_done), false);
                    ticks_done = now;
                }
                if !matches!(ev, Event::Quit { .. }) {
                    continue;
                }
            }
            // panel.md "The switch view": only the mouse buttons act; the mouse stays inside the slot.
            if let Some(sw) = switch_ui.as_mut() {
                match ev {
                    Event::MouseButtonDown { mouse_btn: MouseButton::Left, x, y, .. } if sw.step == SwStep::Up => {
                        let (px, py) = screen_pixel(&canvas, x, y);
                        if let Some(i) = ja1front::panel::switch_at(sw.k, px, py) {
                            play.switch_press(&mut e.world, sw.k, i);
                            sw.step = SwStep::Press(i);
                        }
                    }
                    Event::MouseButtonUp { mouse_btn: MouseButton::Left, .. } => {
                        if let SwStep::Press(i) = sw.step {
                            play.switch_release(&mut e.world, sw.k, i);
                            sw.step = SwStep::Up;
                        }
                    }
                    Event::MouseButtonDown { mouse_btn: MouseButton::Right, .. } if sw.step == SwStep::Up => {
                        sw.step = SwStep::WaitRight;
                    }
                    Event::MouseButtonUp { mouse_btn: MouseButton::Right, .. } if sw.step == SwStep::WaitRight => {
                        sw.step = SwStep::Close;
                        sw.t = Instant::now();
                        eprintln!("switch {} closed flags {:?}", sw.k,
                                  &e.world.soldiers[sw.k as usize * ja1front::soldier::REC + 0x16A..sw.k as usize * ja1front::soldier::REC + 0x16D]);
                    }
                    Event::MouseMotion { x, y, .. } => {
                        let (px, py) = screen_pixel(&canvas, x, y);
                        let (bx, by) = ja1front::panel::slot_origin(sw.k);
                        let (cx, cy) = (px.clamp(bx + 2, bx + 0x2F), py.clamp(by, by + 0x29));
                        if (cx, cy) != (px, py) {
                            warp_frame(&sdl.mouse(), &canvas, cx, cy);
                        }
                    }
                    _ => {}
                }
                if !matches!(ev, Event::Quit { .. }) {
                    continue;
                }
            }
            // cursor.md "ALL-move": the drag loop reads the right button as a level (below); no hold starts.
            if play.drag.is_some() && matches!(ev, Event::MouseButtonDown { mouse_btn: MouseButton::Right, .. } | Event::MouseButtonUp { mouse_btn: MouseButton::Right, .. }) {
                continue;
            }
            // cursor.md "Clicks", right step 3: while the right button is held on the map, each mouse event is a poll
            // of the hold (the left button, the mouse y); the release ends it.
            if let Some((tile, _)) = right_press {
                let y = match ev {
                    Event::MouseMotion { x, y, .. } | Event::MouseButtonDown { x, y, .. } | Event::MouseButtonUp { x, y, .. } =>
                        Some(screen_pixel(&canvas, x, y).1),
                    _ => None,
                };
                if let Some(py) = y {
                    let up = matches!(ev, Event::MouseButtonUp { mouse_btn: MouseButton::Right, .. });
                    inputs.push(Input::Poll { left: left_held, right: !up, y: py });
                    right_press = if up { None } else { Some((tile, py)) };
                    polled = true;
                    continue;
                }
            }
            match ev {
                Event::Quit { .. } => break 'outer,
                // panel.md "Message box": while a box shows on the map, keys and new mouse presses only end it.
                Event::KeyDown { .. } if map.open && map.box_shown() => {
                    // ShowMessage? 0x27C70 reads with KeyOrClick?: byte 0x95DC3 takes the key.
                    note_last_key(&ev);
                    eprintln!("map box key, last key {}", LAST_KEY.with(|v| v.get()));
                    map.box_input(BoxInput::Key).unwrap_or_else(|c| fail("message box", c));
                }
                Event::MouseButtonDown { mouse_btn, .. } if map.open && map.box_shown() => {
                    let n = box_button(mouse_btn);
                    map.box_input(BoxInput::Down(n)).unwrap_or_else(|c| fail("message box", c));
                }
                // The release of a press made while the box showed ends it (the release of the click that opened
                // it goes on to the map).
                Event::MouseButtonUp { mouse_btn, .. } if map.open && map.box_pressed() => {
                    let n = box_button(mouse_btn);
                    map.box_input(BoxInput::Up(n)).unwrap_or_else(|c| fail("message box", c));
                }
                // panel.md "Exit to DOS" (argument 0): Alt-X on the island map or the assignment screen.
                Event::KeyDown { keycode: Some(Keycode::X), keymod, .. }
                    if map.open && keymod.intersects(Mod::LALTMOD | Mod::RALTMOD) =>
                {
                    map_quit = map.alt_x(&e.world).unwrap_or_else(|c| fail("map", c));
                }
                // The map view (key M): T / P / G = mode 0 / 1 / 2, Enter = start the day, M or Escape = leave.
                Event::KeyDown { keycode: Some(k), .. } if map.open => {
                    let c = map_key_byte(k);
                    if map.key(&mut e.world, c).unwrap_or_else(|c| fail("map key", c)) == ja1front::mapui::Key::Start {
                        start_day = true;
                    }
                }
                Event::MouseMotion { x, y, .. } if map.open => {
                    map.track(&mut e.world, screen_pixel(&canvas, x, y)).unwrap_or_else(|c| fail("map mouse", c));
                }
                Event::MouseButtonDown { mouse_btn: MouseButton::Left, x, y, .. }
                    if map.open && map.start_button_at(screen_pixel(&canvas, x, y)) =>
                {
                    // The button routine 0x67CD8(7, 1): the press sound, the pressed face while it is held.
                    start_down = true;
                    strip_hold = Some(ja1front::pointer::ButtonHold::press(screen_pixel(&canvas, x, y)));
                    ja1front::sound::screen_button(iso, exe, 0);
                    map.start_down = true;
                    map.redraw(&e.world).unwrap_or_else(|c| fail("map", c));
                }
                Event::MouseButtonUp { mouse_btn: MouseButton::Left, .. } if map.open && start_down => {
                    // The start button counts when the button goes up (wherever the mouse is then), after the
                    // release sound.
                    start_down = false;
                    map.start_down = false;
                    hold_end(&canvas, strip_hold.take());
                    ja1front::sound::screen_button(iso, exe, 1);
                    eprintln!("map start button up");
                    start_day = true;
                    start_button = 1;
                }
                Event::MouseButtonDown { mouse_btn: btn @ (MouseButton::Left | MouseButton::Right), x, y, .. } if map.open => {
                    let at = screen_pixel(&canvas, x, y);
                    map.track(&mut e.world, at).unwrap_or_else(|c| fail("map mouse", c));
                    let buttons = if btn == MouseButton::Left { 1 } else { 2 };
                    // The day loop's map: the random numbers are play's (one seed for the whole game).
                    if map.day {
                        *map.rng() = ja1front::host::Rng { seed: play.host.rng.seed, draws: play.host.rng.draws };
                    }
                    let ms = sdl.keyboard().mod_state();
                    map.keys = (shift_down(ms), ctrl_down(ms));
                    map.click(&mut e.world, buttons).unwrap_or_else(|c| fail("map click", c));
                    if let Some((sx, sy)) = map.warp.take() {
                        // strategic.md "The duty menu", Open step 6: the mouse moves.
                        warp_frame(&sdl.mouse(), &canvas, sx, sy);
                    }
                    if map.day {
                        play.host.rng.seed = map.rng().seed;
                        play.host.rng.draws = map.rng().draws;
                    }
                }
                Event::MouseButtonUp { mouse_btn: MouseButton::Left, x, y, .. } if map.open => {
                    map.track(&mut e.world, screen_pixel(&canvas, x, y)).unwrap_or_else(|c| fail("map mouse", c));
                    map.release(&mut e.world, 1).unwrap_or_else(|c| fail("map release", c));
                    if let Some((sx, sy)) = map.warp.take() {
                        warp_frame(&sdl.mouse(), &canvas, sx, sy);
                    }
                    start_day |= map_kept_keys(&mut map, &mut e.world, &mut map_quit);
                }
                Event::MouseButtonUp { mouse_btn: MouseButton::Right, .. } if map.open => {
                    map.release(&mut e.world, 2).unwrap_or_else(|c| fail("map release", c));
                    start_day |= map_kept_keys(&mut map, &mut e.world, &mut map_quit);
                }
                ref b if map_eats_button(b, map.open) => {}
                // cursor.md "Clicks": Escape answers an open aim or confirm; else it stops the selected merc
                // (section 8 "Quit"; there is no quit key: Alt-X asks "Exit to DOS?" first).
                // panel.md "Pause (key P) and the clock": while paused, a mouse press only ends the pause; a key: tactkeys.
                Event::MouseButtonDown { .. } if !play.left && play.pause.paused && play.wait.is_none() => {
                    inputs.push(Input::Other)
                }
                // world_load.md section 8 "Click": while an aim or a confirm is open, other keys do nothing.
                Event::KeyDown { keycode, .. } if key_way(keycode == Some(Keycode::Escape), play.left, play.wait.is_some()) == KeyWay::Escape => inputs.push(Input::Escape),
                Event::KeyDown { keycode, .. } if key_way(keycode == Some(Keycode::Escape), play.left, play.wait.is_some()) == KeyWay::Swallow => {}
                // TacticalKeyHandler 0x13490: the rules are in tactkeys::route (keyhandler_diff).
                Event::KeyDown { keycode: Some(k), keymod, .. } if key_way(k == Keycode::Escape, play.left, play.wait.is_some()) == KeyWay::Tactical => {
                    use ja1front::tactkeys::{self, Route};
                    let Some((first, second)) = dos_key_of(k, keymod) else { continue };
                    let g = window_gates(&e.world, play.pause.paused, play.day.save_dir.as_ref().is_some_and(|d| d.join("QUICK.SAV").exists()));
                    if tactkeys::clears_marks(g.mode) {
                        e.world.update.spotted = 0x40;
                        e.world.update.view_tile = 0xC41;
                    }
                    let route = tactkeys::route(first, second, &g);
                    eprintln!("window key {:#x} {:#x} route {:?}", first, second, route);
                    let fan67 = ja1logic::fanfix::on(67);
                    if route.back_fan(fan67) {
                        ja1front::play::Session::new(&mut e.world, &mut play.host).cursor_back();
                    }
                    if route.strip_reload(fan67) {
                        // F67: the strip reload after the box ends a squad lost shade (strips.rs).
                        ja1front::strips::clear_lost_shade();
                    }
                    match route {
                        Route::None => {}
                        Route::EndPause | Route::EndCompress => inputs.push(Input::Other),
                        Route::AskQuit => ask_quit = true,
                        Route::QuickSave => quick_key = Some(true),
                        Route::QuickRestore => quick_key = Some(false),
                        Route::Debug(code) => inputs.push(Input::Debug(code)),
                        // SelectMerc gets the Alt latch 0x284F20 (TacticalKeyHandler 0x136A8 / 0x136BD), which the key
                        // flag step 0x1409B writes after the key: the Alt state of the previous pass, not of this key.
                        Route::Merc(n) => {
                            let alt = play.key_latches.alt;
                            eprintln!("window merc {} alt {}", n, alt as u8);
                            inputs.push(Input::Merc(n, alt))
                        }
                        Route::SeesLine => inputs.push(Input::SeesLine),
                        Route::Options => open_opt = true,
                        Route::Arrow(scan) => inputs.push(Input::Arrow(scan)),
                        Route::View(n) => inputs.push(Input::View(n)),
                        Route::ForceTurn => inputs.push(Input::ForceTurn),
                        Route::NextEnemy => inputs.push(Input::NextEnemy),
                        Route::Escape => {
                            eprintln!("key escape");
                            inputs.push(Input::Escape);
                        }
                        Route::NextMerc => inputs.push(Input::NextMerc),
                        Route::Volume(c) => {
                            eprintln!("window volume key {}", c as char);
                            inputs.push(Input::Volume(c))
                        }
                        Route::Abort => ask_abort = true,
                        Route::Compress | Route::NoCompress => inputs.push(Input::Compress),
                        Route::EndTurn => inputs.push(Input::EndTurn),
                        Route::CursorBack => {}
                        Route::Inventory => open_inv = true,
                        Route::SayAgain => inputs.push(Input::SayAgain),
                        Route::Map => open_map = true,
                        Route::Pause => inputs.push(Input::Pause),
                        Route::StatPopups => {
                            stat_wait = WindowWaitKey::new();
                            eprintln!("window stat popups");
                            stat_keys = Some(ja1front::screen::key_s(&e.world.soldiers, &mut play.cursor.over_map));
                        }
                        Route::Traverse => ask_trav = Some((ja1front::strips::TRAV_KEY_CORNER, true)),
                        Route::Version => inputs.push(Input::Version),
                        Route::Swap => inputs.push(Input::Swap),
                        Route::Popup => inputs.push(Input::Popup),
                    }
                }
                Event::KeyDown { keycode: Some(Keycode::X), keymod, .. } if keymod.intersects(Mod::LALTMOD | Mod::RALTMOD) => {
                    ask_quit = true;
                }
                Event::KeyDown { keycode: Some(Keycode::M), .. } => open_map = true,
                Event::MouseButtonDown { .. } if !play.left && e.world.update.time_compress != 0 => inputs.push(Input::Other),
                // After the sector was left the arrows move the view at once; it goes into the world (view_x /
                // view_y) so that op 99 focus (world_load.md section 8 "Focus") knows what the window shows.
                Event::KeyDown { keycode: Some(k @ (Keycode::Left | Keycode::Right | Keycode::Up | Keycode::Down)), .. } => {
                    let (cols, rows, _, _) = ja1front::viewmode::world_window(&e.world);
                    match k {
                        Keycode::Left => c0 = (c0 - 1).max(0),
                        Keycode::Right => c0 = (c0 + 1).min(56 - cols),
                        Keycode::Up => r0 = (r0 - 1).max(0),
                        _ => r0 = (r0 + 1).min(56 - rows),
                    }
                    e.world.view_x = c0 * 16;
                    e.world.view_y = r0 * 14;
                }
                Event::Window { win_event: sdl2::event::WindowEvent::Leave, .. } if ja1front::dosmouse::leave_counts(frame::dos_captured()) => {
                    // over_map stays as the last hover step left it: the pass's off-map step (off_map) does the
                    // unlight, the name and over_map = 0 in the same frame (TacticalKeyHandler 0x13490).
                    play.set_mouse(None);
                    play.mouse_out = true;
                    pointer_visible(true);
                }
                Event::MouseMotion { x, y, .. } => {
                    // The bar area under the mouse gets the hover frame (panel.md "hover slot").
                    let (px, py) = screen_pixel(&canvas, x, y);
                    // The enemies' turn: no MousePoll? runs, so a move does not count and op 157 puts the mouse back.
                    if play.host.mouse_hold && e.world.combat != 0 {
                        continue;
                    }
                    // TacticalKeyHandler 0x134A0: with 0x92DCC at 0 (an enemy acts in an interrupt too) no MousePoll? runs.
                    if !ja1front::slotmouse::pass_runs(e.world.update.screen_drawn) {
                        continue;
                    }
                    // AimLoop 0x1D744 (0x682AC): while the aim waits, the mouse cannot leave its place.
                    if matches!(play.wait, Some(ja1front::cursor::ClickWait::Aim { .. })) {
                        let (ax, ay) = play.mouse_px;
                        if (px, py) != (ax, ay) {
                            warp_frame(&sdl.mouse(), &canvas, ax, ay);
                        }
                        continue;
                    }
                    play.mouse_px = (px, py);
                    play.mouse_out = false;
                    // panel.md "Clicks on the bottom strip": while an item waits on the cursor under the panel
                    // (cursor_drop), the mouse stays inside x 1..318, y 190..197.
                    if e.world.inv.cursor_drop != 0 && px >= 0 {
                        let (cx, cy) = (px.clamp(1, 318), py.clamp(190, 197));
                        if (cx, cy) != (px, py) {
                            warp_frame(&sdl.mouse(), &canvas, cx, cy);
                        }
                    }
                    let at = spot(&e.world.soldiers, px, py, vw_now(&e.world).0, vw_now(&e.world).1, e.world.opt_onscreen);
                    play.slot_pt = if let Spot::View(..) = at { None } else { Some((px, py)) };
                    play.slots_moved(&mut e.world);
                    screen.hover = ja1front::screen::bar_hover(play.slots.hover);
                    // cursor.md: over the map the system pointer is hidden and the tile cursor shows the mouse
                    // tile; outside it the pointer is back. Over the map each pass runs a hover step (section 8).
                    if let Spot::View(vx, vy) = at {
                        play.set_mouse(Some(map_tile(&e.world, c0, r0, vx, vy)));
                    } else {
                        // 0x143AD: the mouse leaves the map.
                        if play.mouse.is_some() && ja1front::pointer::leave_hides(e.world.opt_onscreen, e.world.view_h == 14 * 13) {
                            pointer_left(true);
                        }
                        play.set_mouse(None);
                    }
                    pointer_visible(ja1front::pointer::tactical_pointer_shows(play.mouse.is_some(), e.world.held[0], play.host.mouse_hold, e.world.combat));
                    let m = sdl2::keyboard::Mod::LSHIFTMOD | sdl2::keyboard::Mod::RSHIFTMOD;
                    let ms = sdl.keyboard().mod_state();
                    play.shift_alt = (ms.intersects(m), ms.intersects(sdl2::keyboard::Mod::LALTMOD | sdl2::keyboard::Mod::RALTMOD));
                }
                Event::MouseButtonDown { mouse_btn: MouseButton::Left, x, y, .. } if !play.left => {
                    let (px, py) = screen_pixel(&canvas, x, y);
                    pointer_left(false);
                    // The step of 0x14308 + 0x143CA comes before the press tests of the same pass.
                    let (px, py) = if play.wait.is_none() && ja1front::slotmouse::pass_runs(e.world.update.screen_drawn) {
                        margin_to(&e.world, play.cursor.over_map != 0, px, py).unwrap_or((px, py))
                    } else {
                        (px, py)
                    };
                    // 0x141EC: in the enemies' turn the handler returns before the ButtonAt? tests of 0x14530..0x14600,
                    // so a strip button shows no pressed face, plays no sound and does nothing.
                    let strip_takes = ja1front::slotmouse::pass_runs(e.world.update.screen_drawn);
                    turn_press |= !strip_takes;
                    // panel.md "Clicks on the top strip": a button that reacts shows its pressed face until the
                    // left button is up; the action follows on release.
                    if let Some(i) = ja1front::strips::button_at(px, py).filter(|_| strip_takes) {
                        if let Some(a) = ja1front::strips::button_action(i, &strip_click_env(&e.world)) {
                            eprintln!("strip press {}", i);
                            screen.pressed = Some(i);
                            strip_hold = Some(ja1front::pointer::ButtonHold::press((px, py)));
                            // sound.md "Button sounds": the press sound with the pressed face.
                            play.host.sound.button(0);
                            strip_action = Some(a);
                            continue;
                        }
                    }
                    // panel.md "Clicks on the top strip": the sector box (286, 3, 306, 11) pauses the game.
                    if strip_takes && (286..=306).contains(&px) && (3..=11).contains(&py) {
                        eprintln!("sector box press");
                        inputs.push(Input::Pause);
                        continue;
                    }
                    let at = spot(&e.world.soldiers, px, py, vw_now(&e.world).0, vw_now(&e.world).1, e.world.opt_onscreen);
                    // panel.md "Clicks on the bottom strip": mouse y 182..198 is the strip of the selected merc.
                    if at == Spot::None && (182..=198).contains(&py) {
                        let ctrl = sdl.keyboard().mod_state().intersects(Mod::LCTRLMOD | Mod::RCTRLMOD);
                        inputs.push(Input::Strip { x: px, right: false, ctrl });
                        continue;
                    }
                    // 0x141EC: in the enemies' turn no press reaches SlotMouse.
                    held = popup_press(&e.world.soldiers, screen.hover, at).filter(|_| ja1front::slotmouse::pass_runs(e.world.update.screen_drawn));
                    match at {
                        // A click on a portrait chooses that merc (the same as its number key).
                        Spot::Slot(k) => inputs.push(Input::Portrait(
                            k, sdl.keyboard().mod_state().intersects(Mod::LALTMOD | Mod::RALTMOD))),
                        // The bar area: the stat popup (held) or nothing; the inventory popup is not ported yet.
                        Spot::Bars(_) | Spot::None => {}
                        Spot::View(vx, vy) => {
                            let m = sdl.keyboard().mod_state();
                            let shift = m.intersects(sdl2::keyboard::Mod::LSHIFTMOD | sdl2::keyboard::Mod::RSHIFTMOD);
                            let alt = m.intersects(Mod::LALTMOD | Mod::RALTMOD);
                            let tile = map_tile(&e.world, c0, r0, vx, vy);
                            // The overview mouse step's left press also runs Walk (panel.md step 6).
                            if play.wait.is_none() && (play.cursor.mode == 1 || e.world.opt_onscreen == 0) {
                                // cursor.md "Drag to an edge": the walk or the drag follows at the release.
                                // UseItemAt 0x1D9B8: only a tile with no merc reaches the loop; a merc is selected at the press.
                                play.cursor = screen.cursor;
                                let walks = play.press_walks(&mut e.world, tile, shift, alt);
                                screen.cursor = play.cursor;
                                eprintln!("left press {} walks {}", tile, walks as u8);
                                if walks {
                                    play.drag_press(tile, (px, py), shift, alt, play.cursor.value, play.cursor.tile);
                                } else {
                                    inputs.push(Input::Click { tile, shift, alt });
                                    map_press = e.world.opt_onscreen != 0;
                                    ov_press = e.world.opt_onscreen == 0;
                                }
                            } else {
                                map_press = play.wait.is_none() && e.world.opt_onscreen != 0;
                                inputs.push(Input::Click { tile, shift, alt });
                            }
                            if map_press {
                                eprintln!("map press hold 1");
                            }
                        }
                    }
                }
                // cursor.md "Clicks", right button over the map with the left button up: the hold starts (its polls
                // and its release come above).
                Event::MouseButtonDown { mouse_btn: MouseButton::Right, x, y, .. } if !play.left => {
                    let (px, py) = screen_pixel(&canvas, x, y);
                    // Live in the original (issue #31): as with a left press, the pointer comes back on rows 196..199 of mode 3.
                    pointer_left(false);
                    let (px, py) = if play.wait.is_none() && ja1front::slotmouse::pass_runs(e.world.update.screen_drawn) {
                        margin_to(&e.world, play.cursor.over_map != 0, px, py).unwrap_or((px, py))
                    } else {
                        (px, py)
                    };
                    let at = spot(&e.world.soldiers, px, py, vw_now(&e.world).0, vw_now(&e.world).1, e.world.opt_onscreen);
                    // panel.md "The switch view": the right button on a merc slot.
                    if let (Spot::Slot(k) | Spot::Bars(k), false) = (at, left_held) {
                        if play.wait.is_none() && ja1front::screen::switch_can_open(&e.world, k) {
                            switch_ui = Some(SwitchUi { k, step: SwStep::Open, t: Instant::now() });
                            eprintln!("switch {} open", k);
                        }
                        continue;
                    }
                    if at == Spot::None && (182..=198).contains(&py) && !left_held {
                        inputs.push(Input::Strip { x: px, right: true, ctrl: false });
                    }
                    if let (Spot::View(vx, vy), false) = (at, left_held) {
                        let tile = map_tile(&e.world, c0, r0, vx, vy);
                        let m = sdl.keyboard().mod_state();
                        let shift = m.intersects(sdl2::keyboard::Mod::LSHIFTMOD | sdl2::keyboard::Mod::RSHIFTMOD);
                        let alt = m.intersects(Mod::LALTMOD | Mod::RALTMOD);
                        right_press = Some((tile, py));
                        inputs.push(Input::RHold { tile, shift, alt, y: py });
                    }
                }
                Event::MouseButtonUp { mouse_btn: MouseButton::Left, .. } => {
                    if held.take().is_some() {
                        // panel.md "Other users of Lock": the hold Locked the game (clock switch 1) meanwhile.
                        let now = ja1front::dostime::game_ticks(start.elapsed());
                        play.locked_ticks(&mut e.world, true, now.saturating_sub(ticks_done), false);
                        ticks_done = now;
                    }
                    if screen.pressed.take().is_some() {
                        hold_end(&canvas, strip_hold.take());
                        play.host.sound.button(1);
                    }
                    match strip_action.take() {
                        Some(StripAction::EndTurn) => inputs.push(Input::EndTurn),
                        Some(StripAction::Inventory) => open_inv = true,
                        Some(StripAction::Map) => open_map = true,
                        Some(StripAction::Abort) => ask_abort = true,
                        Some(StripAction::Options) => open_opt = true,
                        Some(StripAction::Traverse) => ask_trav = Some((ja1front::strips::trav_button_corner(), false)),
                        Some(StripAction::Nothing) | None => {}
                    }
                }
                _ => {}
            }
        }
        // TacticalKeyHandler 0x14308 + 0x143CA: no rest for the mouse on the strip beside the map.
        if ja1front::slotmouse::rest_pass_runs([map.open, play.left, play.wait.is_some(), play.drag.is_some(), switch_ui.is_some(),
                                                  stat_keys.is_some(), held.is_some(), right_press.is_some(), left_held,
                                                  play.pause.paused], e.world.update.screen_drawn)
        {
            let ms = events.mouse_state();
            let (px, py) = screen_pixel(&canvas, ms.x(), ms.y());
            if let Some((nx, ny)) = margin_to(&e.world, play.cursor.over_map != 0, px, py) {
                eprintln!("margin mouse {} {}", nx, ny);
                warp_frame(&sdl.mouse(), &canvas, nx, ny);
            }
        }
        // cursor.md "Clicks", right step 3: the original polls the mouse all the time while the right button is
        // held, so a frame with no mouse event is one poll too.
        if let (Some((_, y)), false) = (right_press, polled) {
            inputs.push(Input::Poll { left: left_held, right: true, y });
        }
        // panel.md "The guide's and Brenda's balloons": the guide's quote of a map click over the island map.
        if map.open {
            let said = map.take_said();
            if quotes_wait(&said) {
                let base = map.shown().to_vec();
                for (op, a) in said {
                    let sub = ja1front::ctlpanel::sub_byte(e.world.update.opt_step_text);
                    if op == 328 {
                        let under = map.fire_base.take().unwrap_or_else(|| base.clone());
                        let p = a.get(3).copied().unwrap_or(0);
                        let pr = e.world.profiles.get(p as usize * 0x5A..p as usize * 0x5A + 10).unwrap_or(&[]);
                        let nick: Vec<u8> = pr.iter().copied().take_while(|&b| b != 0).collect();
                        if !fire_balloon(iso, exe, &a, &nick, sub, &under, &map.pal, &mut canvas, &mut tex, &mut events) {
                            break 'outer;
                        }
                        continue;
                    }
                    if !guide_balloon(iso, exe, op, &a, sub, &base, &map.pal, &mut canvas, &mut tex, &mut events) {
                        break 'outer;
                    }
                }
                ticks_done = ja1front::dostime::game_ticks(start.elapsed());
            }
        }
        // panel.md "Abort and restart questions": question boxes over the tactical screen; the game waits for the
        // answers and the time meanwhile does not count.
        // ShowMouse 0x673AC: byte 0x92E4F is 1 in the boxes, the inventory, the Control Panel, the switch view and a held
        // strip button (ButtonAt? 0x67D99), 0 on the plain tactical screen.
        {
            let flag = show_mouse_flag([ask_quit, ask_abort, open_opt, open_inv, open_map, ask_trav.is_some(),
                                        switch_ui.is_some(), screen.pressed.is_some(), held.is_some(), map.day]);
            // pointer::top_row_hidden (ShowMouse? / MousePoll?: no pointer on rows 0..15 while byte 0x92E4F = 0) is not
            // applied, because the player never sees it: when that rule sets the hide byte 0x92E70, TacticalRedraw 0x176C8
            // draws the pointer into each tactical frame itself (DrawMousePointer? EBX 1, mouse off the map). Live in the
            // original (#36 comment 12569): byte 0 after a Quick Restore and after the inventory, red arrow on screen at
            // y 8 and y 12 in 6 of 6 grabs (the maintainer side by side 2026-11-10 saw the same). The window never hides it.
            pointer_top(false);
            let view = (e.world.opt_onscreen, full_view(e.world.view_h));
            let last = POINTER_LEFT_VIEW.with(|v| v.replace(view));
            pointer_left(ja1front::pointer::left_step(POINTER_LEFT.with(|v| v.get()), last, view, play.mouse.is_none(),
                                                      flag || stat_keys.is_some()));
        }
        if ask_quit || ask_abort {
            let base = play_frame(iso, exe, e, screen, c0, r0, &mut play, false);
            let mut ask = |(text, pic): (&[u8], &str)| {
                let imgs = ja1front::question::screens(iso, exe, text, Some(pic), &base).unwrap_or_else(|c| fail("question box", c));
                question_box(&mut canvas, &mut tex, &mut events, &imgs, &pal)
            };
            if ask_quit {
                // 0x3808D: over_map 0x284F13 = 0 for the box, so the pointer shows also with the mouse over the map.
                pointer_visible(true);
                if ask(ja1front::question::EXIT) {
                    exit_dos = true;
                    quick_key = Some(true);
                } else {
                    pointer_visible(ja1front::pointer::tactical_pointer_shows(play.mouse.is_some(), e.world.held[0], play.host.mouse_hold, e.world.combat));
                }
            }
            if ask_abort {
                // YesNoPrompt 0x28C84 calls ShowMouse: the pointer shows also with the mouse over the map; its end
                // hides the pointer again when over_map 0x284F13 is set (the next pass decides).
                pointer_visible(true);
                if let Some(restart) = ja1front::question::ask_abort(&mut ask) {
                    inputs.push(Input::Abort { restart });
                }
            }
            ticks_done = ja1front::dostime::game_ticks(start.elapsed());
        }
        // panel.md "The Control Panel": over a fresh tactical frame; the game waits until it closes.
        if open_opt && e.world.update.screen_drawn != 0 {
            // panel.md "The Control Panel" Opening step 2: the full draw under the panel is the dark view.
            ja1front::enter::darken_next();
            let base = play_frame(iso, exe, e, screen, c0, r0, &mut play, false);
            let restore = play.day.save_dir.as_ref().is_some_and(|d| d.join("QUICK.SAV").exists());
            let (r, _) = control_panel_loop(&mut canvas, &mut tex, &mut events, iso, exe, &base, &pal, &mut e.world,
                                               &mut play.host, restore, &sdl.mouse());
            eprintln!("control panel closes {}", r);
            if let Some(save) = ja1front::ctlpanel::after_close(r) {
                quick_key = Some(save);
            }
            // panel.md "Other users of Lock": the panel Locked the game (clock switch 0) while it showed.
            let now = ja1front::dostime::game_ticks(start.elapsed());
            play.locked_ticks(&mut e.world, false, now.saturating_sub(ticks_done), false);
            ticks_done = now;
        }
        if let Some(save) = quick_key {
            // savegame.md "Quick Save and Quick Restore: the steps": step 1, the question box over the redrawn
            // tactical screen.
            let base = play_frame(iso, exe, e, screen, c0, r0, &mut play, false);
            let (text, pic) = if save { ja1front::quicksave::SAVE_ASK } else { ja1front::quicksave::RESTORE_ASK };
            let imgs = ja1front::question::screens(iso, exe, text, Some(pic), &base).unwrap_or_else(|c| fail("question box", c));
            if question_box(&mut canvas, &mut tex, &mut events, &imgs, &pal) {
                // The "Saving..." / "Restoring..." message box with no wait: it stays on the screen while the file
                // is written or read, the next draw covers it.
                let mut boxed = base.clone();
                let msg: &[u8] = if save { ja1front::quicksave::SAVING } else { ja1front::quicksave::RESTORING };
                ja1front::msgbox::draw_tactical(iso, exe, e.tileset, msg, &mut boxed).unwrap_or_else(|c| fail("message box", c));
                blit(&mut tex, &boxed, &pal, SW, SH);
                canvas.copy(&tex, None, None).expect("copy");
                canvas.present();
                if save {
                    match play.quick_save(&mut e.world) {
                        Ok(n) => eprintln!("quick save {} bytes", n),
                        Err(m) => eprintln!("quick save: {}", m),
                    }
                } else {
                    match play.quick_restore(&mut e.world) {
                        Ok(t) => eprintln!("quick restore day {} sector {} theme {}", e.world.day, e.world.cur_sector, t),
                        Err(m) => die(m),
                    }
                }
            }
            ticks_done = ja1front::dostime::game_ticks(start.elapsed());
        }
        if map_quit {
            // panel.md "Exit to DOS" (argument 0): the question in the map's box; yes: the day files go when there
            // is no QUICK.SAV, then the quit line.
            let (text, pic) = ja1front::question::EXIT;
            let imgs: Vec<Vec<u8>> = [None, Some(0), Some(1)].into_iter()
                .map(|p| map.question_img(&e.world, text, Some(pic), p).unwrap_or_else(|c| fail("map", c))).collect();
            map.redraw(&e.world).unwrap_or_else(|c| fail("map", c));
            let mpal = map.pal;
            eprintln!("map ask exit");
            if question_box(&mut canvas, &mut tex, &mut events, &imgs, &mpal) {
                exit_dos = true;
                exit_zero = true;
            } else {
                eprintln!("map answer no");
            }
            ticks_done = ja1front::dostime::game_ticks(start.elapsed());
        }
        if exit_dos {
            // The tactical Quick Save (quick_key) ran above.
            let (deleted, line) = ja1front::question::exit_finish(!exit_zero, play.day.save_dir.as_deref(), exe, &mut play.host.rng, || {});
            if deleted {
                eprintln!("exit to dos: day files deleted");
            }
            eprintln!("exit to dos {}", String::from_utf8_lossy(&line));
            println!("{}\n", String::from_utf8_lossy(&line));
            break 'outer;
        }
        // Op 350 mouse_to of a strip click: the mouse goes to that game pixel.
        if let Some((mx, my)) = play.host.mouse_to.take() {
            play.mouse_px = (mx, my);
            warp_frame(&sdl.mouse(), &canvas, mx, my);
        }
        if let Some(((x0, y0), key)) = ask_trav.take() {
            if ja1front::play::can_traverse(&e.world) {
                if key {
                    // 0x1CC3E: the T key takes the tile cursor off the map before its redraw.
                    screen.cursor.tile = ja1front::travprompt::TILE_OFF;
                }
                let base = play_frame(iso, exe, e, screen, c0, r0, &mut play, false);
                if key {
                    // The T key puts the mouse at (x0 + 45, y0 + 35).
                    warp_frame(&sdl.mouse(), &canvas, x0 + 45, y0 + 35);
                }
                // 0x1CD20: the arrow shows over the popup, also over the map.
                pointer_visible(true);
                let (pick, at) = trav_choose(&mut canvas, &mut tex, &mut events, &pal, screen, iso, exe, &base, x0, y0);
                // The popup took the motion events: the game goes on with the mouse where the popup left it.
                play.mouse_px = at;
                let (cols, rows, l, t) = ja1front::viewmode::world_window(&e.world);
                let in_map = at.0 >= l && at.0 < l + cols * 16 && at.1 >= t && at.1 < t + rows * 14;
                ja1front::travprompt::close(&mut screen.cursor, pick, in_map);
                if let Spot::View(vx, vy) = spot(&e.world.soldiers, at.0, at.1, vw_now(&e.world).0, vw_now(&e.world).1, e.world.opt_onscreen) {
                    play.set_mouse(Some(map_tile(&e.world, c0, r0, vx, vy)));
                } else {
                    play.set_mouse(None);
                }
                pointer_visible(ja1front::pointer::tactical_pointer_shows(play.mouse.is_some(), e.world.held[0], play.host.mouse_hold, e.world.combat));
                if let Some(i) = pick {
                    inputs.push(Input::Traverse(ja1front::strips::TRAV_CODES[i]));
                }
            }
            ticks_done = ja1front::dostime::game_ticks(start.elapsed());
        }
        // SquadLost? 0x3A374 fades the old picture out over 56 steps before the load (TraverseMove 0x3164E: op 215 in its
        // traverse_out), and each fades the new picture in over 56 steps. QuickRestore 0x2D7AB does it in 24 steps, after the read: its fade out already uses the
        // palette of the restored sector (LoadTerrainTileset 0x2A9FC runs LoadTilePalette).
        let (fade_steps, out_first, quick_move) = ja1front::screen::sector_fade(play.moved, play.quick_restored);
        if out_first {
            fade_show8(&mut canvas, &mut tex, &last_frame, &pal, fade_steps, 256, 0);
        }
        let traversed = play.traversed.is_some();
        let moved = if let Some(out) = play.traversed.take() {
            // The move: the new sector loads, the window shows it. TraverseMove 0x3169A: MouseSetPos(159, 99); the other
            // roads to a new picture (Squad lost, Quick Restore) have no such call.
            play.traverse_move(&mut e.world, out).unwrap_or_else(|m| die(m));
            eprintln!("trav sector {}", e.world.cur_sector);
            warp_frame(&sdl.mouse(), &canvas, 159, 99);
            true
        } else if std::mem::take(&mut play.moved) {
            // soldier_update.md "Squad lost": the pass loaded sector 59. Quick Restore (0x2C898) also comes here.
            if !std::mem::take(&mut play.quick_restored) {
                eprintln!("squad lost sector {}", e.world.cur_sector);
            }
            true
        } else {
            false
        };
        if moved {
            e.tileset = play.day.tileset;
            pal = palette(iso, e.tileset as i32).unwrap_or_else(|c| fail("palette", c));
            title = format!("ja1front: sector {} (cur_sector {})", e.world.cur_sector as i32 + 1, e.world.cur_sector);
            (c0, r0) = ja1front::enter::view_tile(&e.world);
            *screen = ja1front::screen::Screen::new(iso, exe, &mut e.world).unwrap_or_else(|c| fail("faces", c));
            if quick_move {
                fade_show8(&mut canvas, &mut tex, &last_frame, &pal, fade_steps, 256, 0);
            }
            let idx = play_frame(iso, exe, e, screen, c0, r0, &mut play, false);
            fade_show8(&mut canvas, &mut tex, &idx, &pal, fade_steps, 0, 256);
            eprintln!("sector move faded in {}", fade_steps);
            if traversed {
                play.traverse_theme(&mut e.world);
            }
            ticks_done = ja1front::dostime::game_ticks(start.elapsed());
        }
        if play.left && !play.day.map_open && !map.open {
            // Section 9 steps 1..3 after the sector was left (and its end-of-day screen closed), then the map.
            // Op 319 guide_offer asks in the merc-face question box (strategic.md "The guide's advice" 13) over the
            // island map: pictures with no button / NO / OK pressed for the next guide's profile.
            let gp = e.world.guides_hired.wrapping_add(60);
            let imgs: Vec<Vec<u8>> = [None, Some(0), Some(1)].into_iter()
                .map(|pr| map.guide_offer_img(&e.world, gp, pr).unwrap_or_else(|c| fail("map", c))).collect();
            let mpal = map.pal;
            // The day-end screen keeps the POSTPAN.PCC palette of ReplayScreenInit? 0x64274 (replaypic::postpan_palette).
            let dpal = ja1front::replaypic::postpan_palette(exe).unwrap_or_else(|c| fail("postpan", c));
            let cv = &mut canvas as *mut Frame;
            let tx = &mut tex as *mut sdl2::render::Texture<'_> as *mut sdl2::render::Texture<'static>;
            let ev = &mut events as *mut sdl2::EventPump;
            let gw = &e.world as *const ja1front::world::World;
            play.host.guide_ask = Some(Box::new(move |p: i32| -> bool {
                if p != gp as i32 {
                    // SPEC GAP front-end: the pictures are made for guides_hired + 60 before the steps.
                    eprintln!("guide offer for profile {} (pictures for {}): no", p, gp);
                    return false;
                }
                // The day ended (smoke_play asserts this line: the box waits for an answer).
                eprintln!("guide offer asks (profile {})", p);
                // SAFETY: as for the day start question below (day_steps runs inside this round of the loop).
                unsafe {
                    prompt_mouse(&*cv, &*ev, ja1front::ctlpanel::subtitles(&*gw));
                    question_loop(&mut *cv, &mut *tx, &mut *ev, &imgs, &mpal, None)
                }
            }));
            // Section 9 step 1, "Not enough funds": the dialog polls the window over the assignment picture with its
            // message box; op 275 asks in the merc-face question box ("Don't pay NICK?").
            let mpm = &mut map as *mut ja1front::mapui::MapUi;
            let mp = mpm as *const ja1front::mapui::MapUi;
            let wp = &e.world as *const ja1front::world::World;
            // Op 301: the payroll screen (strategic.md "The payroll screen") over the replay screen, its screens at
            // their times and its sounds; its last screen stays under "Not enough funds".
            let (piso, pexe) = (iso.to_string(), exe.to_string());
            play.host.pointer = Some(Box::new(pointer_shape));
            let mut limit_was = None;
            play.host.mouse_limit = Some(Box::new(move |l| {
                frame::set_limit(l);
                if limit_was != Some(l) {
                    eprintln!("mouse limit {} {} {} {}", l.0, l.1, l.2, l.3);
                    limit_was = Some(l);
                }
            }));
            let place_mouse = sdl.mouse();
            play.host.mouse_place = Some(Box::new(move |x, y| {
                // SAFETY: as for pay_win below.
                let c = unsafe { &*cv };
                warp_frame(&place_mouse, c, x, y);
                eprintln!("mouse place {} {}", x, y);
            }));
            play.host.payroll_show = Some(Box::new(move |under: Option<&[u8]>| {
                // SAFETY: as for pay_win below.
                let (cv, tx, map, w) = unsafe { (&mut *cv, &mut *tx, &mut *mpm, &*wp) };
                map.redraw(w).unwrap_or_else(|c| fail("map", c));
                let steps = map.payroll_steps(w, under).unwrap_or_else(|c| fail("payroll", c));
                let mut names = Vec::new();
                ja1front::sound::screen_main(&piso, &pexe);
                for st in &steps {
                    if let Some((fx, rate, vol, n)) = st.sound {
                        ja1front::sound::screen(|x| x.sfx(fx, rate, vol, n));
                        names.push(ja1front::sound::main_name(fx));
                    }
                    blit(tx, &st.screen, &dpal, SW, SH);
                    cv.copy(tx, None, None).expect("copy");
                    cv.present();
                    // strategic.md "The payroll screen": waits on the game clock (world_load.md "Game speed")
                    ja1front::dostime::sleep(st.ms as i64);
                }
                Some(format!("payroll steps {} sounds {}", steps.len(), names.join(" ")))
            }));
            // strategic.md "The day-end screen": the OK buttons 15 (replay screen) and 16 (payroll screen) wait for
            // a left press in their hit area: the press sound, the pressed face until the left button is up, the
            // up face, the release sound. Closing the window ends the program (#36 C: the original is a DOS
            // program in DOSBox-X, whose window close kills the game at once; re-queued, the event died in the
            // payroll / xp polls and the day ran on).
            let (diso, dexe) = (iso.to_string(), exe.to_string());
            // The last picture of the payroll wait, for the fade out at the end of DayEndScreen? (0x65F9C).
            let dayend_last: std::rc::Rc<std::cell::RefCell<Option<Vec<u8>>>> = Default::default();
            {
                let last = dayend_last.clone();
                play.host.dayend_fade = Some(Box::new(move |shot: Option<(&[u8], &[u8; 768])>| {
                    // SAFETY: as for pay_win below.
                    let (cv, tx) = unsafe { (&mut *cv, &mut *tx) };
                    // strategic.md "The day-end screen": 24 steps, 256 -> 0, in the POSTPAN.PCC palette.
                    if let Some((pic, pal)) = shot {
                        fade_show8(cv, tx, pic, pal, 24, 256, 0);
                        eprintln!("day-end faded out (replay screen)");
                    } else if let Some(pic) = last.borrow_mut().take() {
                        fade_show8(cv, tx, &pic, &dpal, 24, 256, 0);
                        eprintln!("day-end faded out");
                    }
                }));
            }
            let last = dayend_last.clone();
            play.host.dayend_win = Some(Box::new(move |w: &mut ja1front::world::World, b: u8,
                                                       shot: Option<(&[u8], &[u8; 768])>| -> i32 {
                // SAFETY: as for pay_win below; the world comes as the loop's own argument.
                let (cv, tx, ev, map) = unsafe { (&mut *cv, &mut *tx, &mut *ev, &mut *mpm) };
                let (mut s, pal) = match shot {
                    Some((s, p)) => (s.to_vec(), *p),
                    // The payroll wait starts on the screen the morning left: PayDayDialog? ends with the team slot
                    // marks (PAID, 0x4558F), and every later redraw pairs DrawBassMap 0x4420C with
                    // DrawTeamSlotMarks? 0x4508C (live shot 2026-11-05: the marks stay under the Fire question).
                    None => match map.payroll {
                        Some(_) => (map.payroll_again(w).unwrap_or_else(|c| fail("payroll", c)), dpal),
                        None => return 0,
                    },
                };
                ja1front::dayloop::dayend_ok_face(&mut s, &diso, &dexe, b, false);
                eprintln!("day-end wait {}", b);
                let mut host = DayEndWin { question: None, cv, tx, ev, map, w, s, pal, iso: &diso, exe: &dexe, b, at: (0, 0), buttons: 0,
                                           keys: Default::default(), tip: Default::default() };
                host.show();
                match ja1front::dayend::wait(&mut host, b) {
                    ja1front::dayend::End::Exit => 3,
                    _ => {
                        eprintln!("day-end ok {}", b);
                        if b == 16 {
                            *last.borrow_mut() = Some(host.s.clone());
                        }
                        0
                    }
                }
            }));
            // Ops 282..287 (strategic.md "The experience count"): the counts on the payroll screen with their waits
            // and click sounds; Escape drops the waits and the clicks.
            {
                let (xiso, xexe) = (iso.to_string(), exe.to_string());
                let mut xv: Option<ja1front::xpcount::XpView> = None;
                play.host.xp_op = Some(Box::new(move |op: i32, a: &[i32]| {
                    // SAFETY: as for pay_win below.
                    let (cv, tx, ev, map, w) = unsafe { (&mut *cv, &mut *tx, &mut *ev, &*mpm, &*wp) };
                    if xv.is_none() {
                        let faces = map.faces(w).unwrap_or_else(|c| fail("faces", c));
                        let under = map.payroll.clone().unwrap_or_else(|| vec![0u8; 64000]);
                        xv = Some(ja1front::xpcount::XpView::new(&xiso, &xexe, &w.soldiers[..9 * 0x200], &faces,
                                                                 &w.stat_changed, &under).unwrap_or_else(|c| fail("xp", c)));
                    }
                    let v = xv.as_mut().unwrap();
                    let evr: *mut sdl2::EventPump = ev;
                    let mut esc = || {
                        let mut hit = false;
                        // SAFETY: the out closure below polls the same pump only while esc does not run.
                        for evt in unsafe { &mut *evr }.poll_iter() {
                            if let Event::KeyDown { keycode: Some(Keycode::Escape), .. } = evt {
                                hit = true;
                            }
                        }
                        hit
                    };
                    let l = v.op(&xiso, &xexe, op, a, &w.soldiers[..9 * 0x200], &mut esc, &mut |o| match o {
                        ja1front::xpcount::Out::Screen(sc) => {
                            blit(tx, sc, &dpal, SW, SH);
                            cv.copy(tx, None, None).expect("copy");
                            cv.present();
                        }
                        ja1front::xpcount::Out::Wait(ms) => std::thread::sleep(Duration::from_millis(ms as u64)),
                        ja1front::xpcount::Out::Click => {
                            ja1front::sound::screen_sfx(&xiso, &xexe, ja1front::mapui::CLICK_SOUND)
                        }
                        // panel.md "Message box": a key, a new press or 25 s close it.
                        ja1front::xpcount::Out::Message(sc, text) => {
                            eprintln!("window msgbox 25000 {}", text);
                            // ShowMessage? 0x27BBE, 0x27CC0: no pointer while the level box shows.
                            pointer_box(true);
                            blit(tx, sc, &dpal, SW, SH);
                            cv.copy(tx, None, None).expect("copy");
                            cv.present();
                            let shown = Instant::now();
                            // SAFETY: as for esc above. (A close of the window here ends the box only, as before.)
                            box_wait(unsafe { &mut *evr }, 25000, &shown);
                            eprintln!("window msgbox end 25000");
                            pointer_box(false);
                        }
                    });
                    if op == 287 {
                        xv = None;
                    }
                    l
                }));
            }
            let mut shown: Option<(Vec<i32>, ja1front::dayloop::PayView, Vec<u8>)> = None;
            let mut mouse = (0, 0);
            // The dialog's last shown state (strategic.md "Not enough funds"): the boxes and the balloon draw over it.
            let pv = std::rc::Rc::new(std::cell::RefCell::new(ja1front::dayloop::PayView::default()));
            let pv_win = pv.clone();
            play.host.pay_win = Some(Box::new(move |msgs: &[i32], view: &ja1front::dayloop::PayView, redraw: bool| -> Option<(i32, i32, i32)> {
                // SAFETY: as for the day start question below (day_steps runs inside this round of the loop); the
                // map and the world stay in place while it runs, and the library writes only through its view.
                let (cv, tx, ev, map, w) = unsafe { (&mut *cv, &mut *tx, &mut *ev, &*mp, &*wp) };
                if redraw || shown.as_ref().map(|s| (s.0.as_slice(), &s.1)) != Some((msgs, view)) {
                    if shown.as_ref().map(|s| s.0.as_slice()) != Some(msgs) || view.end {
                        eprintln!("pay dialog shows {:?}{}", msgs, if view.end { " end" } else { "" });
                    }
                    let text = ja1front::dayloop::pay_box_text(msgs);
                    let img = map.payday_img(w, text.as_deref(), view).unwrap_or_else(|c| fail("map", c));
                    blit(tx, &img, &dpal, SW, SH);
                    shown = Some((msgs.to_vec(), view.clone(), img));
                    *pv_win.borrow_mut() = view.clone();
                }
                cv.copy(tx, None, None).expect("copy");
                cv.present();
                std::thread::sleep(Duration::from_millis(10));
                let mut b = 0;
                for evt in ev.poll_iter() {
                    match evt {
                        Event::Quit { .. } => return None,
                        Event::MouseMotion { x, y, .. } => mouse = screen_pixel(cv, x, y),
                        Event::MouseButtonDown { mouse_btn: sdl2::mouse::MouseButton::Left, x, y, .. } => {
                            mouse = screen_pixel(cv, x, y);
                            b = 1;
                        }
                        _ => {}
                    }
                }
                Some((mouse.0, mouse.1, b))
            }));
            let pv_say = pv.clone();
            play.host.pay_say = Some(Box::new(move |k: i32, x0: i32, p: i32, sub: bool| {
                // SAFETY: as for pay_win.
                let (cv, tx, ev, map, w) = unsafe { (&mut *cv, &mut *tx, &mut *ev, &*mp, &*wp) };
                let under = map.payday_img(w, None, &pv_say.borrow()).unwrap_or_else(|c| fail("map", c));
                let nick = w.profiles.get(p as usize * 0x5A..p as usize * 0x5A + 10).unwrap_or(&[]);
                if !pay_balloon(&map.iso, &map.exe, k, x0, p, nick, sub, &under, &dpal, cv, tx, ev) {
                    exit(0);
                }
            }));
            let pv_merc = pv.clone();
            play.host.merc_say = Some(Box::new(move |s: ja1front::play::MercSay| {
                // SAFETY: as for pay_win.
                let (cv, tx, ev, map, w) = unsafe { (&mut *cv, &mut *tx, &mut *ev, &*mp, &*wp) };
                let under = map.payday_img(w, None, &pv_merc.borrow()).unwrap_or_else(|c| fail("map", c));
                if !merc_balloons(&map.iso, &map.exe, s, w, &under, &dpal, cv, tx, ev) {
                    exit(0);
                }
            }));
            let pv_ask = pv.clone();
            let pv_corpse = pv.clone();
            let pv_raise = pv.clone();
            play.host.pay_ask = Some(Box::new(move |k: i32, family: i32| -> bool {
                // SAFETY: as for pay_win.
                let (cv, tx, ev, map, w) = unsafe { (&mut *cv, &mut *tx, &mut *ev, &*mp, &*wp) };
                eprintln!("pay dialog asks (slot {} family {})", k, family);
                let under = map.payday_img(w, None, &pv_ask.borrow()).unwrap_or_else(|c| fail("map", c));
                let imgs: Vec<Vec<u8>> = [None, Some(0), Some(1)].into_iter()
                    .map(|pr| map.payday_ask_img(w, &under, k.clamp(0, 7) as usize, family, pr).unwrap_or_else(|c| fail("map", c)))
                    .collect();
                prompt_mouse(cv, ev, ja1front::ctlpanel::subtitles(w));
                question_loop(cv, tx, ev, &imgs, &dpal, None)
            }));
            let cmouse = sdl.mouse();
            let cevents = sdl.event().expect("SDL events");
            // Op 292 corpse_ask (strategic.md "Corpse export"): the question box over the morning's island map.
            play.host.corpse_win = Some(Box::new(move |cost: i32, can: bool| -> bool {
                // SAFETY: as for pay_win.
                let (cv, tx, ev, map, w) = unsafe { (&mut *cv, &mut *tx, &mut *ev, &*mp, &*wp) };
                eprintln!("corpse export asks ({} can pay {})", cost, can as u8);
                let under = map.payday_img(w, None, &pv_corpse.borrow()).unwrap_or_else(|c| fail("map", c));
                // panel.md "The travel animation": the wait row in the box, then the row of the answer.
                let (wr, yr, nr) = ja1front::dayloop::corpse_rows(can);
                confirm_win(cv, tx, ev, &cmouse, &dpal, &map.iso, &map.exe, &ja1front::dayloop::corpse_text(cost),
                            ja1front::dayloop::CORPSE_ARC, (wr, yr, Some(nr)), None, &under, Some(&cevents))
            }));
            // Op 297 raise_offer (strategic.md "Salary raise" step 3): the salary box over the morning's island map.
            let (riso, rexe) = (iso.to_string(), exe.to_string());
            let rmouse = sdl.mouse();
            play.host.raise_win = Some(Box::new(move |prof: u8, nick: &[u8], start: i16| -> i16 {
                // SAFETY: as for pay_win.
                let (cv, tx, ev, map, w) = unsafe { (&mut *cv, &mut *tx, &mut *ev, &*mp, &*wp) };
                eprintln!("window raise asks (profile {} start {})", prof, start);
                let under = map.payday_img(w, None, &pv_raise.borrow()).unwrap_or_else(|c| fail("map", c));
                let mut b = ja1front::salary::Screen::new(&riso, &rexe, prof, nick, start, &under)
                    .unwrap_or_else(|c| fail("salary box", c));
                // 0x29074: a mouse outside the box (52..267, 37..116) moves to (237, 102).
                let ms = ev.mouse_state();
                let (mx, my) = screen_pixel(cv, ms.x(), ms.y());
                if !(52..=267).contains(&mx) || !(37..=116).contains(&my) {
                    warp_frame(&rmouse, cv, 237, 102);
                }
                let r = raise_loop(cv, tx, ev, &mut b, &dpal);
                ja1front::sound::screen(|s| s.stop_all());
                r
            }));
            // Section 9 step 2: the office waits for Sleep (strategic.md "The office").
            let (oiso, oexe) = (iso.to_string(), exe.to_string());
            let sdir = play.day.save_dir.clone();
            let omouse = sdl.mouse();
            play.host.office_win = Some(Box::new(move |w: &mut ja1front::world::World, seed: &mut u32| -> i32 {
                // SAFETY: as for pay_win.
                let (cv, tx, ev) = unsafe { (&mut *cv, &mut *tx, &mut *ev) };
                let mut o = match ja1front::office::Office::new(&oiso, &oexe, w) {
                    Ok(o) => o,
                    Err(c) => {
                        eprintln!("office: error {}", c);
                        return 0;
                    }
                };
                eprintln!("office opens");
                let _pointer = OfficePointer::new();
                // strategic.md "Fades of the office": the room fades in (24 steps).
                fade_show(cv, tx, &o.picture(), &o.pal, 24, 0, 256);
                fn show(cv: &mut Frame, tx: &mut sdl2::render::Texture<'static>,
                        img: &[u8], pal: &[u8; 768]) {
                    blit(tx, img, &ja1front::eod::scale_palette(pal), SW, SH);
                    cv.copy(tx, None, None).expect("copy");
                    cv.present();
                }
                show(cv, tx, &o.picture(), &o.pal);
                let mut mouse = (0, 0);
                let mut hl_clock = ja1front::office::HighlightClock { timer: None, cycled: ja1front::dostime::now_ms() };
                loop {
                    std::thread::sleep(Duration::from_millis(10));
                    let (mut press, mut key_v, mut exit) = (false, false, false);
                    for evt in ev.poll_iter() {
                        match evt {
                            Event::Quit { .. } => return 1,
                            Event::MouseMotion { x, y, .. } => mouse = screen_pixel(cv, x, y),
                            Event::MouseButtonDown { mouse_btn: sdl2::mouse::MouseButton::Left, x, y, .. } => {
                                mouse = screen_pixel(cv, x, y);
                                press = true;
                            }
                            Event::KeyDown { keycode, keymod, .. } => match office_key(keycode, keymod, o.msg.is_some()) {
                                OfficeKey::V => key_v = true,
                                OfficeKey::Exit => exit = true,
                                OfficeKey::None => {}
                            },
                            _ => {}
                        }
                    }
                    // strategic.md "The office": Alt+X fades out (24 steps), the program ends with "Normal exit.".
                    if exit {
                        eprintln!("office exit");
                        ja1front::music::with(|m| m.set_volume(0, 1200));
                        fade_show(cv, tx, &o.picture(), &o.pal, 24, 256, 0);
                        println!("{}\n", ja1front::office::NORMAL_EXIT);
                        return 3;
                    }
                    // strategic.md "The office": more than 80 ms on the game clock (world_load.md "Game speed")
                    if o.cycle(ja1front::dostime::now_ms() - hl_clock.cycled) {
                        hl_clock.cycled = ja1front::dostime::now_ms();
                    }
                    let p = o.poll(mouse.0, mouse.1, press, key_v);
                    {
                        use ja1front::office::{play_frames, HighlightStep as H};
                        let mut pal = o.pal;
                        play_frames(&p, &mut pal, &mut hl_clock, &mut ja1front::dostime::now_ms, &mut |st, pal| match st {
                            H::Sfx { fx, rate, vol, loops } => ja1front::sound::screen(|s| s.sfx(fx, rate, vol, loops)),
                            H::Show(f) | H::Cycled(f) => show(cv, tx, f, pal),
                            // MousePoll? 0x678E8 in each pass of the wait: the pointer moves during the highlight frames
                            // (#64 rig: the original never stands still more than 58 ms on these sweeps).
                            H::Tick => {
                                ev.pump_events();
                                std::thread::sleep(Duration::from_millis(1));
                            }
                        });
                        o.pal = pal;
                    }
                    if let Some(k) = p.click {
                        eprintln!("office click {}", k);
                    }
                    if p.ledger {
                        // strategic.md "The team ledger": open until a left or right press, Return, Escape or Space.
                        // strategic.md "The team ledger", Fades: the room fades out, the page fades in (24 steps,
                        // one per vertical retrace).
                        let fades = |cv: &mut Frame,
                                     tx: &mut sdl2::render::Texture<'static>, img: &[u8], pal: &[u8; 768], out: bool| {
                            let (from, to) = if out { (256, 0) } else { (0, 256) };
                            fade_show(cv, tx, img, pal, 24, from, to);
                        };
                        fades(cv, tx, &o.picture(), &o.pal, true);
                        o.open_ledger(w);
                        eprintln!("office ledger");
                        // DrawOfficeLedger: the page shows shape 0, shape 202 again at its close.
                        pointer_shape(0);
                        fades(cv, tx, &o.picture(), &o.pal, false);
                        'ledger: loop {
                            std::thread::sleep(Duration::from_millis(10));
                            for evt in ev.poll_iter() {
                                match evt {
                                    Event::Quit { .. } => return 1,
                                    Event::MouseMotion { x, y, .. } => mouse = screen_pixel(cv, x, y),
                                    Event::MouseButtonDown { mouse_btn: sdl2::mouse::MouseButton::Left, .. }
                                    | Event::MouseButtonDown { mouse_btn: sdl2::mouse::MouseButton::Right, .. }
                                    | Event::KeyDown { keycode: Some(Keycode::Return | Keycode::Escape | Keycode::Space), .. }
                                        if !o.ledger_again_shown() => break 'ledger,
                                    // panel.md "Say it again (key L)": the box until a key or a click.
                                    Event::KeyDown { .. } | Event::MouseButtonDown { .. } if o.ledger_again_shown() => {
                                        o.ledger_again_end();
                                        show(cv, tx, &o.picture(), &o.pal);
                                    }
                                    Event::KeyDown { keycode: Some(Keycode::L), .. } => {
                                        if let Ok(Some((k, _))) = o.ledger_again(&oiso, &oexe, w) {
                                            eprintln!("office ledger again {}", k);
                                            if let Some((x, y)) = ja1front::question::prompt_warp(ja1front::ctlpanel::subtitles(w), mouse) {
                                                warp_frame(&cv.window().subsystem().sdl().mouse(), cv, x, y);
                                                eprintln!("prompt mouse {} {}", x, y);
                                            }
                                            show(cv, tx, &o.picture(), &o.pal);
                                        }
                                    }
                                    _ => {}
                                }
                            }
                        }
                        fades(cv, tx, &o.picture(), &o.pal, true);
                        o.close_ledger();
                        eprintln!("office ledger closed");
                        pointer_shape(202);
                        fades(cv, tx, &o.picture(), &o.pal, false);
                    }
                    if p.switchbox {
                        // strategic.md "The switch box (Control Panel)": left presses until the close lever.
                        let play_steps = |cv: &mut Frame,
                                          tx: &mut sdl2::render::Texture<'static>, steps: &[ja1front::switchbox::Step],
                                          pal: &[u8; 768]| {
                            for st in steps {
                                match st {
                                    ja1front::switchbox::Step::Show(img, ms) => {
                                        show(cv, tx, img, pal);
                                        // 90 ms frames wait on the game clock (world_load.md "Game speed")
                                        if *ms >= 90 {
                                            ja1front::dostime::sleep(*ms as i64);
                                        } else {
                                            std::thread::sleep(Duration::from_millis((*ms).max(12) as u64));
                                        }
                                    }
                                    ja1front::switchbox::Step::Sfx(fx, rate, vol, heard) => {
                                        if *heard {
                                            ja1front::sound::screen(|s| s.sfx(*fx, *rate, *vol, 1));
                                        }
                                    }
                                    ja1front::switchbox::Step::Music(on, _) => ja1front::music::with(|m| m.switch(*on)),
                                }
                            }
                        };
                        let steps = o.open_switchbox(w);
                        eprintln!("office switchbox");
                        // Test line (window_gameover_switches): the switch values that the box shows.
                        eprintln!("switches {}", (0..16).map(|k| ja1front::ctlpanel::get(w, k).to_string()).collect::<Vec<_>>().join(" "));
                        play_steps(cv, tx, &steps, &o.pal);
                        'sbox: loop {
                            std::thread::sleep(Duration::from_millis(10));
                            let mut at = None;
                            for evt in ev.poll_iter() {
                                match evt {
                                    Event::Quit { .. } => return 1,
                                    Event::MouseMotion { x, y, .. } => mouse = screen_pixel(cv, x, y),
                                    Event::MouseButtonDown { mouse_btn: sdl2::mouse::MouseButton::Left, x, y, .. } => {
                                        mouse = screen_pixel(cv, x, y);
                                        at = Some(mouse);
                                    }
                                    _ => {}
                                }
                            }
                            if let Some((x, y)) = at {
                                let mut steps = vec![];
                                let (t, closed) = o.switchbox_press(w, x, y, &mut steps);
                                play_steps(cv, tx, &steps, &o.pal);
                                for (k, v) in t {
                                    eprintln!("switchbox {} {}", k, v);
                                }
                                let fx_off = ja1front::ctlpanel::fx_off(w);
                                ja1front::sound::screen(|s| s.fx_off = fx_off);
                                if closed {
                                    eprintln!("switchbox closed");
                                    break 'sbox;
                                }
                            }
                        }
                    }
                    if let Some(m) = p.slots {
                        // strategic.md "Save Game and Restore Game": the slot list reads the buttons held and the
                        // typed keys until it ends.
                        o.open_slots(m, sdir.as_deref(), w);
                        eprintln!("slots open {}", m);
                        loop {
                            std::thread::sleep(Duration::from_millis(10));
                            let mut keys: Vec<u8> = vec![];
                            // A press seen in this batch counts as held in this poll (a short click falls between
                            // two polls otherwise).
                            let mut down = 0;
                            for evt in ev.poll_iter() {
                                match evt {
                                    Event::Quit { .. } => return 1,
                                    Event::MouseMotion { x, y, .. } => mouse = screen_pixel(cv, x, y),
                                    Event::MouseButtonDown { mouse_btn, x, y, .. } => {
                                        mouse = screen_pixel(cv, x, y);
                                        down |= match mouse_btn {
                                            sdl2::mouse::MouseButton::Left => 1,
                                            sdl2::mouse::MouseButton::Right => 2,
                                            _ => 0,
                                        };
                                    }
                                    Event::TextInput { text, .. } => keys.extend(text.bytes().filter(|b| b.is_ascii())),
                                    Event::KeyDown { keycode: Some(k), keymod, .. } => match k {
                                        _ if frame::ctrl_plain_char(k.into_i32(), keymod.bits()).is_some() => {
                                            keys.extend(frame::ctrl_plain_char(k.into_i32(), keymod.bits()))
                                        }
                                        _ if !frame::name_line_refused(k.into_i32(), keymod.bits()).is_empty() => {
                                            keys.extend(frame::name_line_refused(k.into_i32(), keymod.bits()))
                                        }
                                        Keycode::F24 => keys.push(frame::CTRL_LETTER.load(std::sync::atomic::Ordering::Relaxed)),
                                        Keycode::Backspace => keys.push(8),
                                        Keycode::Return | Keycode::KpEnter => keys.push(13),
                                        Keycode::Escape => keys.push(27),
                                        Keycode::Home => keys.extend([0, 0x47]),
                                        Keycode::Left => keys.extend([0, 0x4B]),
                                        Keycode::Right => keys.extend([0, 0x4D]),
                                        Keycode::End => keys.extend([0, 0x4F]),
                                        Keycode::Delete => keys.extend([0, 0x53]),
                                        _ => {}
                                    },
                                    _ => {}
                                }
                            }
                            let ms = ev.mouse_state();
                            let b = ms.left() as i32 | (ms.right() as i32) << 1 | down;
                            let r = o.slots_poll(mouse.0, mouse.1, b, &keys);
                            show(cv, tx, &o.picture(), &o.pal);
                            match r {
                                Some(ja1front::saveslots::Outcome::NoSaves) => {
                                    eprintln!("slots none");
                                    break;
                                }
                                Some(out) => {
                                    let sl = o.close_slots().expect("open list");
                                    if let Some(t) = ja1front::saveslots::busy_text(&sl, out) {
                                        show(cv, tx, &o.busy_picture(t), &o.pal);
                                    }
                                    let line = ja1front::saveslots::finish(w, &sl, out, sdir.as_deref(), &oiso, &oexe);
                                    eprintln!("{}", line);
                                    if line.starts_with("slots restored") {
                                        match ja1front::office::Office::new(&oiso, &oexe, w) {
                                            Ok(n) => o = n,
                                            Err(c) => eprintln!("office: error {}", c),
                                        }
                                    }
                                    break;
                                }
                                None => {}
                            }
                        }
                    }
                    if p.aim {
                        // strategic.md "The A.I.M. screen": open until EXIT (or Escape) ends it; the buttons held
                        // and the keys each poll. "Fades of the office": the office fades out first.
                        // The music fade (1200 ms) comes before the picture fade from field 2, after it from field 3.
                        if aim_music_fade_now(p.aim_fade_first, false) {
                            ja1front::music::with(|m| m.set_volume(0, 1200));
                        }
                        fade_show(cv, tx, &o.picture(), &o.pal, 24, 256, 0);
                        if aim_music_fade_now(p.aim_fade_first, true) {
                            ja1front::music::with(|m| m.set_volume(0, 1200));
                        }
                        match ja1front::aim::Aim::new(&oiso, &oexe, w, *seed) {
                            Ok((mut a, lines)) => {
                                a.repeat = true;
                                a.timed = true;
                                *seed = a.rng.seed;
                                for l in lines {
                                    eprintln!("{}", l);
                                }
                                // "The A.I.M. screen", Fades: the screen fades in (24 steps).
                                fade_show(cv, tx, &a.picture(), &a.pal, 24, 0, 256);
                                let mut info = a.info_open();
                                let mut info_last = (a.picture(), a.pal);
                                // The press on Contact A.I.M. does not count on the new screen: wait for the release.
                                // Keys pressed meanwhile (also during the fades) stay queued for the screen.
                                loop {
                                    ev.pump_events();   // the original's KeyPressed 0x5616C (kbhit) is this poll
                                    let ms = ev.mouse_state();
                                    if buttons_up(ms.left(), ms.right()) {
                                        break;
                                    }
                                    std::thread::sleep(Duration::from_millis(10));
                                }
                                loop {
                                    std::thread::sleep(Duration::from_millis(10));
                                    let mut keys: Vec<u8> = vec![];
                                    // As in the slot list: a press seen in this batch counts as held in this poll.
                                    let mut down = 0;
                                    for evt in ev.poll_iter() {
                                        if aim_event(&evt, |x, y| screen_pixel(cv, x, y), &mut mouse, &mut down, &mut keys) {
                                            return 1;
                                        }
                                    }
                                    let ms = ev.mouse_state();
                                    let b = mouse_bits(ms.left(), ms.right(), down);
                                    let r = a.poll(w, mouse.0, mouse.1, b, &keys);
                                    *seed = a.rng.seed;
                                    if let Some((x, y)) = a.warp.take() {
                                        mouse = (x, y);
                                        warp_frame(&omouse, cv, x, y);
                                    }
                                    for l in &r.lines {
                                        eprintln!("{}", l);
                                    }
                                    // strategic.md "Firing a merc": the fired A.I.M. merc's quote balloon at its face.
                                    if let Some((fa, under)) = r.fire.as_ref() {
                                        let p = fa.get(3).copied().unwrap_or(0);
                                        let pr = w.profiles.get(p as usize * 0x5A..p as usize * 0x5A + 10).unwrap_or(&[]);
                                        let nick: Vec<u8> = pr.iter().copied().take_while(|&b| b != 0).collect();
                                        let sub = ja1front::ctlpanel::sub_byte(w.update.opt_step_text);
                                        if !fire_balloon(&oiso, &oexe, fa, &nick, sub, under, &a.pal, cv, tx, ev) {
                                            return 1;
                                        }
                                    }
                                    // panel.md "Exit to DOS" (argument 0): the day files go when there is no
                                    // QUICK.SAV, then the program ends with the quit line.
                                    if let Some(line) = r.quit.as_ref() {
                                        if let Some(d) = sdir.as_ref() {
                                            if ja1front::question::exit_deletes(d) {
                                                eprintln!("aim quit: day files deleted");
                                            }
                                        }
                                        println!("{}\n", String::from_utf8_lossy(line));
                                        return 3;
                                    }
                                    if r.leave {
                                        // Leaving fades out (24 steps).
                                        ja1front::aim::Aim::leave_music();
                                        fade_show(cv, tx, &a.picture(), &a.pal, 24, 256, 0);
                                        ja1front::music::with(|m| m.stop_theme());
                                        break;
                                    }
                                    if a.info_open() != info {
                                        // "The A.I.M. statistics page": the screen shown fades out, the next one in.
                                        fade_show(cv, tx, &info_last.0, &info_last.1, 24, 256, 0);
                                        fade_show(cv, tx, &a.picture(), &a.pal, 24, 0, 256);
                                        info = a.info_open();
                                    }
                                    info_last = (a.picture(), a.pal);
                                    show(cv, tx, &info_last.0, &info_last.1);
                                }
                                // The link, the team and the faces changed: the office is drawn again and fades in.
                                match ja1front::office::Office::new(&oiso, &oexe, w) {
                                    Ok(n) => o = n,
                                    Err(c) => eprintln!("office: error {}", c),
                                }
                                // The mouse stays where the A.I.M. screen left it: the office redraws without a
                                // move, so no hot spot highlights until the mouse moves (OfficeScreen 0x4F2C1).
                                o.set_mouse(mouse.0, mouse.1);
                                fade_show(cv, tx, &o.picture(), &o.pal, 24, 0, 256);
                            }
                            Err(c) => eprintln!("aim: error {}", c),
                        }
                    }
                    show(cv, tx, &o.picture(), &o.pal);
                    if p.sleep {
                        eprintln!("office sleep");
        ja1front::music::with(|m| m.set_volume(0, 2400));
                        // "Fades of the office": 700 ms (a key or a click ends it early), then out over 56 steps.
                        if window_wait_ms(ev, 700, |_| {}).is_none() {
                            return 1;
                        }
                        fade_show(cv, tx, &o.picture(), &o.pal, 56, 256, 0);
                        return 0;
                    }
                    if p.leave {
                        eprintln!("office leave");
                        ja1front::music::with(|m| m.set_volume(0, 1200));
                        fade_show(cv, tx, &o.picture(), &o.pal, 56, 256, 0);
                        return 2;
                    }
                }
            }));
            if let Some(sr) = play.host.sunrise.as_mut() {
                // SAFETY: as for pay_win (the day steps run inside this round of the loop).
                sr.sink = Some(Box::new(WinEod { canvas: cv, tex: tx, events: ev, last: Instant::now(), audio: std::ptr::null(), clock: None, keys: Default::default() }));
            }
            let mut said_quit = false;
            {
                // #36 D: the day-start quotes show at their host op (302 / 317), inside the steps. The guide offer
                // (op 319) runs later in the same steps and its box waits for an answer; a front-end that played
                // the balloons after the steps showed the Elio box before Jack's line (the maintainer, day 2).
                let mpm = &mut map as *mut ja1front::mapui::MapUi;
                let wp = &e.world as *const ja1front::world::World;
                let cv = &mut canvas as *mut Frame;
                let tx = &mut tex as *mut sdl2::render::Texture<'_> as *mut sdl2::render::Texture<'static>;
                let ev = &mut events as *mut sdl2::EventPump;
                let qp = &mut said_quit as *mut bool;
                let (piso, pexe) = (iso.to_string(), exe.to_string());
                let held = dayend_last.clone();
                play.host.guide_say = Some(Box::new(move |op, a, under: Option<(&[u8], &[u8; 768])>| {
                    // SAFETY: as for the day start question below (day_steps runs inside this round of the loop).
                    let (map, w, cv, tx, ev) = unsafe { (&mut *mpm, &*wp, &mut *cv, &mut *tx, &mut *ev) };
                    let sub = ja1front::ctlpanel::sub_byte(w.update.opt_step_text);
                    // DayEndBankrupt? 0x62304 speaks after the payroll wait, before the fade at 0x65F9C: GuideSpeech?
                    // saves and restores the payroll screen under its balloon (POSTPAN palette), not the island map.
                    // DayEndVerdict 0x60756 speaks over the day replay screen in its own palette (`quote_under`).
                    let (base, pal) = match (under, held.borrow().as_ref()) {
                        (Some((s, p)), _) => (s.to_vec(), *p),
                        (None, Some(pic)) => (pic.clone(), dpal),
                        (None, None) => {
                            map.redraw(w).unwrap_or_else(|c| fail("map", c));
                            (map.img.clone(), map.pal)
                        }
                    };
                    if !guide_balloon(&piso, &pexe, op, a, sub, &base, &pal, cv, tx, ev) {
                        unsafe { *qp = true; }
                    }
                }));
            }
            let (m, d) = play.day_steps(&mut e.world);
            play.host.guide_say = None;
            if said_quit {
                break 'outer;
            }
            if let Some(sr) = play.host.sunrise.as_mut() {
                sr.sink = None;
            }
            play.host.guide_ask = None;
            play.host.payroll_show = None;
            play.host.dayend_win = None;
            play.host.dayend_fade = None;
            play.host.pay_win = None;
            play.host.pay_ask = None;
            play.host.corpse_win = None;
            play.host.office_win = None;
            if std::mem::take(&mut play.host.office_quit) {
                break 'outer;
            }
            if std::mem::take(&mut play.day.to_menu) {
                // world_load.md section 12: Leave ends the game; the main menu opens.
                to_menu = true;
                break 'outer;
            }
            if let Some(r) = play.day.verdict {
                eprintln!("verdict {} rating {}", r, play.day.rating.map(ja1front::dayloop::rating_word).unwrap_or("-"));
            }
            if let Some(r) = play.day.game_over {
                // ending.md "Game end": the end show of the code, the game-over screen, a new game with the same
                // difficulty, the main sound bank again; then the main menu.
                eprintln!("game over {}", r);
                if r == 1 {
                    // End code 1: the win sequence and the credits, the rating text, then the program ends.
                    let (v, score) = {
                        let mut se = ja1front::play::Session::new(&mut e.world, &mut play.host);
                        (se.win_vars(), se.performance_rating())
                    };
                    let t = ja1front::ending::rating_text(score);
                    win_window(iso, exe, ja1front::ctlpanel::sub_byte(e.world.update.opt_step_text), v, &t, &mut canvas, &mut tex, &mut events);
                    println!("{}", t);
                    exit(0);
                }
                end_window(iso, exe, ja1front::ctlpanel::sub_byte(e.world.update.opt_step_text), r, &mut canvas, &mut tex, &mut events);
                if ja1logic::fanfix::on(62) {
                    // Fan fix F62 (#70 row 72): after the game-over screen, the rating text; the program ends.
                    let score = ja1front::play::Session::new(&mut e.world, &mut play.host).performance_rating();
                    println!("{}", ja1front::ending::rating_text(score));
                    exit(0);
                }
                e.world = ja1front::ending::new_game_after(iso, exe, &e.world).unwrap_or_else(|m| die(format!("new game: {}", m)));
                ja1front::sound::screen_main(iso, exe);
                to_menu = true;
                break 'outer;
            }
            eprintln!("day morning {} day {}", m.map(|m| m.to_string()).unwrap_or("-".into()), d);
            // DayEndMercUpdate? 0x44197 (StrategicScreen, before the guide): the island map fades in over 24 steps.
            map.redraw(&e.world).unwrap_or_else(|c| fail("map", c));
            let mpal = map.pal;
            fade_show8(&mut canvas, &mut tex, &map.img, &mpal, 24, 0, 256);
            eprintln!("island map faded in");
            // panel.md "The guide's and Brenda's balloons": the quotes of the day's steps over the island map.
            // SPEC GAP (front-end): the verdict's quote shows over the map too (the port draws no verdict screen).
            let said = std::mem::take(&mut play.host.guide_said);
            if !said.is_empty() {
                map.redraw(&e.world).unwrap_or_else(|c| fail("map", c));
                let base = map.img.clone();
                for (op, a) in said {
                    let sub = ja1front::ctlpanel::sub_byte(e.world.update.opt_step_text);
                    if op == 328 {
                        let under = map.fire_base.take().unwrap_or_else(|| base.clone());
                        let p = a.get(3).copied().unwrap_or(0);
                        let pr = e.world.profiles.get(p as usize * 0x5A..p as usize * 0x5A + 10).unwrap_or(&[]);
                        let nick: Vec<u8> = pr.iter().copied().take_while(|&b| b != 0).collect();
                        if !fire_balloon(iso, exe, &a, &nick, sub, &under, &map.pal, &mut canvas, &mut tex, &mut events) {
                            break 'outer;
                        }
                        continue;
                    }
                    if !guide_balloon(iso, exe, op, &a, sub, &base, &map.pal, &mut canvas, &mut tex, &mut events) {
                        break 'outer;
                    }
                }
            }
            map.day = true;
            open_map = true;
        }
        if start_day {
            // Section 9 step 4, Enter: the start checks, then the day start; op 303 waits for Y / N / Escape.
            // Section 9 step 4, Enter: the start checks, then the day start; op 303 asks in the question box
            // (panel.md "Question box"): pictures with no button / NO / OK pressed, per kind.
            let mut imgs = Vec::new();
            for kind in 0..1 {
                let (text, pic) = ja1front::dayloop::ask_question(kind);
                for pressed in [None, Some(0), Some(1)] {
                    imgs.push(map.question_img(&e.world, text, pic, pressed).unwrap_or_else(|c| fail("map", c)));
                }
            }
            map.redraw(&e.world).unwrap_or_else(|c| fail("map", c));
            let mpal = map.pal;
            let cv = &mut canvas as *mut Frame;
            let tx = &mut tex as *mut sdl2::render::Texture<'_> as *mut sdl2::render::Texture<'static>;
            let ev = &mut events as *mut sdl2::EventPump;
            let map_img = map.img.clone();
            let (ai, ae) = (iso.to_string(), exe.to_string());
            let amouse = sdl.mouse();
            let ask = Box::new(move |v: [i32; 4]| -> u32 {
                let kind = v[0];
                let base = 0;
                // panel.md "The travel animation": kind 1 plays the wait row in the box, after yes the go row.
                // SAFETY: the window, texture and event pump outlive this call (it runs inside day_map_start
                // below, and nothing else uses them meanwhile).
                let (cv, tx, ev) = unsafe { (&mut *cv, &mut *tx, &mut *ev) };
                let yes = if kind == 1 {
                    confirm_win(cv, tx, ev, &amouse, &mpal, &ai, &ae, ja1front::dayloop::ask_question(1).0,
                                ja1front::sanim::DAY_ARC, (v[1], v[2], None), Some(v[3]), &map_img, None)
                } else {
                    question_box(cv, tx, ev, &imgs[base..base + 3], &mpal)
                };
                yes as u32
            });
            let (m, r) = play.day_map_start(&mut e.world, start_button, ask);
            eprintln!("start {} {}", m, r.map(|r| r.to_string()).unwrap_or("-".into()));
            map.set_msg(&e.world, m).unwrap_or_else(|c| fail("map", c));
            if r == Some(1) {
                // StrategicScreen 0x43E9E: the music volume, the 24 step fade of the map, the stop.
                ja1front::music::with(|m| m.set_volume(0, 1200));
                fade_show8(&mut canvas, &mut tex, &map.img, &mpal, 24, 256, 0);
                ja1front::music::with(|m| m.stop_theme());
                play.day_enter(&mut e.world, iso, exe).unwrap_or_else(|m| die(m));
                e.tileset = play.day.tileset;
                pal = palette(iso, e.tileset as i32).unwrap_or_else(|c| fail("palette", c));
                title = format!("ja1front: sector {} (cur_sector {})", e.world.cur_sector as i32 + 1, e.world.cur_sector);
                (c0, r0) = ja1front::enter::view_tile(&e.world);
                // Sector entry: the face buffers again (panel.md "The front-end's tactical screen").
                *screen = ja1front::screen::Screen::new(iso, exe, &mut e.world).unwrap_or_else(|c| fail("faces", c));
                // LoadTacticalSector? 0x2B21C (EAX 0): the new sector fades in over 24 steps, and FlushKeys 0x2B239 drops
                // what came meanwhile. A Quick Restore does neither. The timer starts only after the load (0x107F5).
                let idx = play_frame(iso, exe, e, screen, c0, r0, &mut play, false);
                eprintln!("sector fade in");
                fade_show8(&mut canvas, &mut tex, &idx, &pal, 24, 0, 256);
                if events.poll_iter().fold(false, |q, ev| q || matches!(ev, Event::Quit { .. })) {
                    break 'outer;
                }
                eprintln!("tactical ready day {} sector {}", e.world.day, e.world.cur_sector);
                map.open = false;
                map.day = false;
                drawn_left = false;
                ticks_done = ja1front::dostime::game_ticks(start.elapsed());
                continue;
            }
        }
        if key_m_refused(open_map, map.day, ja1front::guardopen::key_m_opens(e.world.combat, e.world.update.screen_drawn)) {
            // TacticalKeyHandler 0x13E78: in the enemies' turn the screen is off and key M does nothing.
        } else if open_map && !map.day {
            // strategic.md "The guard screen": tactical key M and the top strip's map button open it; the mouse
            // moves to the cell of cur_sector.
            for k in ja1front::guardopen::ENTER {
                play.guard_step(&mut e.world, k);
                match guard_window_step(k) {
                    GuardWin::HidePointer => pointer_visible(false),
                    GuardWin::Fade(n, f, t) => {
                        let (c, r) = ja1front::enter::view_tile(&e.world);
                        let idx = play_frame(iso, exe, e, screen, c, r, &mut play, false);
                        fade_show8(&mut canvas, &mut tex, &idx, &pal, n, f, t);
                    }
                    GuardWin::Library => {}
                }
            }
            if map.guard_open(&mut e.world).unwrap_or_else(|c| fail("guard screen", c)) {
                eprintln!("guard screen open");
                if let Some((sx, sy)) = map.warp.take() {
                    warp_frame(&sdl.mouse(), &canvas, sx, sy);
                }
                // GuardAssignScreen? 0x4290C: the mouse moves, then the screen fades in, then the first pass.
                let mpal = map.pal;
                fade_show8(&mut canvas, &mut tex, &map.img, &mpal, 24, 0, 256);
                eprintln!("guard screen faded in");
                map.guard_pass(&mut e.world).unwrap_or_else(|c| fail("guard screen", c));
                pointer_visible(true);
            } else {
                guard_leave = true;
            }
        } else if open_map {
            let ms = events.mouse_state();
            let at = screen_pixel(&canvas, ms.x(), ms.y());
            map.show(&mut e.world, at).unwrap_or_else(|c| fail("island map", c));
            if map.day {
                // Section 9: the last "... seems to have disappeared!" of the strategic screen opening.
                if let Some(t) = play.day.gone_text.take() {
                    map.set_text(&e.world, t).unwrap_or_else(|c| fail("island map", c));
                }
            }
        }
        // Op 344 cursor_to_soldier: a pick-up put the item on the cursor; the merc's inventory screen opens.
        if play.host.pick_inv.is_some() {
            open_inv = true;
        }
        if open_inv {
            // Section 8 "Inventory screen": the game stops while it is open (its op 228 Lock, panel.md "Other users of
            // Lock"); the time meanwhile runs as locked ticks when it closes (fan fix F65: as round_lock2 ticks).
            let ms = events.mouse_state();
            let mouse = screen_pixel(&canvas, ms.x(), ms.y());
            let mut input = WinInput { sdl_mouse: sdl.mouse(), canvas: &mut canvas, tex: &mut tex, events: &mut events, pal: &pal, buttons: 0, release: 0, mouse,
                                       arrow: None, audio: audio.as_ref(), clock: None };
            let r = play.inventory(&mut e.world, iso, exe, e.tileset, &screen.faces, &mut screen.table0, &mut input);
            // Back on the map: the tile cursor (pointer hidden) or the pointer, as the mouse stands.
            pointer_visible(ja1front::pointer::tactical_pointer_shows(play.mouse.is_some(), e.world.held[0], play.host.mouse_hold, e.world.combat));
            if r == Some(1) {
                break 'outer;
            }
            inputs.clear();
            let now = ja1front::dostime::game_ticks(start.elapsed());
            // 1.13b: the time after the first message box in the screen is plain time (play.rs inv_ticks).
            let open_from = play.host.inv_box_end.take().map_or(now, |t| ja1front::dostime::game_ticks(t.saturating_duration_since(start)).max(ticks_done).min(now));
            play.inv_ticks(&mut e.world, open_from.saturating_sub(ticks_done), now - open_from);
            ticks_done = now;
        }
        if let Some(mut g) = map.guard_take_closing() {
            // GuardAssignScreen? 0x431E5: the pointer goes, then the screen fades out.
            pointer_visible(false);
            let mpal = map.pal;
            fade_show8(&mut canvas, &mut tex, &map.img, &mpal, 24, 256, 0);
            eprintln!("guard screen faded out");
            let sent: i32 = g.moves_n.iter().map(|&n| n as i32).sum();
            play.guard_close(&mut e.world, &mut g);
            eprintln!("guard screen closed: {} guards sent, {} here", sent, g.here);
            guard_leave = true;
            inputs.clear();
            ticks_done = ja1front::dostime::game_ticks(start.elapsed());
        }
        if guard_leave {
            for k in ja1front::guardopen::LEAVE {
                play.guard_step(&mut e.world, k);
                if let ja1front::guardopen::Step::Fade(n, f, t) = k {
                    let (c, r) = ja1front::enter::view_tile(&e.world);
                    let idx = play_frame(iso, exe, e, screen, c, r, &mut play, false);
                    fade_show8(&mut canvas, &mut tex, &idx, &pal, n, f, t);
                    eprintln!("guard screen fade in");
                }
            }
            // GuardScreenFromTactical 0x1331B: the timer flag 0x92DE1 stays on through this fade, so its time is not game time.
            ticks_done = ja1front::dostime::game_ticks(start.elapsed());
            after_guard = true;
            pointer_visible(ja1front::pointer::tactical_pointer_shows(play.mouse.is_some(), e.world.held[0], play.host.mouse_hold, e.world.combat));
        }
        if map.open {
            // The flips and the assignment slide-in (MapUi::anim_pointer): no pointer during a flip (PlayFlipAnim's
            // callers hide it), shape 201 through the slide-in, shape 0 again at its end (0x47BD2, 0x47CE4).
            let ap = map.anim_pointer();
            if POINTER_FLIP.with(|f| f.replace(ap == Some(-1))) != (ap == Some(-1)) {
                eprintln!("pointer flip {}", (ap == Some(-1)) as u8);
            }
            if ap == Some(201) {
                pointer_shape(201);
            } else if slide_shape_ends(POINTER_BASE.with(|b| b.get()), map.assign_open()) {
                pointer_shape(0);
            }
            // #20 D: the assignment screen draws a held item with its hand at the mouse; the system arrow is off then.
            let over = map.pointer_off_over_map();
            if POINTER_MAP.with(|f| f.replace(over)) != over {
                eprintln!("pointer map {}", over as u8);
            }
            pointer_visible(ja1front::pointer::map_pointer_shows(map.item_pointer(&e.world), ap, over));
            // The island map: the game stops; the time that passes meanwhile does not count.
            inputs.clear();
            ticks_done = ja1front::dostime::game_ticks(start.elapsed());
            map.box_input(BoxInput::Time).unwrap_or_else(|c| fail("message box", c));
            // ShowMessage? 0x27B98 on the map, as in the tactical box below.
            let map_box = map.box_shown();
            if !map_box && POINTER_BOX.with(|v| v.get()) {
                play.cursor.tile = 0xC41;
            }
            // The held item on the assignment screen is the pointer too: it leaves the picture with the box's start
            // and comes back at its end.
            if box_change_redraws(map_box, POINTER_BOX.with(|v| v.get()), e.world.held[0]) {
                map.redraw(&e.world).unwrap_or_else(|c| fail("map", c));
            }
            pointer_box(map_box);
            let ms = events.mouse_state();
            if map.sound_hold_over(any_input(false, ms.left(), ms.right())) {
                // The screen's loop reads the mouse again: the hover of the place where the mouse is now.
                let at = screen_pixel(&canvas, ms.x(), ms.y());
                map.track(&mut e.world, at).unwrap_or_else(|c| fail("map mouse", c));
            }
            map.guard_pass(&mut e.world).unwrap_or_else(|c| fail("guard screen", c));
            map.after_hold(&mut e.world).unwrap_or_else(|c| fail("map mouse", c));
            if map.cycle(ja1front::dostime::now_ms()) {
                eprintln!("map water cycle");
            }
            if let Ok(png) = std::env::var("JA1_MAP_PNG") {
                write_png_wh(&png, &map.img, &map.pal, SW, SH).unwrap_or_else(|m| die(format!("{}: {}", png, m)));
            }
            if map.day {
                // Section 9 step 0: the rating word of the day before.
                let rating = play.day.rating.map(|r| format!(" | rating {}", ja1front::dayloop::rating_word(r)));
                canvas.window_mut().set_title(&format!("{} | Day {}{}", title, e.world.day, rating.unwrap_or_default())).ok();
            }
            if map.anim.is_empty() {
                map.anim_step(&e.world).unwrap_or_else(|c| fail("map", c));
            }
            // The wage question ends by a key or by the last frame step: the mouse comes back (0x67E30).
            if let Some((sx, sy)) = map.warp.take() {
                warp_frame(&sdl.mouse(), &canvas, sx, sy);
            }
            blit(&mut tex, map.shown(), &map.pal, SW, SH);
            canvas.copy(&tex, None, None).expect("copy");
            canvas.present();
            if map.anim.is_empty() {
                std::thread::sleep(Duration::from_millis(20));
            } else {
                // strategic.md "The finance page": one picture step = 60 ms on the game clock; a slide step 1 ms.
                ja1front::dostime::sleep(map.anim_ms.front().copied().unwrap_or(60));
                map.tick();
            }
            continue;
        }
        // panel.md "The switch view": 3 frames of 20 ms open it; the close runs the frames backwards, then the
        // slot again, the time passed runs as locked ticks (clock switch 1).
        let mut switch_anim = None;
        if let Some(sw) = switch_ui.as_mut() {
            let ms = sw.t.elapsed().as_millis();
            let k = sw.k as usize * ja1front::soldier::REC + 0x17F;
            match sw.step {
                SwStep::Open => match ja1front::panel::slot_view_frame(1, ms) {
                    Some(f) => switch_anim = Some(f as usize),
                    None => {
                        e.world.soldiers[k] = 1;
                        sw.step = SwStep::Up;
                    }
                },
                SwStep::Close => match ja1front::panel::slot_view_frame(0, ms) {
                    Some(f) => {
                        e.world.soldiers[k] = 0;
                        switch_anim = Some(f as usize);
                    }
                    None => {
                        switch_ui = None;
                        let now = ja1front::dostime::game_ticks(start.elapsed());
                        play.locked_ticks(&mut e.world, true, now.saturating_sub(ticks_done), false);
                        ticks_done = now;
                    }
                },
                _ => {}
            }
        }
        // cursor.md "Drag to an edge": one poll of the loop of AllMoveTraverse? 0x1CFEC per main loop, buttons as levels.
        if play.drag.is_none() {
            drag_drawn = None;
        }
        if play.drag.is_some() {
            // SAFETY: SDL is up; null pointers ask for the buttons only.
            let state = unsafe { sdl2::sys::SDL_GetMouseState(std::ptr::null_mut(), std::ptr::null_mut()) };
            let buttons = (state & 1) as u8 | if state & 4 != 0 { 2 } else { 0 };
            let (mx, my) = play.mouse_px;
            let lines0 = play.day.lines.len();
            play.cursor = screen.cursor;
            let go = play.drag_step(&mut e.world, mx, my, buttons);
            screen.cursor = play.cursor;
            for l in play.day.lines[lines0..].iter().filter(|l| l.starts_with("allmove")) {
                eprintln!("{}", l);
            }
            inputs.extend(go);
        }
        if shown_selected != e.world.selected {
            shown_selected = e.world.selected;
            eprintln!("selected {}", shown_selected);
        }
        if shown_view != e.world.opt_onscreen {
            shown_view = e.world.opt_onscreen;
            eprintln!("view mode {}", shown_view);
        }
        // A button press draws its face in one pass, then only the timer runs.
        let face_waits = face_waits(screen.pressed.is_some(), strip_face_shown);
        strip_face_shown = screen.pressed.is_some();
        if (map_press || strip_wait != 0 || face_waits) && inputs.is_empty() && held.is_none() && stat_keys.is_none() && switch_ui.is_none() {
            // WaitLeftUp: only the timer runs, the picture stays, and keys wait for the release.
            let now = ja1front::dostime::game_ticks(start.elapsed());
            play.cursor = screen.cursor;
            play.drag_ticks(&mut e.world, now.saturating_sub(ticks_done));
            screen.cursor = play.cursor;
            ticks_done = now;
            std::thread::sleep(Duration::from_millis(10));
            continue;
        }
        if play.drag.is_some() && held.is_none() && stat_keys.is_none() && switch_ui.is_none() {
            // The game stays in the loop: only the timer runs. Keys and other mouse events wait.
            inputs.clear();
            let now = ja1front::dostime::game_ticks(start.elapsed());
            play.cursor = screen.cursor;
            play.drag_ticks(&mut e.world, now.saturating_sub(ticks_done));
            screen.cursor = play.cursor;
            ticks_done = now;
            std::thread::sleep(Duration::from_millis(10));
            // The loop redraws only when the arrow changes; a timer step (the walk marker's end, a blink) shows after it.
            let shown = play.drag.and_then(|d| d.loop_.shown);
            if drag_drawn == Some(shown) {
                continue;
            }
            drag_drawn = Some(shown);
        } else if press_stops_the_game(held.is_some(), stat_keys.is_some(), switch_ui.is_some()) {
            // The game stops (no turns, no animation, no other input) until the button is up; the time that
            // passes meanwhile runs as locked ticks at the release.
            inputs.clear();
            std::thread::sleep(Duration::from_millis(10));
        } else if play.left {
            // Play stops; the window stays until Escape (the view can still move).
            inputs.clear();
            if drawn_left {
                std::thread::sleep(Duration::from_millis(20));
                continue;
            }
            drawn_left = true;
        } else {
            // Game time: one tick per 10 ms of real time; a pass with no tick due waits for the next one.
            let due = ja1front::dostime::game_ticks(start.elapsed());
            if due <= ticks_done {
                let next = Duration::from_millis((ticks_done + 1) * 10);
                if let Some(wait) = next.checked_sub(start.elapsed()) {
                    std::thread::sleep(wait.min(Duration::from_millis(10)));
                }
                continue;
            }
            // At most 10 ticks per pass; the rest run in the next passes.
            let n = due.saturating_sub(ticks_done).min(10);
            if std::mem::take(&mut after_guard) {
                eprintln!("ticks after guard screen {}", n);
            }
            if let Some(eod) = play.host.eod.as_mut() {
                // SAFETY: the sink's pointers stay valid during this pass; the pass is the only user meanwhile.
                eod.sink = Some(Box::new({
                    WinEod {
                        canvas: &mut canvas,
                        tex: &mut tex as *mut sdl2::render::Texture<'_> as *mut sdl2::render::Texture<'static>,
                        events: &mut events,
                        last: Instant::now(),
                        audio: audio.as_ref().map_or(std::ptr::null(), |q| q as *const _),
                        clock: None,
                        keys: Default::default(),
                    }
                }));
            }
            // The pass's hover step works on the screen's cursor state (the drawing uses it after the pass).
            play.cursor = screen.cursor;
            let view0 = (e.world.view_x, e.world.view_y, e.world.opt_onscreen);
            {
                // TacticalKeyHandler reads the keyboard flags at each pass.
                use sdl2::keyboard::Mod as M;
                let ms = sdl.keyboard().mod_state();
                play.shift_alt = (ms.intersects(M::LSHIFTMOD | M::RSHIFTMOD), ms.intersects(M::LALTMOD | M::RALTMOD));
                let (cm, am) = (ms.intersects(M::LCTRLMOD | M::RCTRLMOD), play.shift_alt.1);
                let old = play.cursor.mode;
                play.key_flags = play.shift_alt.0 as u16 | (cm as u16) << 2 | (am as u16) << 3;
                // Port option (2026-11-25): a released pointer outside the focused window keeps the edge scroll.
                play.edge_out = None;
                let win = canvas.window();
                let focus = win.window_flags() & sdl2::sys::SDL_WindowFlags::SDL_WINDOW_INPUT_FOCUS as u32 != 0;
                if play.mouse_out && focus && !frame::dos_captured() {
                    let (mut gx, mut gy) = (0, 0);
                    unsafe { sdl2::sys::SDL_GetGlobalMouseState(&mut gx, &mut gy) };
                    let ((wx, wy), (ww, wh)) = (win.position(), win.size());
                    if let Some((x, y)) = ja1front::dosmouse::outside_edge((wx, wy, ww, wh), (gx, gy)) {
                        play.edge_out = Some(screen_pixel(&canvas, x, y));
                    }
                }
                play.pass(&mut e.world, &inputs, n as u32);
                // The ticks of the balloons that waited inside the pass (balloon_win).
                play.host.balloon_shown = false;
                let (drag, held) = ja1front::balloon::owed_after_pass(std::mem::take(&mut play.host.balloon_owed), &mut ticks_done);
                if let Some(drag) = drag {
                    play.drag_ticks_ui(&mut e.world, drag);
                }
                if let Some(held) = held {
                    play.locked_ticks_ui(&mut e.world, held);
                    // The view that the library's world held at the end of the pass, after the balloon's wait.
                    eprintln!("pass end view {} {}", e.world.view_x, e.world.view_y);
                }
                if play.cursor.mode != old {
                    eprintln!("cursor mode {} value {:#x}", play.cursor.mode, play.cursor.value);
                }
            }
            screen.cursor = play.cursor;
            // Op 206 pal_cycle (Pal Cycling): the water and fire colours of the tactical palette move.
            for (f, l) in play.host.pal_rot.drain(..) {
                ja1front::eod::rotate(&mut pal, f, l);
            }
            // Op 99 focus or a view mode moved the view (world_load.md section 8 "Focus", panel.md "View modes"):
            // the window follows.
            if (e.world.view_x, e.world.view_y, e.world.opt_onscreen) != view0 {
                (c0, r0) = ja1front::enter::view_tile(&e.world);
                let (mx, my) = play.mouse_px;
                let at_edge = play.edge_out.is_some() || (!play.mouse_out && (mx < 1 || mx > 318 || my < 1 || my > 198));
                if at_edge || inputs.iter().any(|i| matches!(i, Input::Arrow(_))) {
                    eprintln!("window scroll x {} y {}", e.world.view_x, e.world.view_y);
                }
            }
            // End of a turn (0x121D8): the pointer is off and the mouse does not count until the next turn (op 157).
            let hold_now = play.host.mouse_hold && e.world.combat != 0;
            if hold_now {
                pointer_visible(false);
                // Check aid (window_turn_win): when the file JA1_ENEMY_TURN_WIN exists, the sector row has no enemies. Play has no
                // sure route to a death in the enemies' turn; the same write in the original gave the proof on #45.
                if std::env::var_os("JA1_ENEMY_TURN_WIN").is_some_and(|f| std::path::Path::new(&f).exists()) {
                    let row = e.world.cur_sector as usize * 9 + 1;
                    e.world.sector_state[row] = 0;
                }
            }
            // A combat end in the enemies' turn (0x10A01) has no MouseRestorePos2?: the next MousePoll? reads the place of the
            // mouse driver, and a move in that turn counts (live on #45, fight59).
            let hold_over = play.host.mouse_hold && e.world.combat == 0;
            if hold_over {
                play.host.mouse_hold = false;
                let ms = events.mouse_state();
                play.mouse_px = screen_pixel(&canvas, ms.x(), ms.y());
                eprintln!("mouse after combat {} {}", play.mouse_px.0, play.mouse_px.1);
            }
            if tile_log && tile_seen != Some(play.cursor.tile) {
                tile_seen = Some(play.cursor.tile);
                eprintln!("cursor tile {}", play.cursor.tile);
            }
            // The first MousePoll? after 0x92DCC comes back reads the driver's place, unless a mouse move of the game comes first.
            let reads = ja1front::slotmouse::pass_runs(e.world.update.screen_drawn);
            if reads != mouse_reads {
                mouse_reads = reads;
                if reads && play.mouse_warp.is_none() {
                    let ms = events.mouse_state();
                    play.mouse_px = screen_pixel(&canvas, ms.x(), ms.y());
                }
                // Check aid (window_balloon_press): a sighting at the start of the mercs' turn. The same write in the
                // original gave the proof on #31 comment 11937 (build/probe/live_turns3.py).
                if turn_sight_aid(reads, std::env::var_os("JA1_TURN_SIGHT").is_some()) {
                    play.host.sight = ja1front::sighting::Sight { slot: e.world.selected, how: 0, near: 0x40 };
                    eprintln!("turn sight {}", e.world.selected);
                }
                eprintln!("mouse reads {} at {} {}", reads as u8, play.mouse_px.0, play.mouse_px.1);
            }
            let warp = play.mouse_warp.take();
            if let Some((x, y)) = warp {
                warp_frame(&sdl.mouse(), &canvas, x, y);
            }
            if let Some((x, y)) = warp.or(hold_over.then_some(play.mouse_px)) {
                if !hold_now {
                    // The place of the mouse after its return decides the pointer; SDL sends no move when it did not change.
                    let at = spot(&e.world.soldiers, x, y, vw_now(&e.world).0, vw_now(&e.world).1, e.world.opt_onscreen);
                    if let Spot::View(vx, vy) = at {
                        play.set_mouse(Some(map_tile(&e.world, c0, r0, vx, vy)));
                    } else {
                        play.set_mouse(None);
                    }
                    pointer_visible(ja1front::pointer::tactical_pointer_shows(play.mouse.is_some(), e.world.held[0], play.host.mouse_hold, e.world.combat));
                }
            }
            if let Some(eod) = play.host.eod.as_mut() {
                eod.sink = None;
            }
            inputs.clear();
            ticks_done += n;
            // Check aid (window_held_slide): when the file JA1_SLIDE_TEST exists, one slide of the view. The walk that brings the
            // slide with an item on the cursor and the mouse off the map depends on the frame timing of an interrupt.
            if let Some(f) = std::env::var_os("JA1_SLIDE_TEST").filter(|f| std::path::Path::new(f).exists()) {
                let _ = std::fs::remove_file(f);
                let to = (e.world.view_x, e.world.view_y);
                let steps = (to.0 / 16).min(25);
                ja1front::focus::push_slide(ja1front::focus::Slide { from: (to.0 - 16 * steps, to.1), target: to, steps: steps as u32 });
            }
            // first after the ripple timer ran out), each step drawn; the timer runs meanwhile (no frames).
            // A slide of a balloon's view move plays after the balloon (show_balloons).
            for sl in play.host.reactions.iter().filter_map(|r| r.undo.as_ref().and_then(|u| u.slide)) {
                ja1front::focus::hold_slide(&sl);
            }
            if !window_slides(iso, exe, e, screen, &mut play, &mut canvas, &mut tex, &mut events, &pal, &mut ticks_done) {
                break 'outer;
            }
            // JA1_SPEED_LOG=1: `speed MS SLOT TILE` when the selected merc's tile changes (validate/smoke_play.py
            // window_speed times the walk with it).
            if speed_log {
                let k = (e.world.selected as usize).min(47);
                let t = ja1front::play::soldier_tile(&e.world, k);
                if last_tile != Some((k, t)) {
                    eprintln!("speed {} {} {}", start.elapsed().as_millis(), k, t);
                    last_tile = Some((k, t));
                }
                // `speed clock MS MINUTES COMPRESS` when the day clock moves (validate/gate_speed.py times a day
                // with Time compression: one turn of 5 minutes per 200 ms).
                if last_clock != Some(e.world.clock.minutes) {
                    eprintln!("speed clock {} {} {}", start.elapsed().as_millis(), e.world.clock.minutes, e.world.update.time_compress);
                    last_clock = Some(e.world.clock.minutes);
                }
                // `speed hover_ap MS TILE AP`: the AP number of the path marks shows MS after the mouse came to the
                // tile (#29 H: the original at cycles=max ~40 ms).
                let ct = screen.cursor.tile;
                if hover_seen.0 != ct {
                    hover_seen = (ct, start.elapsed().as_millis(), false);
                }
                if screen.cursor.ap != 0 && !hover_seen.2 {
                    eprintln!("speed hover_ap {} {} {}", start.elapsed().as_millis() - hover_seen.1, ct, screen.cursor.ap);
                    hover_seen.2 = true;
                }
                // `speed ap_blink MS PHASE FG` when the drawn AP number's blink phase changes (#29 H: a cost above
                // the merc's AP flashes, fg 1 / 0xFF every 200 ms, cursor.md "Cursor sprite").
                if screen.cursor.ap != 0 && last_blink != Some(screen.cursor.blink_phase) {
                    eprintln!("speed ap_blink {} {} {}", start.elapsed().as_millis(), screen.cursor.blink_phase, screen.cursor.ap_fg);
                    last_blink = Some(screen.cursor.blink_phase);
                }
            }
            // strategic.md "The battle show": the boxes, sounds and waits of the pass in order; a key or a click ends
            // the rest of the show (the battle is already decided). No game time passes meanwhile.
            let show = std::mem::take(&mut play.host.battle_show);
            if !show.is_empty() {
                // At the end of a sector (the Leave path faded the screen out first) the palette stays black: only
                // the sounds and the waits of the show come through.
                // Fan fix F28: the leave path's fade waits for the show (fade_held): the show is lit, then it fades.
                let held = play.host.fade_held.take();
                let dark = play.left && held.is_none();
                let black = [0u8; 768];
                let show_pal = if dark { &black } else { &pal };
                eprintln!("window battle {} steps{}", show.len(), if dark { " dark" } else { "" });
                let base = play_frame(iso, exe, e, screen, c0, r0, &mut play, false);
                let mut shown_box = base.clone();
                'show: for ev in show {
                    let ms = match ev {
                        ja1front::play::BattleEv::Sound(n) => {
                            play.host.sound.battle(&n);
                            0
                        }
                        ja1front::play::BattleEv::Stop => {
                            play.host.sound.speech_stop();
                            0
                        }
                        ja1front::play::BattleEv::WaitSound => 0,
                        ja1front::play::BattleEv::ClearKey => {
                            LAST_KEY.with(|v| v.set(0));
                            0
                        }
                        ja1front::play::BattleEv::Wait(ms) => ms as u64,
                        ja1front::play::BattleEv::Box(s, en, g, t, k) => {
                            shown_box = base.clone();
                            ja1front::battlebox::draw_tactical(iso, exe, e.tileset, s, (en, g, t), k, &mut shown_box)
                                .unwrap_or_else(|c| fail("battle box", c));
                            0
                        }
                    };
                    blit(&mut tex, &shown_box, show_pal, SW, SH);
                    canvas.copy(&tex, None, None).expect("copy");
                    canvas.present();
                    // Only Escape skips the rest of the show; another key or a click ends this wait alone (WaitMs?).
                    if ms > 0 {
                        let mut mixed = 0u128;
                        let sound = &mut play.host.sound;
                        match window_wait_ms(&mut events, ms, |now| {
                            let due = now / 10;
                            if due > mixed {
                                sound.pass((due - mixed) as u32);
                                mixed = due;
                                feed_audio(audio.as_ref(), sound);
                            }
                        }) {
                            None => break 'outer,
                            Some(true) => {
                                play.host.sound.speech_stop();
                                eprintln!("show escape");
                                break 'show;
                            }
                            Some(false) => eprintln!("show wait end {} ms", ms),
                        }
                    }
                }
                eprintln!("window battle end");
                if let (Some((n, from, to)), Some(f)) = (held, play.host.fade_win.as_mut()) {
                    last_frame.clone_from(&shown_box);
                    f(n, from, to);
                }
                let now = ja1front::dostime::game_ticks(start.elapsed());
                play.locked_ticks(&mut e.world, false, now.saturating_sub(ticks_done), false);
                ticks_done = now;
            }
            // panel.md "Faces": a loud death plays the 10 frames of the death morph in the merc's strip face, 10 ms
            // apart (the game waits meanwhile), before the "is dead!" box.
            // MercDies? 0x3A4A4: each loud death plays its morph, then its own "is dead!" box (MsgMercText 0x3A65C),
            // before the next death; then any other box of the batch.
            loop {
                let dead = if play.host.morph.is_empty() { None } else { Some(play.host.morph.remove(0)) };
                let had_death = dead.is_some();
                if let Some((k, text)) = dead {
                    // MercDies? 0x3A519 .. 0x3A666: HideMouse? before the frames, MouseRedraw? after the "is dead!" text.
                    let hold = pointer_hold_start(screen.cursor.over_map);
                    held_gone(&mut play, &e.world, ja1front::pointer::held_gone(screen.cursor.over_map));
                    for i in 0..10 {
                        screen.morph_frame(iso, &e.world, k as usize, i).unwrap_or_else(|c| fail("death morph", c));
                        let idx = play_frame(iso, exe, e, screen, c0, r0, &mut play, false);
                        blit(&mut tex, &idx, &pal, SW, SH);
                        canvas.copy(&tex, None, None).expect("copy");
                        canvas.present();
                        eprintln!("window morph {} {}", k, i);
                        // The first frame's CopySlotToScreen 0x33204 ends in MouseRedraw? 0x67398: off the map the pointer and a
                        // held item are back for the frames (live, #31). The redraw after the "is dead!" text finds them on.
                        if i == 0 {
                            pointer_hold_end(hold, screen.cursor.over_map, e.world.update.screen_drawn != 0, ja1front::pointer::held_is_pointer(e.world.held[0]));
                            held_gone(&mut play, &e.world, false);
                        }
                        std::thread::sleep(Duration::from_millis(10));
                    }
                    let now = ja1front::dostime::game_ticks(start.elapsed());
                    play.locked_ticks(&mut e.world, false, now.saturating_sub(ticks_done), true);
                    ticks_done = now;
                    if let Some(t) = text {
                        play.host.msg_box = Some((t, 800));
                    }
                } else if play.host.msg_box.is_none() && !play.host.msg_more.is_empty() {
                    play.host.msg_box = Some((play.host.msg_more.remove(0), 25000));
                }
                if let Some((text, ms)) = play.host.msg_box.take() {
                    eprintln!("window msgbox {} {}", ms, text);
                    pointer_top(false);
                    pointer_left(false);
                    pointer_box(true);
                    // panel.md "Tactical messages": the box over the screen as it is now; no game time passes while
                    // it shows, and the key or click that closes it does nothing else.
                    // A held item is the pointer (panel.md "Clicks on the bottom strip"), so it goes with it (0x27BBE).
                    let held_was = std::mem::replace(&mut play.held_off, true);
                    let mut idx = play_frame(iso, exe, e, screen, c0, r0, &mut play, false);
                    play.held_off = held_was;
                    ja1front::msgbox::draw_tactical(iso, exe, e.tileset, &ja1front::msgbox::game_bytes(&text), &mut idx).unwrap_or_else(|c| fail("message box", c));
                    // After the Leave path's fade out (a battle's result at the end of the day) the box does not show.
                    let black = [0u8; 768];
                    blit(&mut tex, &idx, if play.left { &black } else { &pal }, SW, SH);
                    canvas.copy(&tex, None, None).expect("copy");
                    canvas.present();
                    let shown = Instant::now();
                    if box_wait(&mut events, ms as u64, &shown) {
                        break 'outer;
                    }
                    eprintln!("window msgbox end {}", ms);
                    pointer_box(false);
                    // ShowMessage? 0x27CCF: the cursor tile 0x229D2C goes to 0xC41 at the end of the box.
                    play.cursor.tile = 0xC41;
                    eprintln!("msgbox tile {}", play.cursor.tile);
                    if speed_log {
                        eprintln!("speed msgbox {} ms {}", shown.elapsed().as_millis(), ms);
                    }
                    // panel.md "Other users of Lock": the box Locked the game while it showed.
                    let now = ja1front::dostime::game_ticks(start.elapsed());
                    play.locked_ticks(&mut e.world, false, now.saturating_sub(ticks_done), true);
                    ticks_done = now;
                }
                if !had_death {
                    break;
                }
            }
            // cursor.md "Debug keys (level 4)": the four debug pages (Ctrl+P, Shift+left click), each over the screen
            // as it is now (pages 1 and 2: the map view, also in the overview); a key or a click shows the next one
            // and does nothing else. The game does not run meanwhile.
            if let Some(k) = play.host.debug_pages.take() {
                eprintln!("window pages {}", k);
                for page in ja1front::debugpage::pages_of(&e.world, k) {
                    let mode = e.world.opt_onscreen;
                    if page.map_view && mode == 0 {
                        e.world.opt_onscreen = 1;
                    }
                    let mut idx = play_frame(iso, exe, e, screen, c0, r0, &mut play, false);
                    e.world.opt_onscreen = mode;
                    ja1front::debugpage::draw_on(iso, exe, &page, &mut idx).unwrap_or_else(|c| fail("debug page", c));
                    blit(&mut tex, &idx, &pal, SW, SH);
                    canvas.copy(&tex, None, None).expect("copy");
                    canvas.present();
                    let mut any = WindowWaitKey::new();
                    'page: loop {
                        for ev in events.poll_iter() {
                            if let Event::Quit { .. } = ev {
                                break 'outer;
                            }
                            if any.event(&ev) {
                                break 'page;
                            }
                        }
                        std::thread::sleep(Duration::from_millis(10));
                    }
                }
                eprintln!("window pages end");
                ticks_done = ja1front::dostime::game_ticks(start.elapsed());
            }
            // panel.md "Say it again (key L)": the last quote's text in the text-only merc-face box until a key or a
            // click; no voice, no game time.
            if std::mem::take(&mut play.again) {
                let base = play_frame(iso, exe, e, screen, c0, r0, &mut play, false);
                let shown = ja1front::balloon::say_again(iso, exe, &e.world, "PANEL_P.PCC", &base).unwrap_or_else(|c| fail("say again", c));
                eprintln!("window again {:?}", e.world.last_quote);
                if let Some((img, _)) = shown {
                    prompt_mouse(&canvas, &events, ja1front::ctlpanel::subtitles(&e.world));
                    blit(&mut tex, &img, &pal, SW, SH);
                    canvas.copy(&tex, None, None).expect("copy");
                    canvas.present();
                    'again: loop {
                        for ev in events.poll_iter() {
                            match ev {
                                Event::KeyDown { .. } | Event::MouseButtonDown { .. } => break 'again,
                                Event::Quit { .. } => break 'outer,
                                _ => {}
                            }
                        }
                        std::thread::sleep(Duration::from_millis(10));
                    }
                    eprintln!("window again end");
                }
                ticks_done = ja1front::dostime::game_ticks(start.elapsed());
            }
            // cursor.md "Tile popups": the popup's screens, 20 ms each; at the end wait the right click waits for the
            // right button up, the key Z for a key or a click. The game does not run meanwhile.
            if let Some((t, key)) = play.popup.take() {
                if let Some(p) = ja1front::play::popup_of(&e.world, t) {
                    eprintln!("window popup {} {:?}", t, p);
                    let (v, wait) = popup_screens(iso, exe, e, screen, c0, r0, &mut play, &p);
                    'pop: for (n, f) in v.iter().enumerate() {
                        blit(&mut tex, f, &pal, SW, SH);
                        canvas.copy(&tex, None, None).expect("copy");
                        canvas.present();
                        if n == wait {
                            let mut any = WindowWaitKey::new();
                            loop {
                                let mut end = !key && !events.mouse_state().right();
                                for ev in events.poll_iter() {
                                    match ev {
                                        Event::Quit { .. } => break 'outer,
                                        Event::MouseButtonUp { mouse_btn: MouseButton::Right, .. } if !key => end = true,
                                        ev => end |= key && any.event(&ev),
                                    }
                                }
                                if end {
                                    break;
                                }
                                std::thread::sleep(Duration::from_millis(10));
                            }
                        } else if n + 1 < v.len() {
                            std::thread::sleep(Duration::from_millis(20));
                            for ev in events.poll_iter() {
                                if let Event::Quit { .. } = ev {
                                    break 'pop;
                                }
                            }
                        }
                    }
                    eprintln!("window popup end");
                    ticks_done = ja1front::dostime::game_ticks(start.elapsed());
                }
            }
            // turns.md "New turn" step 14 e: the view closes to black from both sides (op 164), then the new view
            // opens from the middle row (op 165); 1 ms per step.
            for k in std::mem::take(&mut play.host.wipes) {
                let (cols, rows, left, top) = ja1front::viewmode::world_window(&e.world);
                let r = ja1front::wipe::Rect { left, top, w: 16 * cols, h: 14 * rows };
                let v = if k == 0 {
                    ja1front::wipe::close(&last_frame, r)
                } else {
                    let idx = play_frame(iso, exe, e, screen, c0, r0, &mut play, false);
                    let mut view = Vec::with_capacity((r.w * r.h) as usize);
                    for y in r.top..r.top + r.h {
                        let a = (y * SW as i32 + r.left) as usize;
                        view.extend_from_slice(idx.get(a..a + r.w as usize).unwrap_or(&[]));
                    }
                    ja1front::wipe::open(&last_frame, &view, r)
                };
                eprintln!("window wipe {} {}", if k == 0 { "close" } else { "open" }, v.len());
                for f in &v {
                    blit(&mut tex, f, &pal, SW, SH);
                    canvas.copy(&tex, None, None).expect("copy");
                    canvas.present();
                    std::thread::sleep(Duration::from_millis(ja1front::wipe::STEP_MS));
                }
                if let Some(f) = v.last() {
                    last_frame.clone_from(f);
                }
            }
            // combat.md "Doors" (op 210 door_anim, PlayDoorFx 0x3B03C): the try film and then the outcome film play
            // on the screen at their place, frame i at i x frame time (FlcPlay 0x5BC58), the door film flag 0 (the
            // palette stays); Escape stops a film (FilmCallback 0x5BE18). The game waits meanwhile; the next frame
            // of the game draws over the film.
            let films = std::mem::take(&mut play.host.door_films);
            if !films.is_empty() {
                let begin = Instant::now();
                let mut shown_frame = last_frame.clone();
                'films: for (m, x, y, data) in &films {
                    let Some(mut f) = ja1front::flc::Flc::new(data) else { continue };
                    let ms = f.speed_ms.max(1) as u64;
                    let start = Instant::now();
                    let mut k = 0u64;
                    while f.next_frame(&mut shown_frame, *x, *y) {
                        if k > 0 {
                            let due = start + Duration::from_millis(ms * k);
                            while Instant::now() < due {
                                std::thread::sleep(Duration::from_millis(1));
                            }
                        }
                        k += 1;
                        blit(&mut tex, &shown_frame, &pal, SW, SH);
                        canvas.copy(&tex, None, None).expect("copy");
                        canvas.present();
                        let mut esc = false;
                        for ev in events.poll_iter() {
                            match ev {
                                Event::Quit { .. } => break 'films,
                                Event::KeyDown { keycode: Some(Keycode::Escape), .. } if k > 1 => esc = true,
                                _ => {}
                            }
                        }
                        if esc {
                            break;
                        }
                    }
                    eprintln!("window door film {} {} {} {}", m, x, y, k);
                }
                last_frame.clone_from(&shown_frame);
                ticks_done += ja1front::dostime::game_ticks(begin.elapsed());
            }
            // LoadPopups? 0x288DC (op 71): the drown / injection picture grows over the merc's slot, 50 ms for the
            // first frame and 60 ms for each next one (timer 0x92EF8), then Dissolve puts the saved block back. The
            // game waits meanwhile; the next frame of the game draws over the popup.
            let anim_popups = std::mem::take(&mut play.host.popups);
            if !anim_popups.is_empty() {
                let begin = Instant::now();
                let base = play_frame(iso, exe, e, screen, c0, r0, &mut play, false);
                'pops: for (slot, flag, selected) in &anim_popups {
                    let f = match ja1front::popup::frames(exe, *slot, *selected, *flag) {
                        Ok(f) => f,
                        Err(_) => continue,
                    };
                    eprintln!("window anim popup {} {} {}", slot, flag, f.len());
                    for (i, fr) in f.iter().enumerate() {
                        let mut img = base.clone();
                        ja1front::popup::blit(fr, &mut img);
                        blit(&mut tex, &img, &pal, SW, SH);
                        canvas.copy(&tex, None, None).expect("copy");
                        canvas.present();
                        std::thread::sleep(Duration::from_millis(if i == 0 { 50 } else { 60 }));
                        for ev in events.poll_iter() {
                            if let Event::Quit { .. } = ev {
                                break 'pops;
                            }
                        }
                    }
                }
                blit(&mut tex, &base, &pal, SW, SH);
                canvas.copy(&tex, None, None).expect("copy");
                canvas.present();
                last_frame.clone_from(&base);
                ticks_done += ja1front::dostime::game_ticks(begin.elapsed());
            }
            // panel.md "Speech balloon": the face slides in, the text shows until the wait ends, the face slides
            // out; no game time passes meanwhile. The view moves after the wait; a right click skips that move.
            // panel.md "Santino's balloon": Santino's speech (op 38) shows his face the same way (None below).
            if std::mem::take(&mut play.host.scene_closed) {
                break 'outer;
            }
            let turn_back = std::mem::take(&mut play.host.turn_back);
            let mut balloons: Vec<_> = std::mem::take(&mut play.host.reactions).into_iter().filter_map(|r| r.balloon.map(|b| Some((b, r.how, r.text, r.nat, r.undo, r.marks)))).collect();
            balloons.extend(std::mem::take(&mut play.host.npc_said).into_iter().filter(|&(cat, _)| cat == 9).map(|_| None));
            if !show_balloons(iso, exe, e, screen, c0, r0, &mut play, &mut canvas, &mut tex, &mut events, &pal,
                              audio.as_ref(), start, &mut ticks_done, balloons, true, false) {
                break 'outer;
            }
            if turn_back {
                // A move in the balloon counts only until MouseRestorePos2?, which stands behind SelectMerc? in the turn start.
                let p = ja1front::pointer::mouse_saved();
                eprintln!("balloon mouse back {} {}", p.0, p.1);
                play.mouse_px = p;
                play.mouse_warp = Some(p);
                ja1front::pointer::mouse_poll(p);
            }
            (c0, r0) = ja1front::enter::view_tile(&e.world);
            // panel.md "The guide's and Brenda's balloons": a quote during the tactical passes shows over the
            // tactical screen.
            for (op, a) in std::mem::take(&mut play.host.guide_said) {
                let idx = play_frame(iso, exe, e, screen, c0, r0, &mut play, false);
                if !guide_balloon(iso, exe, op, &a, ja1front::ctlpanel::sub_byte(e.world.update.opt_step_text), &idx, &pal, &mut canvas, &mut tex, &mut events) {
                    break 'outer;
                }
                ticks_done = ja1front::dostime::game_ticks(start.elapsed());
            }
            if let Some(restart) = play.aborted.take() {
                play.abort_day(&mut e.world, restart).unwrap_or_else(|m| die(m));
                eprintln!("abort {} day {} sector {}", restart as u8, e.world.day, e.world.cur_sector);
                if !play.left {
                    // Restart: the sector entry ran again; the window shows the new entry.
                    e.tileset = play.day.tileset;
                    pal = palette(iso, e.tileset as i32).unwrap_or_else(|c| fail("palette", c));
                    title = format!("ja1front: sector {} (cur_sector {})", e.world.cur_sector as i32 + 1, e.world.cur_sector);
                    (c0, r0) = ja1front::enter::view_tile(&e.world);
                    *screen = ja1front::screen::Screen::new(iso, exe, &mut e.world).unwrap_or_else(|c| fail("faces", c));
                    drawn_left = false;
                }
                // Abort: the day loop goes on at step 2 (the office) in the next round of the loop.
                ticks_done = ja1front::dostime::game_ticks(start.elapsed());
                continue;
            }
        }
        // SlotMouse 0x11B18: SelectMerc? 0x1148C, then WaitLeftUp 0x67C54, and only then the rest of the pass and its
        // frame. A press on a face shows nothing new while the button is down; the new view comes after the release
        // (#64 rig: original 0 of 40 holds, the view 29 ms after the release). The next pass waits for the release.
        if play.face_press {
            continue;
        }
        // The sector box flips with each blink step of the loop (panel.md "The front-end's tactical screen").
        let blink_step = last_step != Some(play.pause.steps);
        last_step = Some(play.pause.steps);
        let mut idx = play_frame(iso, exe, e, screen, c0, r0, &mut play, blink_step);
        // #20 B: the frame draws a held item at the mouse; the system pointer is off then (and over the map).
        pointer_visible(ja1front::pointer::tactical_pointer_shows(play.mouse.is_some(), e.world.held[0], play.host.mouse_hold, e.world.combat));
        if let Some(k) = held {
            screen.popup(iso, exe, &e.world, k, &mut idx).unwrap_or_else(|c| fail("popup", c));
        }
        if let Some(sw) = switch_ui.as_ref() {
            let pressed = match sw.step {
                SwStep::Press(i) => Some((i, e.world.soldiers[sw.k as usize * ja1front::soldier::REC + 0x16A + i] != 0)),
                _ => None,
            };
            screen.switch_part(iso, exe, &e.world, sw.k, switch_anim, pressed, &mut idx)
                .unwrap_or_else(|c| fail("switch", c));
        }
        for &k in stat_keys.iter().flatten() {
            screen.popup(iso, exe, &e.world, k, &mut idx).unwrap_or_else(|c| fail("popup", c));
        }
        // After the Leave path's fade out the palette stays black until the next screen fades in.
        blit(&mut tex, &idx, if play.left { &[0u8; 768] } else { &pal }, SW, SH);
        last_frame.clone_from(&idx);
        canvas.window_mut().set_title(&format!(
                "{} at ({}, {}) selected {}{}{}{}{}{}",
                title,
                c0,
                r0,
                e.world.selected,
                play.host.banner.map(|b| format!(" {}", b.text)).unwrap_or_default(),
                play.host.noise_msg.as_ref().map(|m| format!(" | {}", m)).unwrap_or_default(),
                format!(" | {}{}{}", ja1front::play::clock_text(&e.world),
                        if e.world.update.time_compress != 0 { " C" } else { "" },
                        play.host.status.as_ref().map(|m| format!(" | {}", m)).unwrap_or_default()),
                if play.host.end_of_day { " | end of day" } else { "" },
                if play.left { " sector left" } else { "" }
            )).ok();
        canvas.copy(&tex, None, None).expect("copy");
        canvas.present();
    }
    drop(tex);
    (to_menu, Gfx { sdl, canvas, events, music })
}

/// soldier_update.md "Traverse to a neighbour sector", the popup over `base` at (x0, y0): the pick (0 NORTH, 1 WEST,
/// 2 EAST, 3 SOUTH) and the mouse point at its end (travprompt::Prompt = the loop of TraversePrompt 0x1CBA8). The
/// original reads the button levels; a press and release inside one poll here counts as held in that poll. After the
/// loop it waits until both buttons are up (WaitLeftUp, WaitRightUp).
#[cfg(feature = "gui")]
#[allow(clippy::too_many_arguments)]
fn trav_choose(canvas: &mut Frame, tex: &mut sdl2::render::Texture,
               events: &mut sdl2::EventPump, pal: &[u8; 768], screen: &ja1front::screen::Screen, iso: &str, exe: &str,
               base: &[u8], x0: i32, y0: i32) -> (Option<usize>, (i32, i32)) {
    let mut prompt = ja1front::travprompt::Prompt::new(x0, y0);
    let mut first = true;
    let (mut pick, mut at);
    'outer: loop {
        // One key per poll, as GetKey; the keys after the one that ends the popup stay out of the game.
        let (mut keys, mut pressed, mut quit) = (Vec::new(), 0u8, false);
        for ev in events.poll_iter() {
            match ev {
                Event::KeyDown { keycode: Some(k), .. } => {
                    let c = k.into_i32();
                    if (0..128).contains(&c) {
                        keys.push(c as u8);
                    }
                }
                Event::MouseButtonDown { mouse_btn: sdl2::mouse::MouseButton::Left, .. } => pressed |= 1,
                Event::MouseButtonDown { mouse_btn: sdl2::mouse::MouseButton::Right, .. } => pressed |= 2,
                // SPEC GAP: closing the window while the popup is open closes it with no pick.
                Event::Quit { .. } => quit = true,
                _ => {}
            }
        }
        let ms = events.mouse_state();
        at = screen_pixel(canvas, ms.x(), ms.y());
        let buttons = button_bits(pressed, ms.left(), ms.right());
        keys.reverse();
        loop {
            let s = prompt.poll(at.0, at.1, buttons, keys.pop());
            pick = s.pick;
            if first || s.redraw {
                first = false;
                let mut f = base.to_vec();
                screen.trav_popup(iso, exe, x0, y0, s.lit, &mut f).unwrap_or_else(|c| fail("traverse popup", c));
                blit(tex, &f, pal, SW, SH);
                canvas.copy(tex, None, None).expect("copy");
                canvas.present();
            }
            if s.done {
                break 'outer;
            }
            if quit {
                pick = None;
                break 'outer;
            }
            if keys.is_empty() {
                break;
            }
        }
        std::thread::sleep(std::time::Duration::from_millis(10));
    }
    // WaitLeftUp, WaitRightUp: the release events stay in the queue for the main loop.
    loop {
        events.pump_events();
        let ms = events.mouse_state();
        if !ms.left() && !ms.right() {
            break;
        }
        std::thread::sleep(std::time::Duration::from_millis(10));
    }
    (pick, at)
}

/// The window as the inventory screen's input (section 8 "Inventory screen", op 221).
#[cfg(feature = "gui")]
struct WinInput<'a, 'b> {
    sdl_mouse: sdl2::mouse::MouseUtil,
    canvas: &'a mut Frame,
    tex: &'a mut sdl2::render::Texture<'b>,
    events: &'a mut sdl2::EventPump,
    pal: &'a [u8; 768],
    /// Buttons down (bit 0 left, bit 1 right) and the ones released in the same poll as their press (up at the
    /// next poll).
    buttons: u8,
    release: u8,
    mouse: (i32, i32),
    /// The last pointer state set (None = not yet in this screen).
    arrow: Option<bool>,
    /// The sound card's queue and the clock of the sound passed in this screen (start, ticks mixed).
    audio: Option<&'a sdl2::audio::AudioQueue<i16>>,
    clock: Option<(std::time::Instant, u128)>,
}

#[cfg(feature = "gui")]
impl ja1front::invscreen::InvInput for WinInput<'_, '_> {
    fn warp(&mut self, at: (i32, i32)) {
        warp_frame(&self.sdl_mouse, self.canvas, at.0, at.1);
        self.mouse = at;
    }
    fn pointer(&mut self, arrow: bool) {
        if self.arrow != Some(arrow) {
            self.arrow = Some(arrow);
            pointer_visible(arrow);
            eprintln!("inv pointer {}", if arrow { "arrow" } else { "item" });
        }
    }
    fn pointer_box(&mut self, on: bool) {
        pointer_box(on);
    }
    // the maintainer play test #14 D: the item voice plays at once and in real time while the screen waits (not 10 ms per
    // frame, sent at the close).
    fn sound(&mut self, s: &mut ja1front::sound::Sound) {
        let (t0, mixed) = self.clock.get_or_insert((std::time::Instant::now(), 0));
        let due = ja1front::dostime::game_ticks(t0.elapsed()) as u128 + 1;
        if due > *mixed {
            s.pass((due - *mixed) as u32);
            *mixed = due;
        }
        feed_audio(self.audio, s);
    }
    fn poll(&mut self, frame: &[u8]) -> Option<ja1front::invscreen::InvPoll> {
        use sdl2::keyboard::Scancode;
        use sdl2::mouse::MouseButton;
        blit(self.tex, frame, self.pal, SW, SH);
        self.canvas.copy(self.tex, None, None).expect("copy");
        self.canvas.present();
        std::thread::sleep(std::time::Duration::from_millis(10));
        self.buttons &= !self.release;
        self.release = 0;
        let mut pressed = 0u8;
        let mut keys = Vec::new();
        let bit = |b: MouseButton| match b {
            MouseButton::Left => 1u8,
            MouseButton::Right => 2,
            _ => 0,
        };
        for ev in self.events.poll_iter() {
            match ev {
                Event::Quit { .. } => return None,
                Event::MouseMotion { x, y, .. } => self.mouse = screen_pixel(self.canvas, x, y),
                Event::MouseButtonDown { mouse_btn, x, y, .. } => {
                    self.mouse = screen_pixel(self.canvas, x, y);
                    pressed |= bit(mouse_btn);
                    self.buttons |= bit(mouse_btn);
                }
                Event::MouseButtonUp { mouse_btn, .. } => {
                    if pressed & bit(mouse_btn) != 0 {
                        self.release |= bit(mouse_btn);
                    } else {
                        self.buttons &= !bit(mouse_btn);
                    }
                }
                Event::KeyDown { keycode: Some(k), .. } => {
                    let c = k.into_i32();
                    if k == Keycode::Escape {
                        keys.push(0x1B);
                    } else if (0x20..0x7F).contains(&c) {
                        keys.push((c as u8).to_ascii_uppercase());
                    }
                }
                _ => {}
            }
        }
        // A box inside the press (the merge question) takes the release event: the real button state ends the hold,
        // or the release wait of the press would eat the next click.
        let held = self.events.mouse_state();
        if !held.left() && pressed & 1 == 0 {
            self.buttons &= !1;
        }
        if !held.right() && pressed & 2 == 0 {
            self.buttons &= !2;
        }
        let ks = self.events.keyboard_state();
        let mut flags = 0u16;
        if ks.is_scancode_pressed(Scancode::LShift) || ks.is_scancode_pressed(Scancode::RShift) {
            flags |= 1;
        }
        if ks.is_scancode_pressed(Scancode::LCtrl) || ks.is_scancode_pressed(Scancode::RCtrl) {
            flags |= 4;
        }
        Some(ja1front::invscreen::InvPoll { x: self.mouse.0, y: self.mouse.1, buttons: self.buttons, flags, keys })
    }
}

/// The window as the end-of-day screen's output (section 8 "End-of-day screen"): raw pointers, valid during one pass.
#[cfg(feature = "gui")]
struct WinEod {
    canvas: *mut Frame,
    tex: *mut sdl2::render::Texture<'static>,
    events: *mut sdl2::EventPump,
    /// The time of the previous op 207 frame (70 frames per second).
    last: std::time::Instant,
    /// The window's audio queue (null: none).
    audio: *const sdl2::audio::AudioQueue<i16>,
    /// The game clock at the end of the previous op 255 wait (None: no wait yet).
    clock: Option<i64>,
    /// The DOS key queue that EscPressed? reads.
    keys: std::collections::VecDeque<u8>,
}

#[cfg(feature = "gui")]
impl ja1front::eod::EodSink for WinEod {
    fn clock_wait(&mut self, ms: i64) {
        self.clock = Some(match self.clock {
            Some(c) => ja1front::dostime::wait_from(c, ms),
            None => ja1front::dostime::now_ms(),
        });
    }
    fn show(&mut self, s: &[u8], pal: &[u8; 768], retrace: bool) {
        if retrace {
            let next = ja1front::dostime::next_retrace(self.last);
            if let Some(w) = next.checked_duration_since(std::time::Instant::now()) {
                std::thread::sleep(w);
            }
            self.last = std::time::Instant::now();
        }
        // SAFETY: see the binding in play_window.
        unsafe {
            blit(&mut *self.tex, s, pal, SW, SH);
            (*self.canvas).copy(&*self.tex, None, None).expect("copy");
            (*self.canvas).present();
        }
    }
    fn delay(&mut self, ms: u32) {
        std::thread::sleep(std::time::Duration::from_millis(ms as u64));
    }
    fn audio(&mut self, pcm: &[i16]) {
        // SAFETY: as above.
        if let Some(q) = unsafe { self.audio.as_ref() } {
            queue_part(q, pcm);
        }
    }
    fn poll(&mut self) -> bool {
        // SAFETY: as above. A key press or a mouse button press since the last poll; closing the window counts.
        let events = unsafe { &mut *self.events };
        let mut hit = false;
        for ev in events.poll_iter() {
            note_last_key(&ev);
            if matches!(ev, Event::KeyDown { .. } | Event::MouseButtonDown { .. } | Event::Quit { .. }) {
                hit = true;
            }
        }
        hit
    }

    fn poll_escape(&mut self) -> bool {
        // strategic.md "The day replay": EscPressed? 0x67300 reads one waiting key per call; only Escape stops the
        // replay, a click or another key does not. Only a window close also ends it here.
        // SAFETY: as above.
        let events = unsafe { &mut *self.events };
        let mut closed = false;
        for ev in events.poll_iter() {
            match ev {
                Event::KeyDown { keycode: Some(k), .. } => push_dos_key(&mut self.keys, k),
                Event::Quit { .. } => closed = true,
                _ => {}
            }
        }
        ja1front::invextra::esc_pressed(&mut self.keys) || closed
    }
}

/// A key press in the DOS key queue: its character code, or the two bytes 0 + a scan code of a key without one.
#[cfg(feature = "gui")]
fn push_dos_key(q: &mut std::collections::VecDeque<u8>, k: sdl2::keyboard::Keycode) {
    let c = k.into_i32();
    if k == sdl2::keyboard::Keycode::F24 {
        q.push_back(frame::CTRL_LETTER.load(std::sync::atomic::Ordering::Relaxed));
    } else if (1..128).contains(&c) {
        q.push_back(c as u8);
    } else {
        q.extend([0, 0x3B]);
    }
}

// Byte 0x95DC3: the last key that KeyOrClick 0x67E44 read in any box. Only WaitMs? clears it, at an Escape, so a click in
// a later WaitMs? reads an old Escape from a box too.
#[cfg(feature = "gui")]
thread_local! {
    static LAST_KEY: std::cell::Cell<u8> = const { std::cell::Cell::new(0) };
}

/// A key event in a KeyOrClick wait that has no key queue: byte 0x95DC3 takes the key.
#[cfg(feature = "gui")]
fn note_last_key(e: &sdl2::event::Event) {
    if let sdl2::event::Event::KeyDown { keycode: Some(k), .. } = e {
        let mut q = std::collections::VecDeque::new();
        push_dos_key(&mut q, *k);
        if let Some(c) = ja1front::invextra::read_key(&mut q) {
            LAST_KEY.with(|v| v.set(c));
        }
    }
}

/// WaitKeyOrClick 0x67F34 fed by window events: `event` answers true when the wait ended. An Up of a button that was
/// down before the wait counts: the original saw that button at its first poll.
#[cfg(feature = "gui")]
struct WindowWaitKey {
    w: ja1front::invextra::WaitKey,
    level: u8,
    keys: std::collections::VecDeque<u8>,
}

#[cfg(feature = "gui")]
impl WindowWaitKey {
    fn new() -> Self {
        WindowWaitKey { w: ja1front::invextra::WaitKey::new(ja1front::invextra::latch(), 0), level: 0, keys: Default::default() }
    }

    fn poll(&mut self, buttons: u8) -> bool {
        let key = if self.w.in_release() {
            None
        } else {
            ja1front::invextra::read_key(&mut self.keys)
        };
        if let Some(k) = key {
            LAST_KEY.with(|v| v.set(k));
        }
        self.w.step(buttons, key)
    }

    fn event(&mut self, ev: &sdl2::event::Event) -> bool {
        match ev {
            Event::KeyDown { keycode: Some(k), .. } => push_dos_key(&mut self.keys, *k),
            Event::MouseButtonDown { mouse_btn, .. } => self.level |= dos_button(*mouse_btn),
            Event::MouseButtonUp { mouse_btn, .. } => {
                let bit = dos_button(*mouse_btn);
                if bit != 0 && self.poll(self.level | bit) {
                    return true;
                }
                self.level &= !bit;
            }
            _ => return false,
        }
        self.poll(self.level)
    }
}

/// WaitMs? 0x67F48 in the window: `tick` runs at each pass. Some(true) = Escape, Some(false) = the time passed or
/// another key or a click ended this wait, None = the window closed.
/// The start flags stay as the last KeyOrClick left them (`invextra::latch`); the button level is the one at the start.
#[cfg(feature = "gui")]
fn window_wait_ms(events: &mut sdl2::EventPump, ms: u64, tick: impl FnMut(u128)) -> Option<bool> {
    let m = events.mouse_state();
    let level = m.left() as u8 | (m.right() as u8) << 1;
    let w = ja1front::invextra::WaitMs::from_latch(ms as i32, LAST_KEY.with(|v| v.get()));
    window_wait_run(events, w, level, tick)
}

/// window_wait_ms with button start flags `start`: a button down at the start counts only after it came up.
#[cfg(feature = "gui")]
fn window_wait_from(events: &mut sdl2::EventPump, ms: u64, start: u8, tick: impl FnMut(u128)) -> Option<bool> {
    let w = ja1front::invextra::WaitMs::new(ms as i32, start, LAST_KEY.with(|v| v.get()));
    window_wait_run(events, w, start, tick)
}

/// The poll loop of window_wait_from / window_wait_ms; `level` = the button level at the start.
#[cfg(feature = "gui")]
fn window_wait_run(events: &mut sdl2::EventPump, mut w: ja1front::invextra::WaitMs, mut level: u8, mut tick: impl FnMut(u128)) -> Option<bool> {
    let (t0, mut keys) = (Instant::now(), std::collections::VecDeque::new());
    loop {
        let mut ups = 0u8;
        for ev in events.poll_iter() {
            match ev {
                Event::Quit { .. } => return None,
                Event::KeyDown { keycode: Some(k), .. } => push_dos_key(&mut keys, k),
                Event::MouseButtonDown { mouse_btn, .. } => level |= dos_button(mouse_btn),
                Event::MouseButtonUp { mouse_btn, .. } => ups |= dos_button(mouse_btn),
                _ => {}
            }
        }
        let key = if w.in_release() { None } else { keys.pop_front().map(|k| if k == 0 { keys.pop_front().unwrap_or(0) } else { k }) };
        let ms_now = t0.elapsed().as_millis();
        let answer = w.step(ms_now.min(i32::MAX as u128) as i32, level, key);
        LAST_KEY.with(|v| v.set(w.last_key));
        if let Some(esc) = answer {
            return Some(esc);
        }
        // A press and a release in one pass still count: the level goes down after the step that saw the press.
        level &= !ups;
        tick(ms_now);
        std::thread::sleep(Duration::from_millis(5));
    }
}

/// The DOS button bit of a window button (bit 0 left, bit 1 right).
#[cfg(feature = "gui")]
fn dos_button(b: sdl2::mouse::MouseButton) -> u8 {
    match b {
        sdl2::mouse::MouseButton::Left => 1,
        sdl2::mouse::MouseButton::Right => 2,
        _ => 0,
    }
}

/// Window pixel -> tactical screen pixel (-1 outside).
#[cfg(feature = "gui")]
fn screen_pixel(canvas: &Frame, x: i32, y: i32) -> (i32, i32) {
    let (ww, wh) = canvas.window().size();
    frame::limited_pixel(ww, wh, x, y)
}

/// The end of a button hold (MouseRestorePos? 0x67444): the mouse goes back to the place of the press.
#[cfg(feature = "gui")]
fn hold_end(canvas: &Frame, hold: Option<ja1front::pointer::ButtonHold>) {
    if let Some((x, y)) = hold.and_then(|h| h.step(0)) {
        eprintln!("mouse back {} {}", x, y);
        warp_frame(&canvas.window().subsystem().sdl().mouse(), canvas, x, y);
    }
}

/// Moves the mouse to a screen pixel.
#[cfg(feature = "gui")]
/// YesNoPrompt? 0x28D88 in the window: `question::prompt_warp` with the place of the mouse now.
fn prompt_mouse(cv: &Frame, ev: &sdl2::EventPump, sub: bool) {
    let ms = ev.mouse_state();
    if let Some((x, y)) = ja1front::question::prompt_warp(sub, screen_pixel(cv, ms.x(), ms.y())) {
        warp_frame(&cv.window().subsystem().sdl().mouse(), cv, x, y);
        eprintln!("prompt mouse {} {}", x, y);
    }
}

/// `viewmode::margin_mouse` with the map window of the world.
fn margin_to(w: &ja1front::world::World, over_map: bool, x: i32, y: i32) -> Option<(i32, i32)> {
    let (cols, rows, l, t) = ja1front::viewmode::world_window(w);
    ja1front::viewmode::margin_mouse((l, t, l + cols * 16 - 1, t + rows * 14 - 1), over_map, x, y)
}

fn warp_frame(mouse: &sdl2::mouse::MouseUtil, canvas: &Frame, x: i32, y: i32) {
    let (ww, wh) = canvas.window().size();
    let (wx, wy) = frame::window_of(ww, wh, x, y);
    mouse.warp_mouse_in_window(canvas.window(), wx, wy);
}

/// The game state of the top-strip reaction rule (panel.md "Clicks on the top strip").
fn strip_click_env(w: &ja1front::world::World) -> ja1front::strips::ClickEnv {
    let cur = w.cur_sector as usize;
    let mercs = if cur < 60 { w.sector_state.get(cur * 9 + 2).copied().unwrap_or(0) } else { 0 };
    let sel = w.selected as usize;
    let sel_hp = if sel < 8 { w.soldiers[sel * ja1front::soldier::REC + 0x183] } else { 0 };
    ja1front::strips::ClickEnv { turn_shown: w.update.turn_shown, combat: w.combat, selected: w.selected, mercs, sel_hp }
}

/// ending.md "Game end" in the window: the end show plays with real waits, fades (one level per vertical retrace,
/// 1/70 s) and films; keys reach the player. Its sounds go to the screen sound, its music to the music.
#[cfg(feature = "gui")]
use std::time::{Duration, Instant};

#[cfg(feature = "gui")]
struct WinSab<'a, 'b> {
    canvas: &'a mut Frame,
    tex: &'a mut sdl2::render::Texture<'b>,
    events: &'a mut sdl2::EventPump,
    dir: std::path::PathBuf,
    iso: String,
    t0: Instant,
    /// The window was closed: every wait and film ends, the keys answer Escape.
    quit: bool,
    /// The Subtitles option (g_options + 5) that op 29 reads.
    subtitles: bool,
}

#[cfg(feature = "gui")]
impl WinSab<'_, '_> {
    fn show(&mut self, screen: &[u8], dac: &[u8; 1024]) {
        let pal = ja1front::ending::dac_rgb(dac);
        blit(self.tex, screen, &pal, SW, SH);
        self.canvas.copy(self.tex, None, None).expect("copy");
        self.canvas.present();
    }

    /// The next key as the player reads it (its character code; Escape 27), None when no key waits.
    fn poll_key(&mut self) -> Option<i8> {
        for ev in self.events.poll_iter() {
            if let Some(k) = sab_event(&ev, &mut self.quit) {
                return Some(k);
            }
        }
        self.quit.then_some(27)
    }
}

/// One event of a scene's key poll: the window close sets `quit`; a key gives its first BIOS byte.
#[cfg(feature = "gui")]
fn sab_event(ev: &Event, quit: &mut bool) -> Option<i8> {
    match ev {
        Event::Quit { .. } => *quit = true,
        // GetKey 0x560F4: an extended key (F1, an arrow, Alt+letter) gives byte 0: no Escape, but any key stops the
        // film of FilmCallback 0x5BE18 when its any-key byte is set. Shift alone is no BIOS key.
        Event::KeyDown { keycode: Some(k), keymod, .. } => {
            if let Some((first, _)) = dos_key_of(*k, *keymod) {
                return Some(first as i8);
            }
        }
        _ => {}
    }
    None
}

#[cfg(feature = "gui")]
impl ja1front::sab::SabIo for WinSab<'_, '_> {
    fn file(&mut self, name: &str) -> Option<Vec<u8>> {
        let rel = name.replace('\\', "/");
        std::fs::read(self.dir.join(&rel)).ok().or_else(|| ja1front::iso::Iso::open(&self.iso).ok().and_then(|mut i| i.read(&rel).ok()))
    }
    fn key(&mut self, _s: &ja1front::sab::Sab) -> Option<i8> {
        self.poll_key()
    }
    fn elapsed_ms(&mut self) -> i64 {
        if self.quit {
            return i64::MAX;
        }
        self.t0.elapsed().as_millis() as i64
    }
    fn wait_ms(&mut self, s: &ja1front::sab::Sab, ms: i64) {
        if self.quit {
            return;
        }
        // Site 0x66ADA of the credits: a key or a click ends the wait, and the answer is not used.
        let pal = ja1front::ending::dac_rgb(&s.dac);
        let (canvas, tex) = (&mut *self.canvas, &mut *self.tex);
        eprintln!("credits last wait");
        if window_wait_ms(self.events, ms as u64, |_| {
            blit(tex, &s.screen, &pal, SW, SH);
            canvas.copy(tex, None, None).expect("copy");
            canvas.present();
        }).is_none() {
            self.quit = true;
        }
        eprintln!("credits last wait end");
    }
    fn timer_start(&mut self) {
        self.t0 = Instant::now();
    }
    fn idle(&mut self, s: &ja1front::sab::Sab) {
        self.show(&s.screen, &s.dac);
        std::thread::sleep(Duration::from_millis(5));
    }
    fn retrace(&mut self, s: &ja1front::sab::Sab) {
        // One 1/70 s frame (the VGA retrace the credits wait for).
        self.show(&s.screen, &s.dac);
        std::thread::sleep(Duration::from_micros(1_000_000 / 70));
    }
    fn fade(&mut self, s: &mut ja1front::sab::Sab, pal: [u8; 1024], steps: i32, from: i32, to: i32) {
        // Op 24 / 25: levels l0 .. l1 toward l1, each colour (R, G, B) · l / n.
        let n = steps.max(1);
        let (l0, l1) = (n * from / 256, n * to / 256);
        let mut l = l0;
        loop {
            ja1front::sab::fade_dac(&mut s.dac, &pal, l, n);
            if !self.quit {
                let (scr, dac) = (s.screen.clone(), s.dac);
                self.show(&scr, &dac);
                std::thread::sleep(Duration::from_micros(14_286));
            }
            if l == l1 {
                break;
            }
            l += if l1 > l0 { 1 } else { -1 };
        }
    }
    fn film(&mut self, s: &mut ja1front::sab::Sab, name: &str, x: i32, y: i32, flag: i32, loops: i32) {
        // ending.md "Films": the frames at (x, y) on the screen, each after its frame time; a key stops the film
        // (with the film flag every key, else Escape; both set the abort flag). Loops 0 = until a key.
        let Some(data) = s.graphic(name).map(|d| d.to_vec()) else { return };
        s.play_film(self, &data, x, y, flag, loops);
        eprintln!("film end {}", name);
    }
    fn film_wait(&mut self, start: Instant, after_ms: u64) {
        let due = start + Duration::from_millis(after_ms);
        while Instant::now() < due {
            std::thread::sleep(Duration::from_millis(1));
        }
    }
    fn film_key(&mut self, _s: &ja1front::sab::Sab, _j: i32) -> Option<i8> {
        self.poll_key()
    }
    fn film_show(&mut self, s: &ja1front::sab::Sab) {
        self.show(&s.screen, &s.dac);
    }
    fn music_load(&mut self, name: &str) {
        ja1front::music::with(|m| m.load(name));
    }
    fn music(&mut self, theme: i32, stop_first: i32) {
        ja1front::music::with(|m| m.play(theme, stop_first != 0));
    }
    fn theme(&self) -> i32 {
        ja1front::music::with(|m| m.playing())
    }
    fn sequence_volume(&mut self, v: i32, ms: i32) {
        ja1front::music::with(|m| m.set_volume(v, ms));
    }
    fn music_end(&mut self) {
        ja1front::music::with(|m| m.stop_theme());
    }
    fn sfx_load(&mut self, name: &str, slot: i32, data: Option<&[u8]>) {
        ja1front::sound::screen(|s| s.set_effect(slot.max(0) as usize, name, data));
    }
    fn sfx_play(&mut self, slot: i32, rate: i32, vol: u8, loops: i32) {
        ja1front::sound::screen(|s| s.sfx(slot, rate, vol as i32, loops));
    }
    fn speech(&mut self, rate: i32, vol: i32, data: &[u8]) {
        ja1front::sound::screen(|s| s.speech_raw("end", rate, vol, data));
    }
    fn speaking(&self) -> bool {
        ja1front::sound::screen(|s| s.speaking())
    }
    fn speech_stop(&mut self) {
        ja1front::sound::screen(|s| s.speech_stop());
    }
    fn speech_end_quiet(&mut self) {
        ja1front::sound::screen(|s| s.speech_stop());
    }
    fn samples_end(&mut self) {
        ja1front::sound::screen(|s| s.effects_stop());
    }
    fn subtitles(&self) -> bool {
        self.subtitles
    }
    fn int_quote(&mut self, q: i32) -> Vec<u8> {
        ja1front::sab::int_quote(&self.iso, &self.dir.join("JA.EXE").to_string_lossy(), q)
    }
    fn draw_subtitle(&mut self, screen: &mut [u8], text: &[u8], fg: u8, shadow: u8) {
        ja1front::sab::draw_subtitle_files(&self.iso, &self.dir.join("JA.EXE").to_string_lossy(), screen, text, fg, shadow);
    }
    fn draw_caption(&mut self, screen: &mut [u8], text: &[u8], y: i32) {
        ja1front::sab::draw_caption(&self.iso, &self.dir.join("JA.EXE").to_string_lossy(), screen, text, y);
    }
}

/// ending.md "Game end" step 3 in the window for end code 1: 2000 ms, the win sequence with variables `v` (WINGAME.SAB,
/// INTRO05.FLC, the credits), then the rating text `text`. The original prints it on the DOS text screen as the
/// program ends; the window shows it white on black until a key or a click (SPEC GAP: front-end choice).
/// panel.md "The guide's and Brenda's balloons": the face slides in over `base`, waits while the voices speak on
/// the screen sound (or the wait without a voice), and slides out; Enter, Escape, Space or a click end the wait.
/// Returns false when the window closed.
#[cfg(feature = "gui")]
#[allow(clippy::too_many_arguments)]
fn guide_balloon(iso: &str, exe: &str, op: i32, a: &[i32], sub: bool, base: &[u8], pal: &[u8; 768],
                 canvas: &mut Frame, tex: &mut sdl2::render::Texture,
                 events: &mut sdl2::EventPump) -> bool {
    // Subtitles on: no face slides in; per part its voice starts, then the text-only merc-face box with the
    // part's text waits (stderr `balloon guide box Q` / `balloon brenda box Q`).
    if sub {
        let Some((who, c, _)) = ja1front::sound::npc_parts(op, a) else { return true };
        let parts = ja1front::balloon::npc_sub_screens(iso, exe, op, a, base, "PANEL_P.PCC")
            .unwrap_or_else(|c| fail("speech balloon", c));
        for (q, scr, wait) in parts {
            ja1front::sound::screen(|s| {
                if !s.has_files() {
                    *s = ja1front::sound::Sound::with_files(iso, exe);
                    s.load_main();
                }
                s.cat_quote(ja1front::sound::CATS[c], who, q);
            });
            eprintln!("balloon {} box {}", ja1front::balloon::npc_name(op), q);
            if !balloon_frames(&[scr], wait, 0, pal, canvas, tex, events) {
                return false;
            }
        }
        return true;
    }
    // The voices start with the balloon (PlayHost / MapHost voice_later); `speech C Q TEXT` per part.
    for l in ja1front::sound::npc_say(Some((iso, exe)), op, a) {
        eprintln!("{}", l);
    }
    eprintln!("balloon {}", ja1front::balloon::npc_name(op));
    let (v, _) = ja1front::balloon::npc_screens(iso, exe, op, a, base).unwrap_or_else(|c| fail("speech balloon", c));
    // SpeechBalloon? 0x29544: one SpeechWait? per part; the first part's voice plays alone, the second part's voice
    // starts after the first wait ended (its hold of 1000 ms passed with the face quiet, or a key ended it).
    let waits = ja1front::balloon::npc_waits(iso, exe, op, a).unwrap_or_else(|c| fail("speech balloon", c));
    let Some((who, c, _)) = ja1front::sound::npc_parts(op, a) else { return true };
    if ja1front::balloon::first_voice_here(waits.len()) {
        ja1front::sound::screen(|s| s.cat_quote(ja1front::sound::CATS[c], who, waits[0].0));
    }
    let parts: Vec<(u32, i64)> = waits.iter().map(|&(_, t, h)| (t, h)).collect();
    balloon_frames_parts(&v, &parts, 4, |k| {
        ja1front::sound::screen(|s| s.cat_quote(ja1front::sound::CATS[c], who, waits[k].0));
    }, pal, canvas, tex, events)
}

/// strategic.md "Not enough funds", Dismiss: the dismissed merc (team slot k, face x x0, profile p) speaks PST quote
/// 0 on the screen sound; its face slides in over `base` at (x0, 20) and out (Subtitles on: the merc-face box with
/// the text) as the guide's balloon. Returns false when the window closed.
#[cfg(feature = "gui")]
#[allow(clippy::too_many_arguments)]
fn pay_balloon(iso: &str, exe: &str, k: i32, x0: i32, p: i32, nick: &[u8], sub: bool, base: &[u8], pal: &[u8; 768],
               canvas: &mut Frame, tex: &mut sdl2::render::Texture,
               events: &mut sdl2::EventPump) -> bool {
    ja1front::sound::screen(|s| {
        if !s.has_files() {
            *s = ja1front::sound::Sound::with_files(iso, exe);
            s.load_main();
        }
        s.cat_quote("PST", p as u8, 0);
    });
    eprintln!("balloon pay {} {} {}", k, x0, p);
    let (v, wait) = ja1front::balloon::pay_screens(iso, exe, x0, p as u8, nick, sub, base).unwrap_or_else(|c| fail("speech balloon", c));
    balloon_frames(&v, wait, if sub { 0 } else { 4 }, pal, canvas, tex, events)
}

/// The boxes of a merc's morning quote (`MercSay`) as (quote, decoded text), and the nicknames of p and m.
#[cfg(feature = "gui")]
fn merc_say_texts(iso: &str, exe: &str, s: ja1front::play::MercSay, w: &ja1front::world::World) -> (Vec<(i32, Vec<u8>)>, Vec<u8>) {
    let nick = |p: i32| ja1front::balloon::profile_nick(&w.profiles, p);
    let boxes = ja1front::balloon::say_boxes(iso, exe, s.p as u8, s.q, &nick(s.m), s.first);
    (boxes, nick(s.p))
}

/// A merc's morning quote in the window: per box its voice (the name sample comes before quote q, PlayMercVoice 0x24D14
/// in 0x44D28; HatedCheck? loads none) and the balloon of 0x50908 over `base`. Returns false when the window closed.
#[cfg(feature = "gui")]
#[allow(clippy::too_many_arguments)]
fn merc_balloons(iso: &str, exe: &str, s: ja1front::play::MercSay, w: &ja1front::world::World, base: &[u8], pal: &[u8; 768],
                 canvas: &mut Frame, tex: &mut sdl2::render::Texture,
                 events: &mut sdl2::EventPump) -> bool {
    let (boxes, nick) = merc_say_texts(iso, exe, s, w);
    for (q, text) in boxes {
        ja1front::sound::screen(|c| {
            if !c.has_files() {
                *c = ja1front::sound::Sound::with_files(iso, exe);
                c.load_main();
            }
            if let Some((p, m)) = ja1front::play::name_voice(&s, q) {
                eprintln!("voice name {} {}", p, m);
                c.name_voice(p, m);
            }
            eprintln!("voice quote {} {}", s.p, q);
            c.cat_quote("PST", s.p as u8, q);
        });
        eprintln!("balloon merc {} {} {} quote {}", s.k, s.x0, s.p, q);
        let (v, wait) = ja1front::balloon::text_screens(iso, exe, s.x0, s.p as u8, &nick, s.sub, text, base)
            .unwrap_or_else(|c| fail("speech balloon", c));
        if !balloon_frames(&v, wait, if s.sub { 0 } else { 4 }, pal, canvas, tex, events) {
            return false;
        }
    }
    true
}

/// strategic.md "Firing a merc", host op 328 fire_quote (k, x, y, p): the fired AIM merc speaks AIM quote 9 on the
/// screen sound in the balloon of 0x50908 over `base`, as the dismiss balloon of the funds dialog. Returns false
/// when the window closed.
#[cfg(feature = "gui")]
#[allow(clippy::too_many_arguments)]
/// strategic.md "The day-end screen": the window as the host of the wait loop (dayend.rs).
struct DayEndWin<'a> {
    cv: &'a mut Frame,
    tx: &'a mut sdl2::render::Texture<'static>,
    ev: &'a mut sdl2::EventPump,
    map: &'a mut ja1front::mapui::MapUi,
    w: &'a mut ja1front::world::World,
    s: Vec<u8>,
    pal: [u8; 768],
    iso: &'a str,
    exe: &'a str,
    b: u8,
    at: (i32, i32),
    buttons: u8,
    keys: std::collections::VecDeque<u8>,
    tip: ja1front::assign::Tooltip,
    /// The picture with the "Fire NICK?" box, while the box is on the screen.
    question: Option<Vec<u8>>,
}

impl DayEndWin<'_> {
    fn show(&mut self) {
        blit(self.tx, &self.s, &self.pal, SW, SH);
        self.cv.copy(self.tx, None, None).expect("copy");
        self.cv.present();
    }

    /// The events since the last read. Closing the window ends the program (#36 C: the original is a DOS program in
    /// DOSBox-X, whose window close kills the game at once). A button that went down and up between two reads counts
    /// as down for one poll (the window test clicks fast).
    fn read_input(&mut self) {
        use sdl2::{keyboard::Mod, mouse::MouseButton};
        let mut pressed = 0u8;
        for evt in self.ev.poll_iter() {
            match evt {
                Event::Quit { .. } => exit(0),
                Event::MouseButtonDown { mouse_btn: MouseButton::Left, .. } => pressed |= 1,
                Event::MouseButtonDown { mouse_btn: MouseButton::Right, .. } => pressed |= 2,
                Event::KeyDown { keycode: Some(Keycode::X), keymod, .. } if keymod.intersects(Mod::LALTMOD | Mod::RALTMOD) => {
                    self.keys.extend([0, 0x2D]);
                }
                Event::KeyDown { keycode: Some(k), .. } if (1..128).contains(&k.into_i32()) => {
                    self.keys.push_back(k.into_i32() as u8);
                }
                _ => {}
            }
        }
        let ms = self.ev.mouse_state();
        self.at = screen_pixel(self.cv, ms.x(), ms.y());
        self.buttons = button_bits(pressed, ms.left(), ms.right());
    }

    /// A box over the screen until a key or a mouse button goes down (and the button is up again), or `ms` pass
    /// (panel.md "Message box" step 3; "Say it again" waits without a time limit).
    /// WaitKeyOrClick 0x67F34 (site 0x28F29 of the text-only YesNoPrompt): a key, or the release of the left or right button.
    fn wait_key(&mut self) {
        let mut any = WindowWaitKey::new();
        loop {
            std::thread::sleep(Duration::from_millis(10));
            for evt in self.ev.poll_iter() {
                if matches!(evt, Event::Quit { .. }) || any.event(&evt) {
                    return;
                }
            }
        }
    }
}

impl ja1front::dayend::Host for DayEndWin<'_> {
    fn poll(&mut self) -> bool {
        std::thread::sleep(Duration::from_millis(10));
        self.read_input();
        true
    }
    fn mouse(&self) -> (i32, i32, u8) {
        (self.at.0, self.at.1, self.buttons)
    }
    fn key(&mut self) -> Option<u8> {
        self.keys.pop_front()
    }
    fn key2(&mut self) -> u8 {
        self.keys.pop_front().unwrap_or(0)
    }
    fn exit_prompt(&mut self) -> bool {
        let (text, pic) = ja1front::question::EXIT;
        let imgs = ja1front::question::screens(self.iso, self.exe, text, Some(pic), &self.s)
            .unwrap_or_else(|c| fail("question", c));
        eprintln!("day-end ask exit");
        let pal = self.pal;
        let yes = question_box(self.cv, self.tx, self.ev, &imgs, &pal);
        if !yes {
            self.show();
        }
        yes
    }
    fn say_again(&mut self) {
        match ja1front::balloon::say_again(self.iso, self.exe, self.w, "PANEL_P.PCC", &self.s) {
            Ok(Some((img, _))) => {
                eprintln!("day-end again");
                prompt_mouse(self.cv, self.ev, ja1front::ctlpanel::subtitles(self.w));
                blit(self.tx, &img, &self.pal, SW, SH);
                self.cv.copy(self.tx, None, None).expect("copy");
                self.cv.present();
                self.wait_key();
                eprintln!("day-end box end");
                self.show();
            }
            Ok(None) => eprintln!("day-end again none"),
            Err(c) => fail("say again", c),
        }
    }
    fn toggle_text(&mut self) {
        let v = ja1front::ctlpanel::get(self.w, 5) ^ 1;
        ja1front::ctlpanel::set(self.w, 5, v);
        let text = if v != 0 { "Text  on" } else { "Text  off" };
        eprintln!("day-end msg {}", text);
        let img = self.map.msg_img(&self.s, text.as_bytes()).unwrap_or_else(|c| fail("message box", c));
        blit(self.tx, &img, &self.pal, SW, SH);
        self.cv.copy(self.tx, None, None).expect("copy");
        self.cv.present();
        // ShowMessage? 0x27BF4: the start flags come from the button level; 25 s, a key or a click ends the wait.
        window_wait_from(self.ev, 25_000, self.buttons, |_| {});
        eprintln!("day-end box end");
        self.show();
    }
    fn button_sound(&mut self, which: u8) {
        ja1front::sound::screen_button(self.iso, self.exe, which as i32);
    }
    fn ok_face(&mut self, b: u8, pressed: bool) {
        ja1front::dayloop::dayend_ok_face(&mut self.s, self.iso, self.exe, b, pressed);
        self.show();
    }
    fn wait_left_up(&mut self) {
        let hold = ja1front::pointer::ButtonHold::press(self.at);
        loop {
            std::thread::sleep(Duration::from_millis(10));
            self.read_input();
            if self.buttons & 1 == 0 {
                break;
            }
        }
        hold_end(self.cv, Some(hold));
    }
    fn present(&self, k: usize) -> bool {
        self.w.soldiers[k * 0x200 + 0x182] != 0
    }
    fn tip(&mut self, k: Option<usize>) {
        // ShowTooltip with the shown number and HideTooltip with none shown change nothing.
        if k.map_or(0, |k| k as u8 + 1) != self.tip.n {
            eprintln!("day-end tip {}", k.map_or(0, |k| k + 1));
            self.map.payroll_tip(&mut self.s, &mut self.tip, k).unwrap_or_else(|c| fail("payroll", c));
            self.show();
        }
    }
    fn ask_fire(&mut self, k: usize) -> bool {
        let imgs = self.map.payroll_fire_imgs(self.w, k, &self.s).unwrap_or_else(|c| fail("payroll", c));
        eprintln!("day-end ask fire {}", k);
        let pal = self.pal;
        prompt_mouse(self.cv, self.ev, ja1front::ctlpanel::subtitles(self.w));
        let yes = question_loop(self.cv, self.tx, self.ev, &imgs, &pal, None);
        if !yes {
            eprintln!("day-end fire {} no", k);
        }
        self.read_input();
        self.question = imgs.into_iter().next();
        yes
    }
    fn subtitles(&self) -> bool {
        ja1front::ctlpanel::subtitles(self.w)
    }
    fn close_question(&mut self) {
        self.question = None;
    }
    fn fire(&mut self, k: usize) {
        let nick: Vec<u8> = self.w.soldiers[k * 0x200 + 0x16E..k * 0x200 + 0x177].iter()
            .copied().take_while(|&c| c != 0).collect();
        let sub = ja1front::ctlpanel::subtitles(self.w);
        let said = self.map.payroll_fire(self.w, k);
        eprintln!("day-end fire {}", k);
        eprintln!("day-end quote over the question {}", self.question.is_some() as u8);
        if let Some(a) = said {
            // 0x65E1E: with Subtitles on, the quote shows over the question box.
            let (s, pal) = (self.question.take().unwrap_or_else(|| self.s.clone()), self.pal);
            fire_balloon(self.iso, self.exe, &a, &nick, sub, &s, &pal, self.cv, self.tx, self.ev);
        }
        self.read_input();
    }
    fn redraw(&mut self) {
        self.s = self.map.payroll_again(self.w).unwrap_or_else(|c| fail("payroll", c));
        ja1front::dayloop::dayend_ok_face(&mut self.s, self.iso, self.exe, self.b, false);
        self.show();
    }
}

fn fire_balloon(iso: &str, exe: &str, a: &[i32], nick: &[u8], sub: bool, base: &[u8], pal: &[u8; 768],
                canvas: &mut Frame, tex: &mut sdl2::render::Texture,
                events: &mut sdl2::EventPump) -> bool {
    let arg = |i: usize| a.get(i).copied().unwrap_or(0);
    let (k, x0, y0, p) = (arg(0), arg(1), arg(2), arg(3));
    ja1front::sound::screen(|s| {
        if !s.has_files() {
            *s = ja1front::sound::Sound::with_files(iso, exe);
            s.load_main();
        }
        s.cat_quote("AIM", p as u8, 9);
    });
    eprintln!("balloon fire {} {} {}{}", k, x0, p, if sub { " box" } else { "" });
    let (v, wait) = ja1front::balloon::fire_screens(iso, exe, x0, y0, p as u8, nick, sub, base).unwrap_or_else(|c| fail("speech balloon", c));
    balloon_frames(&v, wait, if sub { 0 } else { 4 }, pal, canvas, tex, events)
}

/// Shows the frames `v` of a balloon (30 ms each); frame `hold` waits while the voice speaks (or `wait` ms without a
/// voice) until Enter, Escape, Space or a click. Returns false when the window closed.
#[cfg(feature = "gui")]
fn balloon_frames(v: &[Vec<u8>], wait: u32, hold: usize, pal: &[u8; 768],
                  canvas: &mut Frame, tex: &mut sdl2::render::Texture,
                  events: &mut sdl2::EventPump) -> bool {
    balloon_frames_parts(v, &[(wait, 0)], hold, |_| {}, pal, canvas, tex, events)
}

/// A click in the wait of a balloon: the left button is a left click, every other button a right click
/// (SpeechWait? 0x28682 reads the left button and "any other button").
#[cfg(feature = "gui")]
fn balloon_click(input: &mut ja1front::faceanim::WaitInput, b: sdl2::mouse::MouseButton) {
    match b {
        sdl2::mouse::MouseButton::Left => input.left = true,
        _ => input.right = true,
    }
}

/// As `balloon_frames` with one wait per part at frame `hold`: `parts` = (text time ms, hold ms after the talk), as
/// SpeechBalloon? 0x29544 runs SpeechWait? once per part. `next(k)` starts part k (k >= 1) after the wait of part
/// k - 1 ended; each wait starts the face again on its clean picture (0x28232 puts the saved face back).
#[cfg(feature = "gui")]
#[allow(clippy::too_many_arguments)]
fn balloon_frames_parts(v: &[Vec<u8>], parts: &[(u32, i64)], hold: usize, mut next: impl FnMut(usize), pal: &[u8; 768],
                        canvas: &mut Frame, tex: &mut sdl2::render::Texture,
                        events: &mut sdl2::EventPump) -> bool {
    // A single held frame is the text-only box of YesNoPrompt? (Subtitles on).
    if hold == 0 && v.len() == 1 {
        prompt_mouse(canvas, events, true);
    }
    for (n, f) in v.iter().enumerate() {
        blit(tex, f, pal, SW, SH);
        canvas.copy(tex, None, None).expect("copy");
        canvas.present();
        // SpeechBalloon? 0x296DA: WaitLeftUp between the face's way in and SpeechWait?.
        let fw = ja1front::balloon::frame_wait(n, v.len(), hold);
        if fw.release {
            let ms = speech_buttons_up(canvas, events, true, false, |_| {});
            if ms > 0 {
                eprintln!("balloon release middle {}", ms);
            }
        }
        for (k, &(wait, hold_ms)) in parts.iter().enumerate().take(if n == hold { parts.len() } else { 1 }) {
            if ja1front::balloon::part_restarts(k) {
                next(k);
                eprintln!("balloon part {}", k);
                blit(tex, f, pal, SW, SH);
                canvas.copy(tex, None, None).expect("copy");
                canvas.present();
            }
            let shown = Instant::now();
            let ms = if n == hold { wait as u64 } else { ja1front::balloon::FRAME_MS as u64 };
            // With a voice the wait ends when the voice ends; the voice's own length bounds it (a window without an
            // audio device does not play the screen sound).
            let voice = if n == hold { ja1front::sound::screen(|s| s.speech_ms_left()) } else { 0 };
            let voiced = has_voice(voice);
            // panel.md "Face animation": the face blinks and talks during the wait.
            let lip = if n == hold { ja1front::sound::screen(|s| s.speech_lip()) } else { None };
            // A single frame is the text-only merc-face box (Subtitles on): its face talks too.
            let mut face = if fw.face {
                ja1front::balloon::last_face().map(|sp| ja1front::faceanim::WaitFace::new(sp, lip.as_ref().map(|l| l.0.as_slice()), shown.elapsed().as_nanos() as u32 ^ 0x5EED))
            } else {
                None
            };
            // A single held frame is the text-only box: Subtitles are on, only the player ends that wait.
            let rules = ja1front::faceanim::WaitRules { text_ms: ms as i64, voiced, hold: hold_ms, subtitles: hold == 0 && v.len() == 1, has_text: false };
            let mut wait_rules = face.take().map(|wf| ja1front::faceanim::SpeechWait::new(wf, rules));
            'wait: while wait_rules.is_some() || if voiced {
                ja1front::sound::screen(|s| s.speaking()) && shown.elapsed() < Duration::from_millis(voice + 200)
            } else {
                shown.elapsed() < Duration::from_millis(ms + hold_ms as u64)
            } {
                let mut input = ja1front::faceanim::WaitInput::default();
                for ev in events.poll_iter() {
                    match ev {
                        Event::KeyDown { keycode: Some(Keycode::Return | Keycode::Escape | Keycode::Space), .. } if n == hold => input.key = true,
                        Event::MouseButtonDown { mouse_btn, .. } if n == hold => balloon_click(&mut input, mouse_btn),
                        Event::Quit { .. } => return false,
                        _ => {}
                    }
                }
                if let Some(sw) = wait_rules.as_mut() {
                    let ms_ = events.mouse_state();
                    (input.x, input.y) = screen_pixel(canvas, ms_.x(), ms_.y());
                    input.left |= ms_.left();
                    input.right |= ms_.right();
                    input.now = shown.elapsed().as_millis() as i64;
                    if let Some((_, us)) = ja1front::sound::screen(|s| s.speech_lip()) {
                        input.played_us = us;
                    }
                    // The voice's own length bounds it (a window without an audio device does not play the screen sound).
                    input.playing = voice_plays(ja1front::sound::screen(|s| s.speaking()), shown.elapsed(), voice);
                    let step = sw.pass(&input);
                    if step.redraw {
                        let mut fr = f.clone();
                        sw.face.spot.paint(&mut fr, sw.face.anim.e, sw.face.anim.m);
                        blit(tex, &fr, pal, SW, SH);
                        canvas.copy(tex, None, None).expect("copy");
                        canvas.present();
                    }
                    if step.end.is_some() {
                        break 'wait;
                    }
                } else if input.pressed() {
                    break 'wait;
                }
                std::thread::sleep(Duration::from_millis(5));
            }
            let face = wait_rules.map(|sw| sw.face);
            face_note(&face);
            // The end of SpeechWait? 0x28682: WaitLeftUp + WaitRightUp, and the picture stays until the release.
            if fw.release {
                let ms = speech_buttons_up(canvas, events, true, true, |_| {});
                if ms > 0 {
                    eprintln!("balloon release end {}", ms);
                }
            }
        }
    }
    eprintln!("balloon end");
    true
}

/// The face redraws of the last animated balloon wait (stderr `face anim N`).
#[cfg(feature = "gui")]
fn face_note(face: &Option<ja1front::faceanim::WaitFace>) {
    if let Some(wf) = face {
        eprintln!("face anim {}", wf.redraws);
    }
}

/// ending.md "The title part (program start)" in the window: the films sirtech, title1 (with its cue sounds), title2
/// and the credits with real frame times; Escape stops one film or the credits. Then every sound stops.
#[cfg(feature = "gui")]
fn title_window(iso: &str, exe: &str, subtitles: bool, canvas: &mut Frame,
                tex: &mut sdl2::render::Texture, events: &mut sdl2::EventPump) {
    let dir = std::path::Path::new(exe).parent().map(|p| p.to_path_buf()).unwrap_or_default();
    let mut io = WinSab { canvas, tex, events, dir, iso: iso.to_string(), t0: Instant::now(), quit: false, subtitles };
    ja1front::sound::screen_files(iso, exe);
    let mut s = ja1front::sab::Sab::new();
    let stop = ja1front::ending::title(&mut io, &mut s, &mut ja1front::ending::credits_draw(iso, exe));
    eprintln!("title stop {}", stop as i32);
    ja1front::music::with(|m| m.stop_theme());
    ja1front::sound::screen(|s| s.stop_all());
    if io.quit {
        exit(0);
    }
}

#[cfg(feature = "gui")]
fn win_window(iso: &str, exe: &str, subtitles: bool, v: [i16; 4], text: &str, canvas: &mut Frame,
              tex: &mut sdl2::render::Texture, events: &mut sdl2::EventPump) {
    let dir = std::path::Path::new(exe).parent().map(|p| p.to_path_buf()).unwrap_or_default();
    let mut io = WinSab { canvas, tex, events, dir, iso: iso.to_string(), t0: Instant::now(), quit: false, subtitles };
    ja1front::sound::screen_files(iso, exe);
    use ja1front::sab::SabIo;
    let mut s = ja1front::sab::Sab::new();
    io.timer_start();
    while io.elapsed_ms() < 2000 {
        io.poll_key();
        std::thread::sleep(Duration::from_millis(10));
    }
    let (r, stop) = ja1front::ending::win(&mut io, &mut s, v, &mut ja1front::ending::credits_draw(iso, exe));
    eprintln!("end show WINGAME.SAB {}", r);
    eprintln!("credits stop {}", stop as i32);
    ja1front::music::with(|m| m.stop_theme());
    ja1front::sound::screen(|s| s.stop_all());
    // The DOS text screen: the text at the top left, white on black (font 2).
    let mut buf = vec![0u8; 64000];
    let mut dac = [0u8; 1024];
    dac[4 * 0xFF..4 * 0xFF + 3].copy_from_slice(&[63, 63, 63]);
    if let (Ok(c), Ok(d)) = (ja1front::common(iso), ja1front::panel::panel_data(iso, exe)) {
        let mut t0 = c.remap[0];
        let mut p = ja1front::panel::Painter { d: &d, buf: &mut buf, table0: &mut t0, table29: ja1front::panel::table29(&c) };
        p.text(text.as_bytes(), 4, 4, false, 0xFF, 0, 2);
    }
    eprintln!("rating {}", text);
    while !io.quit {
        io.show(&buf, &dac);
        let mut hit = false;
        for ev in io.events.poll_iter() {
            match ev {
                Event::KeyDown { .. } | Event::MouseButtonDown { .. } => hit = true,
                Event::Quit { .. } => io.quit = true,
                _ => {}
            }
        }
        if hit {
            break;
        }
        std::thread::sleep(Duration::from_millis(20));
    }
}

/// ending.md "The new-game intro" in the window: INTRO\SAT.DAT island.SAB with real waits, films and sounds; then
/// every sound and the music stop (SPEC GAP: front-end choice; Set Difficulty plays none).
#[cfg(feature = "gui")]
fn intro_window(iso: &str, exe: &str, subtitles: bool, canvas: &mut Frame,
                tex: &mut sdl2::render::Texture, events: &mut sdl2::EventPump) {
    let dir = std::path::Path::new(exe).parent().map(|p| p.to_path_buf()).unwrap_or_default();
    let mut io = WinSab { canvas, tex, events, dir: dir.clone(), iso: iso.to_string(), t0: Instant::now(), quit: false, subtitles };
    ja1front::sound::screen_files(iso, exe);
    let t0 = Instant::now();
    match ja1front::ending::play_intro(&mut io, &dir) {
        Some((_, r)) => eprintln!("intro island.SAB {}", r),
        None => eprintln!("intro none"),
    }
    // JA1_SPEED_LOG=1 (world_load.md "Game speed"): the intro's time from START NEW GAME to its end
    if std::env::var("JA1_SPEED_LOG").is_ok_and(|v| v == "1") {
        eprintln!("speed intro {} ms", t0.elapsed().as_millis());
    }
    use ja1front::sab::SabIo;
    io.samples_end();
    io.speech_stop();
    io.music_end();
    if io.quit {
        exit(0);
    }
}

/// ending.md "Game end" steps 3, 4 in the window for end code `code` (2, 3, 4 and above): the end show, then the
/// game-over screen (fade in over 24 levels, a key or a click, fade out over 56 levels).
#[cfg(feature = "gui")]
fn end_window(iso: &str, exe: &str, subtitles: bool, code: i32, canvas: &mut Frame,
              tex: &mut sdl2::render::Texture, events: &mut sdl2::EventPump) {
    let dir = std::path::Path::new(exe).parent().map(|p| p.to_path_buf()).unwrap_or_default();
    let mut io = WinSab { canvas, tex, events, dir, iso: iso.to_string(), t0: Instant::now(), quit: false, subtitles };
    ja1front::sound::screen_files(iso, exe);
    if let Some((_, r)) = ja1front::ending::play_show(&mut io, code) {
        eprintln!("end show {} {}", ja1front::ending::show_member(code).unwrap_or("-"), r);
    }
    // Step 1 of the game-over screen: the music and every sound stop.
    ja1front::music::with(|m| m.stop_theme());
    ja1front::sound::screen(|s| s.stop_all());
    let mut buf = vec![0u8; 64000];
    let Ok(pal) = ja1front::ending::game_over_picture(iso, exe, &mut buf) else { return };
    let mut slot = [0u8; 1024];
    for i in 0..256 {
        slot[4 * i..4 * i + 3].copy_from_slice(&pal[3 * i..3 * i + 3]);
    }
    let mut s = ja1front::sab::Sab::new();
    s.screen = buf;
    use ja1front::sab::SabIo;
    io.fade(&mut s, slot, 24, 0, 256);
    eprintln!("game over screen");
    while !io.quit {
        let mut hit = false;
        for ev in io.events.poll_iter() {
            match ev {
                Event::KeyDown { .. } | Event::MouseButtonDown { .. } => hit = true,
                Event::Quit { .. } => io.quit = true,
                _ => {}
            }
        }
        if hit {
            break;
        }
        std::thread::sleep(Duration::from_millis(10));
    }
    io.fade(&mut s, slot, 56, 256, 0);
}

/// The DOS key bytes of an SDL key (tactkeys::dos_key).
fn dos_key_of(k: Keycode, m: sdl2::keyboard::Mod) -> Option<(u8, u8)> {
    use sdl2::keyboard::Mod;
    if k == Keycode::F24 {
        return Some((frame::CTRL_LETTER.load(std::sync::atomic::Ordering::Relaxed), 0));
    }
    let alt = m.intersects(Mod::LALTMOD | Mod::RALTMOD);
    let ctrl = m.intersects(Mod::LCTRLMOD | Mod::RCTRLMOD);
    let shift = m.intersects(Mod::LSHIFTMOD | Mod::RSHIFTMOD);
    const F: [Keycode; 10] = [Keycode::F1, Keycode::F2, Keycode::F3, Keycode::F4, Keycode::F5, Keycode::F6, Keycode::F7,
                              Keycode::F8, Keycode::F9, Keycode::F10];
    let f = F.iter().position(|&x| x == k).map_or(0, |i| i as u8 + 1);
    let scan = match k {
        Keycode::F11 => 0x85,
        Keycode::F12 => 0x86,
        Keycode::Up => 0x48,
        Keycode::Left => 0x4B,
        Keycode::Right => 0x4D,
        Keycode::Down => 0x50,
        Keycode::PageUp => 0x49,
        Keycode::PageDown => 0x51,
        Keycode::Insert => 0x52,
        Keycode::Home => 0x47,
        Keycode::End => 0x4F,
        Keycode::Delete => 0x53,
        _ => 0,
    };
    let key = match k {
        Keycode::KpEnter => 13,
        Keycode::KpPlus => b'+' as i32,
        Keycode::KpMinus => b'-' as i32,
        Keycode::KpPeriod => b'.' as i32,
        _ => k.into_i32(),
    };
    ja1front::tactkeys::dos_key(key, f, scan, alt, ctrl, shift)
}

/// The state that the key arms of TacticalKeyHandler read.
fn window_gates(w: &ja1front::world::World, paused: bool, quick_file: bool) -> ja1front::tactkeys::Gates {
    ja1front::tactkeys::gates_of(w, paused, quick_file)
}

/// A held strip press, the stat keys or the switch box stop the game (no turns, no animation, no other input)
/// until the button is up.
/// panel.md "Speech balloon" step 7: the weapon text shows over the face in frame 4 of the balloon only.
fn weapon_text_frame(n: usize) -> bool {
    n == 4
}
/// The balloon wait goes on while the face-and-voice rules run, or until the text time `ms` passed.
fn balloon_waits(rules: bool, elapsed: Duration, ms: u64) -> bool {
    rules || elapsed < Duration::from_millis(ms)
}
/// Byte 0x92F65 (the right button in the wait) takes the balloon's view move back only for a message whose move
/// the right button skips.
fn right_undoes_move(right: bool, skips: bool) -> bool {
    right && skips
}
/// A balloon frame has a voice when the screen sound has speech time left.
fn has_voice(voice_ms: u64) -> bool {
    voice_ms > 0
}
/// The voice plays while the screen sound speaks, at most the voice's own length + 200 ms after the frame shows.
fn voice_plays(speaking: bool, elapsed: Duration, voice_ms: u64) -> bool {
    speaking && elapsed < Duration::from_millis(voice_ms + 200)
}
/// The day-end screen's button value: presses since the last read and the held buttons (bit 0 left, bit 1 right).
fn button_bits(pressed: u8, left: bool, right: bool) -> u8 {
    pressed | left as u8 | (right as u8) << 1
}
/// The speech button-up wait ends when no button that it waits for is still down.
fn wanted_buttons_up(left: bool, right: bool, left_down: bool, right_down: bool) -> bool {
    !(left && left_down) && !(right && right_down)
}
/// Check aid JA1_TURN_SIGHT (window_balloon_press): the sighting at the turn start only with the aid set.
fn turn_sight_aid(reads: bool, aid: bool) -> bool {
    reads && aid
}
/// panel.md "The guide's and Brenda's balloons": the map click left quotes to show.
fn quotes_wait<T>(said: &[T]) -> bool {
    !said.is_empty()
}

/// Release net after a box took the release event (WaitLeftUp 0x67C54 reads the level): (left press ends, mouse hold ends).
fn release_net(left: bool, right: bool) -> (bool, bool) {
    (!left, !left && !right)
}
/// main_ 0x10150 in the port: a word goes to ParseCmdArg when it is known, or when the two paths are given and it is
/// letters only and names no file.
fn takes_cmd_word(unknown: bool, paths: usize, letters: bool, exists: impl FnOnce() -> bool) -> bool {
    !unknown || (paths >= 2 && letters && !exists())
}
/// The office to A.I.M. music fade: before the picture fade from field 2, after it from field 3 (`aim_fade_first`).
fn aim_music_fade_now(fade_first: bool, after_picture: bool) -> bool {
    fade_first == after_picture
}
/// The map view has its full 14 rows of 13 pixels (no small panel).
fn full_view(view_h: i32) -> bool {
    view_h == 14 * 13
}
/// Either Ctrl key.
fn ctrl_down(ms: sdl2::keyboard::Mod) -> bool {
    ms.intersects(sdl2::keyboard::Mod::LCTRLMOD | sdl2::keyboard::Mod::RCTRLMOD)
}
/// Either Shift key.
fn shift_down(ms: sdl2::keyboard::Mod) -> bool {
    ms.intersects(sdl2::keyboard::Mod::LSHIFTMOD | sdl2::keyboard::Mod::RSHIFTMOD)
}
/// ShowMouse 0x673AC: byte 0x92E4F is 1 while any of the boxes, screens or held presses is open.
fn show_mouse_flag(open: [bool; 10]) -> bool {
    open.iter().any(|&b| b)
}
/// The wait after the press on Contact A.I.M. ends when both buttons are up.
fn buttons_up(left: bool, right: bool) -> bool {
    !left && !right
}
/// TacticalKeyHandler 0x13E78: tactical key M does nothing while the guard screen may not open.
fn key_m_refused(open_map: bool, day: bool, opens: bool) -> bool {
    open_map && !day && !opens
}
/// 0x47CE4: the slide-in's shape 201 goes back to shape 0 once the assignment screen is open.
fn slide_shape_ends(base: i32, assign_open: bool) -> bool {
    base == 201 && assign_open
}
/// The held item drawn as the pointer leaves the map picture when a box starts and comes back when it ends.
fn box_change_redraws(box_now: bool, box_before: bool, held: u8) -> bool {
    box_now != box_before && ja1front::pointer::held_is_pointer(held)
}
/// A strip button press draws its face in one pass, then only the timer runs.
fn face_waits(pressed: bool, shown_before: bool) -> bool {
    pressed && shown_before
}

fn press_stops_the_game(held: bool, stat_keys: bool, switch_ui: bool) -> bool {
    held || stat_keys || switch_ui
}

/// The window's part of a guard screen step (strategic.md "The guard screen"): the pointer goes, the tactical picture
/// fades; the other steps are the library's (`Play::guard_step`).
#[derive(Debug, PartialEq)]
enum GuardWin {
    HidePointer,
    Fade(i32, i32, i32),
    Library,
}

fn guard_window_step(k: ja1front::guardopen::Step) -> GuardWin {
    use ja1front::guardopen::Step;
    match k {
        Step::HideMouse => GuardWin::HidePointer,
        Step::Fade(n, f, t) => GuardWin::Fade(n, f, t),
        _ => GuardWin::Library,
    }
}

/// A key on the A.I.M. screen as the bytes of the original's key read (0 + scan code for an extended key).
fn aim_key_bytes(k: Keycode, keymod: sdl2::keyboard::Mod) -> Vec<u8> {
    use sdl2::keyboard::Mod;
    match k {
        Keycode::Escape => vec![27],
        Keycode::Return | Keycode::KpEnter => vec![13],
        Keycode::Space => vec![32],
        Keycode::X if keymod.intersects(Mod::LALTMOD | Mod::RALTMOD) => vec![0, 0x2D],
        // letters as their bytes (V, L, T; Y / N / O answer a question)
        _ if (Keycode::A.into_i32()..=Keycode::Z.into_i32()).contains(&k.into_i32()) => vec![k.into_i32() as u8],
        Keycode::Up => vec![0, 0x48],
        Keycode::Down => vec![0, 0x50],
        Keycode::Left => vec![0, 0x4B],
        Keycode::Right => vec![0, 0x4D],
        Keycode::PageUp => vec![0, 0x49],
        Keycode::PageDown => vec![0, 0x51],
        _ => vec![],
    }
}

/// A key in the office (strategic.md "The office").
#[derive(Debug, PartialEq)]
enum OfficeKey {
    /// Key V (the View Team ledger), or any key while a message shows (it ends the message).
    V,
    /// Alt+X: "Exit to DOS".
    Exit,
    None,
}

fn office_key(k: Option<Keycode>, keymod: sdl2::keyboard::Mod, msg: bool) -> OfficeKey {
    match k {
        Some(Keycode::V) => OfficeKey::V,
        _ if msg => OfficeKey::V,
        Some(Keycode::X) if keymod.intersects(sdl2::keyboard::Mod::LALTMOD | sdl2::keyboard::Mod::RALTMOD) => OfficeKey::Exit,
        _ => OfficeKey::None,
    }
}

/// Where a key goes on the tactical screen (no map open).
#[derive(Debug, PartialEq)]
enum KeyWay {
    /// Escape answers an open aim or confirm (or the left sector's screen).
    Escape,
    /// While an aim or a confirm is open, other keys do nothing (world_load.md section 8 "Click").
    Swallow,
    /// TacticalKeyHandler 0x13490 (tactkeys::route).
    Tactical,
    /// Later arms (the sector is left).
    Other,
}

fn key_way(escape: bool, left: bool, wait: bool) -> KeyWay {
    if escape && (left || wait) {
        KeyWay::Escape
    } else if wait {
        KeyWay::Swallow
    } else if !left {
        KeyWay::Tactical
    } else {
        KeyWay::Other
    }
}

/// The keys of the island map and the assignment screen as the original's key codes: Escape, Enter, M T P G (view
/// modes), Y / N (the answer of a question box, question::key_answer), O, E, L; any other key is 0.
fn map_key_byte(k: Keycode) -> u8 {
    match k {
        Keycode::Escape => 27,
        Keycode::Return | Keycode::KpEnter => 13,
        Keycode::M => b'M',
        Keycode::T => b'T',
        Keycode::P => b'P',
        Keycode::G => b'G',
        Keycode::Y => b'Y',
        Keycode::N => b'N',
        Keycode::O => b'O',
        Keycode::E => b'E',
        Keycode::L => b'L',
        _ => 0,
    }
}

#[cfg(all(test, feature = "gui"))]
mod window_tests {
    /// Ledger rows ja1front:355/361 (popup_on), 838/845 (box_wait), 1418/1539 (play_window), 829/857
    /// (pointer_hold_end), 1981 (WindowWaitKey), 1752/1764 (note_last_key), 2170 (dos_key_of F11).
    #[test]
    fn window_input_rules() {
        use sdl2::event::Event;
        use sdl2::keyboard::{Keycode, Mod};
        // The popup block: only on-screen pixels are read; rows above the screen save 0.
        let clean: Vec<u8> = (0..64000).map(|i| (i % 251) as u8 + 1).collect();
        let save = super::popup_save(&clean, 310, -4);
        assert_eq!(save[0], 0);
        assert_eq!(save[16 * 4], clean[310]);
        assert_eq!(save[16 * 4 + 10], 0);
        let save = super::popup_save(&clean, -3, 195);
        assert_eq!(save[3], clean[320 * 195]);
        assert_eq!(save[16 * 5 + 3], 0);
        // Mouse driver bits: left 1, right 2, presses or-ed in.
        assert_eq!(super::mouse_bits(true, false, 1), 1);
        assert_eq!(super::mouse_bits(false, true, 0), 2);
        assert_eq!(super::mouse_bits(true, true, 2), 3);
        // The end of a hold with the pointer back on clears the left-the-window flag.
        super::POINTER_LEFT.with(|v| v.set(true));
        super::pointer_hold_end(false, 1, true, false);
        assert!(!super::POINTER_LEFT.with(|v| v.get()));
        super::POINTER_LEFT.with(|v| v.set(true));
        super::pointer_hold_end(true, 1, true, false);
        assert!(super::POINTER_LEFT.with(|v| v.get()));
        super::POINTER_LEFT.with(|v| v.set(false));
        // Keys stay queued in a right press alone and in a hold wait alone (row 0b4d529:1058).
        assert!(super::keys_held(false, false, true, false));
        assert!(super::keys_held(false, false, false, true));
        assert!(super::keys_held(true, false, false, false));
        assert!(super::keys_held(false, true, false, false));
        assert!(!super::keys_held(false, false, false, false));
        // A middle press or release on the open island map stays there; with the map closed it goes on (row 1143).
        let mid = Event::MouseButtonDown { timestamp: 0, window_id: 0, which: 0, mouse_btn: sdl2::mouse::MouseButton::Middle,
                                           clicks: 1, x: 5, y: 5 };
        let mid_up = Event::MouseButtonUp { timestamp: 0, window_id: 0, which: 0, mouse_btn: sdl2::mouse::MouseButton::Middle,
                                            clicks: 1, x: 5, y: 5 };
        assert!(super::map_eats_button(&mid, true));
        assert!(super::map_eats_button(&mid_up, true));
        assert!(!super::map_eats_button(&mid, false));
        assert!(!super::map_eats_button(&Event::Quit { timestamp: 0 }, true));
        // WaitKeyOrClick: a press sets its button in the level.
        let mut k = super::WindowWaitKey::new();
        let down = Event::MouseButtonDown { timestamp: 0, window_id: 0, which: 0, mouse_btn: sdl2::mouse::MouseButton::Left,
                                            clicks: 1, x: 5, y: 5 };
        k.event(&down);
        assert_eq!(k.level, 1);
        // A key in a KeyOrClick wait goes to byte 0x95DC3.
        super::LAST_KEY.with(|v| v.set(0));
        let key = Event::KeyDown { timestamp: 0, window_id: 0, keycode: Some(Keycode::A), scancode: None, keymod: Mod::NOMOD,
                                   repeat: false };
        super::note_last_key(&key);
        assert_ne!(super::LAST_KEY.with(|v| v.get()), 0);
        // Balloon wait rules (ledger rows ja1front:989, 993, 961, 999, 1464).
        assert!(super::seen_or_held(true, false) && super::seen_or_held(false, true) && !super::seen_or_held(false, false));
        assert!(super::any_input(true, false, false) && super::any_input(false, true, false) && super::any_input(false, false, true));
        assert!(!super::any_input(false, false, false));
        assert!(super::pass_owes_ticks(true, true) && !super::pass_owes_ticks(true, false) && !super::pass_owes_ticks(false, true));
        // The box wait: the window close ends it (ledger row ja1front:867).
        let (mut kk, mut q, mut d) = (false, false, 0u8);
        super::box_event(&Event::Quit { timestamp: 0 }, &mut kk, &mut q, &mut d);
        assert!(q && !kk && d == 0);
        super::box_event(&down, &mut kk, &mut q, &mut d);
        assert_eq!(d, 1);
        // A scene's key poll: the window close sets quit (ledger row ja1front:2069).
        let mut q = false;
        assert_eq!(super::sab_event(&Event::Quit { timestamp: 0 }, &mut q), None);
        assert!(q);
        // The question box takes no key while a button is held (ledger row ja1front:709).
        assert!(super::question_key(Keycode::Return, false).is_some());
        assert_eq!(super::question_key(Keycode::Return, true), None);
        // Ctrl on the map: either Ctrl key (ledger row ja1front:1225).
        assert!(super::ctrl_down(Mod::LCTRLMOD) && super::ctrl_down(Mod::RCTRLMOD) && !super::ctrl_down(Mod::LSHIFTMOD));
        // The full map view: 14 rows of 13 pixels (ledger row ja1front:1295).
        assert!(super::full_view(182) && !super::full_view(1) && !super::full_view(169));
        // A.I.M. music fade order (ledger row ja1front:1409).
        assert!(super::aim_music_fade_now(false, false) && !super::aim_music_fade_now(false, true));
        assert!(super::aim_music_fade_now(true, true) && !super::aim_music_fade_now(true, false));
        // Command words (ledger row ja1front:594): a letters-only word that names a file is a path.
        assert!(super::takes_cmd_word(false, 0, false, || true));
        assert!(super::takes_cmd_word(true, 2, true, || false));
        assert!(!super::takes_cmd_word(true, 2, true, || true));
        assert!(!super::takes_cmd_word(true, 2, false, || false));
        assert!(!super::takes_cmd_word(true, 1, true, || false));
        // F11 = scan code 0x85.
        assert_eq!(super::dos_key_of(Keycode::F11, Mod::NOMOD).map(|k| k.1), Some(0x85));
    }
    /// Ledger rows d057aca:ja1front:1026/1172/1266/1280/1406/1407/1453/1466/1472/1489: small rules of the window loop.
    #[test]
    fn the_window_loop_rules() {
        use sdl2::keyboard::Mod;
        assert_eq!(super::release_net(false, true), (true, false));
        assert_eq!(super::release_net(false, false), (true, true));
        assert_eq!(super::release_net(true, false), (false, false));
        assert!(super::shift_down(Mod::LSHIFTMOD) && super::shift_down(Mod::RSHIFTMOD) && !super::shift_down(Mod::LCTRLMOD));
        assert!(super::quotes_wait(&[1]) && !super::quotes_wait::<u8>(&[]));
        for i in 0..10 {
            let mut o = [false; 10];
            o[i] = true;
            assert!(super::show_mouse_flag(o), "{i}");
        }
        assert!(!super::show_mouse_flag([false; 10]));
        assert!(super::buttons_up(false, false) && !super::buttons_up(true, false) && !super::buttons_up(false, true));
        assert!(super::key_m_refused(true, false, false));
        assert!(!super::key_m_refused(true, false, true) && !super::key_m_refused(true, true, false) && !super::key_m_refused(false, false, false));
        assert!(super::slide_shape_ends(201, true) && !super::slide_shape_ends(201, false) && !super::slide_shape_ends(0, true));
        let held = (1..=255u8).find(|&h| ja1front::pointer::held_is_pointer(h)).unwrap();
        assert!(super::box_change_redraws(true, false, held) && super::box_change_redraws(false, true, held));
        assert!(!super::box_change_redraws(true, true, held) && !super::box_change_redraws(true, false, 0));
        assert!(super::face_waits(true, true) && !super::face_waits(true, false) && !super::face_waits(false, true));
    }
    /// Ledger row c2aea50:ja1front:1657: each of a held strip press, the stat keys and the switch box alone stops the game.
    #[test]
    fn each_held_box_stops_the_game() {
        assert!(super::press_stops_the_game(true, false, false));
        assert!(super::press_stops_the_game(false, true, false));
        assert!(super::press_stops_the_game(false, false, true));
        assert!(!super::press_stops_the_game(false, false, false));
    }
    /// Ledger rows c2aea50:ja1front:1581/1582: on the way to the guard screen (key M) the window hides the pointer,
    /// then fades the tactical picture out in 24 steps (GuardScreenFromTactical 0x1331B).
    #[test]
    fn the_guard_screen_way_hides_the_pointer_then_fades() {
        use super::{guard_window_step, GuardWin};
        let w: Vec<GuardWin> = ja1front::guardopen::ENTER.into_iter().map(guard_window_step).collect();
        assert_eq!(w, vec![GuardWin::Library, GuardWin::HidePointer, GuardWin::Fade(24, 256, 0), GuardWin::Library, GuardWin::Library]);
    }
    /// Ledger rows c2aea50:ja1front:1526/1530: on the A.I.M. screen the arrow and page keys are extended keys (0 +
    /// scan code: Up 0x48, PageUp 0x49 ...), as the original's key read gives them.
    #[test]
    fn a_balloon_click_is_left_or_right() {
        // Ledger row d057aca:ja1front:2108.
        use sdl2::mouse::MouseButton;
        let mut i = ja1front::faceanim::WaitInput::default();
        super::balloon_click(&mut i, MouseButton::Left);
        assert!(i.left && !i.right);
        let mut i = ja1front::faceanim::WaitInput::default();
        super::balloon_click(&mut i, MouseButton::Right);
        assert!(!i.left && i.right);
    }

    #[test]
    fn aim_arrow_keys_are_extended() {
        use sdl2::keyboard::{Keycode, Mod};
        let k = |c| super::aim_key_bytes(c, Mod::NOMOD);
        assert_eq!(k(Keycode::Up), vec![0, 0x48]);
        assert_eq!(k(Keycode::Down), vec![0, 0x50]);
        assert_eq!(k(Keycode::PageUp), vec![0, 0x49]);
        assert_eq!(k(Keycode::PageDown), vec![0, 0x51]);
        assert_eq!(k(Keycode::Escape), vec![27]);
        // Ledger row 0b4d529:ja1front:2215: Space gives the byte 32 (the A.I.M. waits end on any key, aim.md).
        assert_eq!(k(Keycode::Space), vec![32]);
        assert_eq!(super::aim_key_bytes(Keycode::X, Mod::LALTMOD), vec![0, 0x2D]);
    }
    /// Ledger row 0b4d529:ja1front:1410 (the A.I.M. screen's poll): closing the window ends the program; a press
    /// gives the place and its button bit, a key its bytes, and neither ends the screen.
    #[test]
    fn aim_poll_events_close_press_and_key() {
        use sdl2::event::Event;
        use sdl2::keyboard::{Keycode, Mod};
        use sdl2::mouse::MouseButton;
        let (mut mouse, mut down, mut keys) = ((0, 0), 0, Vec::new());
        let half = |x: i32, y: i32| (x / 2, y / 2);
        assert!(super::aim_event(&Event::Quit { timestamp: 0 }, half, &mut mouse, &mut down, &mut keys), "closing the window was ignored");
        let press = Event::MouseButtonDown { timestamp: 0, window_id: 0, which: 0, mouse_btn: MouseButton::Right, clicks: 1, x: 100, y: 40 };
        assert!(!super::aim_event(&press, half, &mut mouse, &mut down, &mut keys));
        assert_eq!((mouse, down), ((50, 20), 2));
        let key = Event::KeyDown { timestamp: 0, window_id: 0, keycode: Some(Keycode::Escape), scancode: None, keymod: Mod::NOMOD, repeat: false };
        assert!(!super::aim_event(&key, half, &mut mouse, &mut down, &mut keys));
        assert_eq!(keys, vec![27]);
    }
    /// Ledger rows c2aea50:ja1front:1464/1465: in the office any key ends a message (as key V), Alt+X exits, a plain
    /// X does nothing.
    /// Ledger rows d057aca:ja1front:803/804: a left press and the arrow keys reach the main menu.
    #[test]
    fn main_menu_events() {
        use sdl2::event::Event;
        use sdl2::keyboard::{Keycode, Mod};
        use sdl2::mouse::{MouseButton, MouseState};
        let mut keys = std::collections::VecDeque::new();
        let press = Event::MouseButtonDown { timestamp: 0, window_id: 0, which: 0, mouse_btn: MouseButton::Left, clicks: 1, x: 5, y: 6 };
        assert_eq!(super::menu_event(&press, &mut keys), Some((5, 6, true)));
        let right = Event::MouseButtonDown { timestamp: 0, window_id: 0, which: 0, mouse_btn: MouseButton::Right, clicks: 1, x: 5, y: 6 };
        assert_eq!(super::menu_event(&right, &mut keys), None);
        let mv = Event::MouseMotion { timestamp: 0, window_id: 0, which: 0, mousestate: MouseState::from_sdl_state(0), x: 7, y: 8, xrel: 0, yrel: 0 };
        assert_eq!(super::menu_event(&mv, &mut keys), Some((7, 8, false)));
        for k in [Keycode::Up, Keycode::Down, Keycode::KpEnter] {
            let ev = Event::KeyDown { timestamp: 0, window_id: 0, keycode: Some(k), scancode: None, keymod: Mod::NOMOD, repeat: false };
            assert_eq!(super::menu_event(&ev, &mut keys), None);
        }
        assert_eq!(keys, [0, 0x48, 0, 0x50, 13]);
    }

    /// Ledger row d057aca:ja1front:693: the Control Panel moves the mouse back only out of its box and with focus.
    #[test]
    fn control_panel_mouse_hold() {
        let focus = sdl2::sys::SDL_WindowFlags::SDL_WINDOW_INPUT_FOCUS as u32;
        assert!(super::panel_warps((10, 10), (5, 10), focus));
        assert!(!super::panel_warps((10, 10), (10, 10), focus));
        assert!(!super::panel_warps((10, 10), (5, 10), 0));
    }

    /// Ledger row d057aca:ja1front:977: the end of a balloon shows the pointer again only on the drawn tactical screen.
    #[test]
    fn balloon_end_pointer_needs_the_drawn_screen() {
        assert!(!super::tactical_ready(0));
        assert!(super::tactical_ready(1));
        assert!(ja1front::pointer::mouse_redraw(true, 0, super::tactical_ready(0)));
        assert!(!ja1front::pointer::mouse_redraw(true, 0, super::tactical_ready(1)));
    }

    /// Ledger row d057aca:ja1front:2182: the keypad Enter gives the BIOS Enter byte 13, as the main Enter key.
    #[test]
    fn keypad_enter_is_enter() {
        use sdl2::keyboard::{Keycode, Mod};
        assert_eq!(super::dos_key_of(Keycode::KpEnter, Mod::NOMOD).map(|k| k.0), Some(13));
        assert_eq!(super::dos_key_of(Keycode::KpEnter, Mod::NOMOD), super::dos_key_of(Keycode::Return, Mod::NOMOD));
    }

    /// Ledger rows 0b4d529:ja1front:964/968/1002/2060/2062/2092/1964/926/1540: small rules of the balloon wait,
    /// the day-end buttons, the speech button-up wait and a check aid (panel.md "Speech balloon", "Face animation").
    #[test]
    fn the_balloon_and_button_rules() {
        use std::time::Duration;
        assert!(super::weapon_text_frame(4) && !super::weapon_text_frame(3) && !super::weapon_text_frame(0));
        let ms = |v| Duration::from_millis(v);
        assert!(super::balloon_waits(true, ms(900), 500) && super::balloon_waits(false, ms(400), 500));
        assert!(!super::balloon_waits(false, ms(500), 500));
        assert!(super::right_undoes_move(true, true));
        assert!(!super::right_undoes_move(true, false) && !super::right_undoes_move(false, true));
        assert!(super::has_voice(1) && !super::has_voice(0));
        assert!(super::voice_plays(true, ms(1100), 1000) && !super::voice_plays(true, ms(1200), 1000));
        assert!(!super::voice_plays(false, ms(0), 1000));
        assert_eq!((super::button_bits(0, true, false), super::button_bits(0, false, true), super::button_bits(1, false, true)), (1, 2, 3));
        assert_eq!(super::button_bits(0, false, false), 0);
        // Ledger row 0b4d529:ja1front:1967: a press that is still held counts once (bit 1 stays set).
        assert_eq!((super::button_bits(1, true, false), super::button_bits(3, true, true)), (1, 3));
        assert!(super::wanted_buttons_up(true, false, false, true) && super::wanted_buttons_up(false, true, true, false));
        assert!(!super::wanted_buttons_up(true, true, false, true) && !super::wanted_buttons_up(true, true, true, false));
        assert!(super::wanted_buttons_up(true, true, false, false));
        assert!(super::turn_sight_aid(true, true) && !super::turn_sight_aid(true, false) && !super::turn_sight_aid(false, true));
    }

    /// Ledger row 0b4d529:ja1front:2152: the four arrow keys give their BIOS scan codes (Up 0x48, Left 0x4B,
    /// Right 0x4D, Down 0x50; TacticalKeyHandler scrolls the view by them, the original reads INT 16h scan codes).
    #[test]
    fn the_arrow_keys_give_their_bios_scan_codes() {
        use sdl2::keyboard::{Keycode, Mod};
        for (k, scan) in [(Keycode::Up, 0x48), (Keycode::Left, 0x4B), (Keycode::Right, 0x4D), (Keycode::Down, 0x50)] {
            let want = ja1front::tactkeys::dos_key(k.into_i32(), 0, scan, false, false, false);
            assert!(want.is_some(), "{k:?}");
            assert_eq!(super::dos_key_of(k, Mod::NOMOD), want, "{k:?}");
            assert_ne!(super::dos_key_of(k, Mod::NOMOD), ja1front::tactkeys::dos_key(k.into_i32(), 0, 0, false, false, false), "{k:?}");
        }
    }

    #[test]
    fn office_keys() {
        use super::{office_key, OfficeKey};
        use sdl2::keyboard::{Keycode, Mod};
        assert_eq!(office_key(Some(Keycode::V), Mod::NOMOD, false), OfficeKey::V);
        assert_eq!(office_key(Some(Keycode::Q), Mod::NOMOD, true), OfficeKey::V);
        assert_eq!(office_key(Some(Keycode::Q), Mod::NOMOD, false), OfficeKey::None);
        assert_eq!(office_key(Some(Keycode::X), Mod::LALTMOD, false), OfficeKey::Exit);
        assert_eq!(office_key(Some(Keycode::X), Mod::NOMOD, false), OfficeKey::None);
    }
    /// Ledger rows c2aea50:ja1front:1191/1192: while an aim or a confirm is open, Escape answers it and other keys do
    /// nothing; else keys go to TacticalKeyHandler, but not after the sector is left.
    #[test]
    fn keys_wait_while_an_aim_is_open() {
        use super::{key_way, KeyWay};
        assert_eq!(key_way(true, false, true), KeyWay::Escape);
        assert_eq!(key_way(true, true, false), KeyWay::Escape);
        assert_eq!(key_way(false, false, true), KeyWay::Swallow);
        assert_eq!(key_way(false, false, false), KeyWay::Tactical);
        assert_eq!(key_way(true, false, false), KeyWay::Tactical);
        assert_eq!(key_way(false, true, false), KeyWay::Other);
    }
    /// Ledger row c2aea50:ja1front:1215: key N answers No to a question box on the island map.
    #[test]
    fn key_n_answers_no_on_the_map() {
        use sdl2::keyboard::Keycode;
        assert_eq!(super::map_key_byte(Keycode::N), b'N');
        assert_eq!(ja1front::question::key_answer(super::map_key_byte(Keycode::N)), Some(false));
        assert_eq!(ja1front::question::key_answer(super::map_key_byte(Keycode::Y)), Some(true));
        assert_eq!(super::map_key_byte(Keycode::Q), 0);
    }

    /// Ledger row c2aea50:ja1front:2014: the DOS mouse button mask, bit 1 left and bit 2 right (KeyOrClick? 0x67E44
    /// tests 0x229CF4 & 1 and & 2); other buttons count as none.
    #[test]
    fn the_right_button_is_dos_bit_2() {
        use sdl2::mouse::MouseButton;
        assert_eq!(super::dos_button(MouseButton::Left), 1);
        assert_eq!(super::dos_button(MouseButton::Right), 2);
        assert_eq!(super::dos_button(MouseButton::Middle), 0);
    }
}

#[cfg(test)]
mod save_dir_tests {
    /// Section 10: the day saves go beside the loaded save, as the original writes them into its game folder (the
    /// folder it loads from); `JA1_SAVE_DIR` names another folder, and the play script with no folder writes none.
    #[test]
    fn the_day_saves_go_beside_the_loaded_save() {
        use std::path::{Path, PathBuf};
        let beside = Path::new("games").join("week1");
        let save = beside.join("DAY.SAV");
        assert_eq!(super::save_dir_of(None, save.to_str()), Some(beside.clone()));
        assert_eq!(super::save_dir_of(Some(String::new()), save.to_str()), Some(beside));
        assert_eq!(super::save_dir_of(None, Some("DAY.SAV")), Some(PathBuf::from(".")));
        assert_eq!(super::save_dir_of(Some("other".into()), save.to_str()), Some(PathBuf::from("other")));
        assert_eq!(super::save_dir_of(None, None), None);
        assert_eq!(super::save_dir_of(Some("other".into()), None), Some(PathBuf::from("other")));
    }
}

/// The window bodies of the films and the day-end screen on SDL's dummy video driver (no desktop needed).
#[cfg(all(test, feature = "gui"))]
mod sdl_window_tests {
    use super::{ButtonRead, DayEndWin, Frame, WinSab, SH, SW};
    use sdl2::pixels::PixelFormatEnum;
    use std::time::Instant;
    static SDL_LOCK: std::sync::Mutex<()> = std::sync::Mutex::new(());

    fn files() -> Option<(String, String)> {
        let root = concat!(env!("CARGO_MANIFEST_DIR"), "/../..");
        let (iso, exe) = (format!("{}/rom/JA1.gog", root), format!("{}/build/game113b/JA.EXE", root));
        (std::path::Path::new(&iso).exists() && std::path::Path::new(&exe).exists()).then_some((iso, exe))
    }

    fn with_window(f: impl FnOnce(&mut Frame, &mut sdl2::render::Texture<'static>, &mut sdl2::EventPump)) {
        let _g = SDL_LOCK.lock().unwrap_or_else(|e| e.into_inner());
        sdl2::hint::set("SDL_VIDEODRIVER", "dummy");
        let sdl = sdl2::init().expect("sdl");
        let video = sdl.video().expect("video");
        let mut cv = Frame::new(video.window("test", SW, SH).hidden(), SW, SH);
        let tc: &'static sdl2::render::TextureCreator<_> = Box::leak(Box::new(cv.texture_creator()));
        let mut tex = tc.create_texture_streaming(PixelFormatEnum::RGB24, SW, SH).expect("texture");
        let mut ev = sdl.event_pump().expect("events");
        f(&mut cv, &mut tex, &mut ev);
    }

    /// The colour in the middle of the window.
    fn middle(cv: &Frame) -> [u8; 3] {
        let (w, h) = cv.window().size();
        let px = cv.read_pixels(None, PixelFormatEnum::RGB24).expect("pixels");
        let at = ((h / 2) * w + w / 2) as usize * 3;
        [px[at], px[at + 1], px[at + 2]]
    }

    /// SDL starts on one thread only, so one test holds both window bodies.
    #[test]
    fn window_bodies_on_the_dummy_driver() {
        a_film_shows_its_frames_plays_its_effects_and_stops_its_theme();
        the_day_end_screen_shows_its_picture();
        the_end_show_fades_loads_effects_and_speaks();
        the_film_shows_draw_subtitles_by_the_option_and_captions();
        the_speech_waits_hold_until_the_buttons_are_up();
        let _g = SDL_LOCK.lock().unwrap_or_else(|e| e.into_inner());
        sdl2::hint::set("SDL_VIDEODRIVER", "dummy");
        let sdl = sdl2::init().expect("sdl");
        super::frame::dos_draw_tests::the_pointer_is_drawn_into_the_picture(&sdl.video().expect("video"));
        drop(_g);
        the_day_end_poll_sees_a_key_or_a_press();
    }

    /// Ledger row 0b4d529:ja1front:1744 (eod.rs, the end-of-day waits end on a key or a button press,
    /// KeyOrClick 0x67E44): the window's poll is false with no input and true after a key or a press.
    fn the_day_end_poll_sees_a_key_or_a_press() {
        use ja1front::eod::EodSink;
        use sdl2::event::Event;
        use sdl2::keyboard::{Keycode, Mod, Scancode};
        with_window(|cv, tex, ev| {
            let es = sdl2::init().expect("sdl").event().expect("events");
            let mut sink = super::WinEod { canvas: cv, tex, events: ev, last: Instant::now(), audio: std::ptr::null(), clock: None,
                                           keys: Default::default() };
            while sink.poll() {}
            assert!(!sink.poll(), "no input");
            es.push_event(Event::KeyDown { timestamp: 0, window_id: 0, keycode: Some(Keycode::A), scancode: Some(Scancode::A),
                                           keymod: Mod::NOMOD, repeat: false }).expect("push");
            assert!(sink.poll(), "a key");
            assert!(!sink.poll(), "the key counts once");
            es.push_event(Event::MouseButtonDown { timestamp: 0, window_id: 0, which: 0, mouse_btn: sdl2::mouse::MouseButton::Left,
                                                   clicks: 1, x: 10, y: 10 }).expect("push");
            assert!(sink.poll(), "a press");
        });
    }

    /// Held buttons for the button-up wait: one (left, right) state per read, then both up.
    struct Held(Vec<(bool, bool)>, usize);
    impl ButtonRead for Held {
        fn buttons(&mut self) -> (bool, bool, i32, i32) {
            self.1 += 1;
            let (l, r) = if self.0.is_empty() { (false, false) } else { self.0.remove(0) };
            (l, r, 0, 0)
        }
    }

    /// Ledger row 0b4d529:ja1front:924 (WaitLeftUp 0x67C54 / WaitRightUp 0x67C68 of the speech roads, live #31
    /// comment 11937): the wait goes on while a button that it waits for is down, the timer ticks in it, and it gives
    /// its time (at least 1 ms); with the buttons up it gives 0 at once. A right button held in a left-only wait does
    /// not hold it.
    fn the_speech_waits_hold_until_the_buttons_are_up() {
        with_window(|cv, _tex, _ev| {
            let mut ticks = 0;
            let mut h = Held(vec![(true, false), (true, true), (false, true)], 0);
            let ms = super::speech_buttons_up(cv, &mut h, true, true, |_| ticks += 1);
            assert!(ms >= 1 && ticks == 3 && h.1 == 4, "held 3 reads: ms {ms}, ticks {ticks}, reads {}", h.1);
            let (mut ticks, mut h) = (0, Held(vec![], 0));
            assert_eq!(super::speech_buttons_up(cv, &mut h, true, true, |_| ticks += 1), 0);
            assert_eq!((ticks, h.1), (0, 1));
            let mut h = Held(vec![(false, true)], 0);
            assert_eq!(super::speech_buttons_up(cv, &mut h, true, false, |_| {}), 0, "the right button held a left-only wait");
        });
    }

    /// Ledger row 0b4d529:ja1front:17097 (ending.md op 29, op 30): the window's film shows give op 29 the Subtitles
    /// option: off, a quote below 77 draws no text; on, it draws INTQUOTE's text in rows 152 and below; quote 79 (the
    /// win sequence) draws with the option off. Op 30 draws the intro caption "Metavira Island" at y 85.
    fn the_film_shows_draw_subtitles_by_the_option_and_captions() {
        let Some((iso, exe)) = files() else { return };
        with_window(|cv, tex, ev| {
            let dir = std::path::Path::new(&exe).parent().unwrap().to_path_buf();
            let mut io = WinSab { canvas: cv, tex, events: ev, dir, iso: iso.clone(), t0: Instant::now(), quit: false, subtitles: false };
            let drawn = |io: &mut WinSab, q: i32| {
                let mut s = ja1front::sab::Sab::default();
                s.screen = vec![0u8; (SW * SH) as usize];
                s.subtitle(io, q, 127, 25);
                let rows: Vec<usize> = (0..SH as usize).filter(|y| s.screen[y * SW as usize..(y + 1) * SW as usize].iter().any(|&b| b != 0)).collect();
                (rows.first().copied(), rows.len())
            };
            assert_eq!(drawn(&mut io, 38), (None, 0), "a quote below 77 drew text with the Subtitles option off");
            let (first, n) = drawn(&mut io, 79);
            assert!(first.is_some_and(|y| y >= 152) && n > 0, "quote 79 drew no text in rows 152..: {:?}", (first, n));
            io.subtitles = true;
            let (first, n) = drawn(&mut io, 38);
            assert!(first.is_some_and(|y| y >= 152) && n > 0, "quote 38 drew no text with the option on: {:?}", (first, n));
            use ja1front::sab::SabIo;
            let mut screen = vec![0u8; (SW * SH) as usize];
            io.draw_caption(&mut screen, b"Metavira Island", 85);
            let rows: Vec<usize> = (0..SH as usize).filter(|y| screen[y * SW as usize..(y + 1) * SW as usize].iter().any(|&b| b != 0)).collect();
            assert!(rows.first().is_some_and(|&y| (80..=90).contains(&y)), "the caption rows: {:?}", rows.first());
        });
    }

    /// Ledger rows c2aea50:ja1front:2064/2128/2130: a film frame shows its screen in its DAC colours (6-bit values
    /// scaled to 8 bits), the film's effect slot plays on the screen sound, and the end of a film stops the theme.
    fn a_film_shows_its_frames_plays_its_effects_and_stops_its_theme() {
        use ja1front::sab::SabIo;
        let Some((iso, exe)) = files() else { return };
        with_window(|cv, tex, ev| {
            let dir = std::path::Path::new(&exe).parent().unwrap().to_path_buf();
            let mut io = WinSab { canvas: cv, tex, events: ev, dir, iso: iso.clone(), t0: Instant::now(), quit: false, subtitles: false };
            let mut dac = [0u8; 1024];
            dac[4 * 7..4 * 7 + 3].copy_from_slice(&[63, 32, 0]);
            io.show(&vec![7u8; (SW * SH) as usize], &dac);
            assert_eq!(middle(io.canvas), [255, 130, 0]);
            // Effect slot 3 holds one second of sound: the film's sfx call starts it on the screen sound.
            ja1front::sound::screen(|s| {
                *s = ja1front::sound::Sound::default();
                s.set_effect(3, "test", Some(&vec![128u8; 22050]));
            });
            io.sfx_play(3, 22050, 100, 1);
            assert!(ja1front::sound::screen(|s| s.effects_playing()) > 0, "the effect did not start");
            ja1front::sound::screen(|s| *s = ja1front::sound::Sound::default());
            ja1front::music::set_exe(&exe);
            ja1front::music::screen("INTRO", 1);
            assert_eq!(ja1front::music::with(|m| m.playing()), 1);
            io.music_end();
            assert_eq!(ja1front::music::with(|m| m.playing()), -1);
        });
    }

    /// Ledger rows d057aca:ja1front:1884/1886/1914/1916/1917: a fade of the end show (ops 24 / 25) shows each level
    /// and ends on its last level (here the full palette); the effect a SAB script loads goes into its slot of the
    /// screen sound; its voice plays on the screen sound and `speaking` reports it (false when nothing speaks).
    fn the_end_show_fades_loads_effects_and_speaks() {
        use ja1front::sab::SabIo;
        let Some((iso, exe)) = files() else { return };
        with_window(|cv, tex, ev| {
            let dir = std::path::Path::new(&exe).parent().unwrap().to_path_buf();
            let mut io = WinSab { canvas: cv, tex, events: ev, dir, iso: iso.clone(), t0: Instant::now(), quit: false, subtitles: false };
            io.show(&vec![0u8; (SW * SH) as usize], &[0u8; 1024]);
            assert_eq!(middle(io.canvas), [0, 0, 0]);
            let mut s = ja1front::sab::Sab::default();
            s.screen = vec![7u8; (SW * SH) as usize];
            let mut pal = [0u8; 1024];
            pal[4 * 7..4 * 7 + 3].copy_from_slice(&[63, 32, 0]);
            io.fade(&mut s, pal, 4, 0, 256);
            assert_eq!(&s.dac[4 * 7..4 * 7 + 3], &[63, 32, 0], "the fade did not end on the full palette");
            assert_eq!(middle(io.canvas), [255, 130, 0], "the fade did not show its last level");
            // Ledger row 0b4d529:ja1front:1862 (ending.md, the credits wait for the VGA retrace): one retrace
            // shows the screen and lasts one 1/70 s frame.
            io.show(&vec![0u8; (SW * SH) as usize], &[0u8; 1024]);
            let t = Instant::now();
            io.retrace(&s);
            assert!(t.elapsed().as_micros() >= 1_000_000 / 70, "a retrace shorter than 1/70 s");
            assert_eq!(middle(io.canvas), [255, 130, 0], "the retrace did not show the screen");
            ja1front::sound::screen(|s| *s = ja1front::sound::Sound::default());
            assert!(!io.speaking(), "speaking with no voice");
            io.sfx_load("TEST", 3, Some(&vec![128u8; 22050]));
            io.sfx_play(3, 22050, 100, 1);
            assert!(ja1front::sound::screen(|s| s.effects_playing()) > 0, "the loaded effect did not play");
            io.speech(22050, 100, &vec![128u8; 22050]);
            assert!(io.speaking(), "the voice did not start");
            ja1front::sound::screen(|s| *s = ja1front::sound::Sound::default());
        });
    }

    /// Ledger row c2aea50:ja1front:2196: the day-end screen shows its picture in its palette.
    fn the_day_end_screen_shows_its_picture() {
        let Some((iso, exe)) = files() else { return };
        let mut w = ja1front::world::World::new_game(&iso, &exe, 1, 1).expect("new game");
        let mut map = ja1front::mapui::MapUi::new(&iso, &exe, 1);
        with_window(|cv, tx, ev| {
            let mut pal = [0u8; 768];
            pal[3 * 9..3 * 9 + 3].copy_from_slice(&[10, 200, 30]);
            let mut host = DayEndWin { question: None, cv, tx, ev, map: &mut map, w: &mut w, s: vec![9u8; (SW * SH) as usize], pal,
                                       iso: &iso, exe: &exe, b: 0, at: (0, 0), buttons: 0, keys: Default::default(),
                                       tip: Default::default() };
            host.show();
            assert_eq!(middle(host.cv), [10, 200, 30]);
        });
    }
}
