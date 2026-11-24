//! Checks against the user's own game files; skipped when rom/ is missing.
use std::path::PathBuf;

fn rom() -> Option<(String, String)> {
    let d = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../rom");
    let (iso, exe) = (d.join("JA1.gog"), d.join("JA.EXE"));
    if iso.exists() && exe.exists() {
        Some((iso.to_string_lossy().into(), exe.to_string_lossy().into()))
    } else {
        eprintln!("rom/ missing: skipped");
        None
    }
}

#[test]
fn every_tileset_decodes() {
    let Some((iso, _)) = rom() else { return };
    for t in 0..10u8 {
        let ts = ja1front::tileset_data(&iso, t).unwrap();
        assert_eq!(ts.decode_errors, 0, "tileset {}", t);
        assert!(ts.sprites.iter().filter(|s| !s.is_empty()).count() > 1000);
    }
}

#[test]
fn every_sector_renders() {
    let Some((iso, exe)) = rom() else { return };
    for s in 1..=60 {
        let ts = ja1front::sector_tileset(&iso, s).unwrap();
        assert!(ts <= 9);
        for (c0, r0) in [(0, 0), (42, 44), (20, 20)] {
            let v = ja1front::render_view(&iso, &exe, s, c0, r0, 14, 12).unwrap();
            assert_eq!(v.len(), 224 * 168);
            assert!(v.iter().any(|&b| b != 0), "sector {} empty", s);
        }
    }
}

#[test]
fn errors() {
    let Some((iso, exe)) = rom() else { return };
    assert_eq!(ja1front::render_view(&iso, &exe, 0, 0, 0, 14, 12), Err(ja1front::E_SECTOR));
    assert_eq!(ja1front::render_view(&iso, &exe, 61, 0, 0, 14, 12), Err(ja1front::E_SECTOR));
    assert_eq!(ja1front::render_view(&iso, &exe, 1, 43, 0, 14, 12), Err(ja1front::E_VIEW));
    assert_eq!(ja1front::render_view("/nonexistent", &exe, 1, 0, 0, 14, 12), Err(ja1front::E_FILE));
    let p = ja1front::palette(&iso, 0).unwrap();
    assert!(p.iter().any(|&b| b > 200));
}

#[test]
fn c_entry_points() {
    let Some((iso, exe)) = rom() else { return };
    let iso_c = std::ffi::CString::new(iso).unwrap();
    let exe_c = std::ffi::CString::new(exe).unwrap();
    let mut out = vec![0u8; 224 * 168];
    let mut pal = [0u8; 768];
    unsafe {
        assert_eq!(ja1front::ja1f_render_view(iso_c.as_ptr(), exe_c.as_ptr(), 1, 0, 0, 14, 12, out.as_mut_ptr()), 0);
        let t = ja1front::ja1f_tileset(iso_c.as_ptr(), 1);
        assert!((0..=9).contains(&t));
        assert_eq!(ja1front::ja1f_palette(iso_c.as_ptr(), t, pal.as_mut_ptr()), 0);
        assert!(ja1front::ja1f_tileset(iso_c.as_ptr(), 0) < 0);
    }
}

#[test]
fn sector_entry() {
    let Some((iso, exe)) = rom() else { return };
    let save = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../build/DAY_before_restart.SAV");
    if !save.exists() {
        eprintln!("build/DAY_before_restart.SAV missing: skipped");
        return;
    }
    let save = save.to_string_lossy().into_owned();
    let a = ja1front::enter::enter(&iso, &exe, &save, 1793863554).unwrap();
    let b = ja1front::enter::enter(&iso, &exe, &save, 1793863554).unwrap();
    assert_eq!(a.world.image().len(), ja1front::world::IMAGE_SIZE);
    assert_eq!(a.world.image(), b.world.image());
    assert_eq!((a.seed, a.draws), (b.seed, b.draws));
    assert!(a.draws > 0);
    // Some soldier now stands on the map: a tile's occupant byte is below 0x30.
    assert!((0..3136).any(|k| a.world.tiles[k * 8 + 5] & 0x7F < 0x30));
    let c = ja1front::enter::enter(&iso, &exe, &save, 1).unwrap();
    assert_ne!(c.seed, a.seed);
}

/// u8 +off of soldier record `slot`.
fn rec8(w: &ja1front::world::World, slot: usize, off: usize) -> u8 {
    w.soldiers[slot * ja1front::soldier::REC + off]
}

#[test]
fn turn_cycle() {
    // docs/spec/turns.md: in turn-based combat a walk spends AP, End turn hands the turn over, and the new round
    // gives the mercs their AP back.
    let Some((iso, exe)) = rom() else { return };
    let save = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../build/DAY_before_restart.SAV");
    if !save.exists() {
        eprintln!("build/DAY_before_restart.SAV missing: skipped");
        return;
    }
    let mut e = ja1front::enter::enter(&iso, &exe, &save.to_string_lossy(), 1).unwrap();
    let mut host = ja1front::play::PlayHost::new(e.seed);
    host.log = false;
    let w = &mut e.world;
    w.combat = 1;
    w.update.screen_drawn = 1;
    w.update.turn_shown = 1;
    let slot = w.selected as usize;
    let tile = ja1front::play::soldier_tile(w, slot);
    let ap0 = rec8(w, slot, 0x1A5);
    {
        let mut s = ja1front::play::Session::new(w, &mut host);
        assert_eq!(s.move_order(slot as i32, tile as i32 + 3, 0, 0, 0, 0, 0), 1);
        for _ in 0..600 {
            s.tick();
            s.frame();
        }
    }
    assert_eq!(ja1front::play::soldier_tile(w, slot), tile + 3, "the merc walks 3 tiles");
    let ap1 = rec8(w, slot, 0x1A5);
    assert!(ap1 < ap0, "the walk spends AP ({} -> {})", ap0, ap1);
    w.update.new_round = 0;
    {
        let mut s = ja1front::play::Session::new(w, &mut host);
        assert!(s.end_turn());
        for _ in 0..3000 {
            s.tick();
            s.frame();
        }
    }
    assert_eq!(w.update.new_round, 1, "a new round starts");
    assert!(rec8(w, slot, 0x1A5) > ap1, "the new round gives the AP back");
}

#[test]
fn island_map_draws_cells() {
    use std::ffi::CString;
    let Some((iso, exe)) = rom() else { return };
    let (iso, exe) = (CString::new(iso).unwrap(), CString::new(exe).unwrap());
    let (sectors, marks) = ([0u8; 540], [0i8; 60]);
    let (mut screen, mut pal) = (vec![0u8; 64000], vec![0u8; 768]);
    let r = unsafe {
        ja1front::ja1f_island_map(iso.as_ptr(), exe.as_ptr(), sectors.as_ptr(), marks.as_ptr(), screen.as_mut_ptr(), pal.as_mut_ptr())
    };
    assert_eq!(r, 0);
    // Sector 0 cell: (6, 4), 30 x 22.
    let any = (0..22).any(|y| screen[(4 + y) * 320 + 6..(4 + y) * 320 + 36].iter().any(|&b| b != 0));
    assert!(any, "cell of sector 0 is empty");
    assert!(pal.iter().all(|&v| v < 64) && pal.iter().any(|&v| v != 0));
}

#[test]
fn map_sector_under_the_mouse() {
    use ja1front::ja1f_map_sector as f;
    assert_eq!(f(0, 0), 0);
    assert_eq!(f(6, 4), 0);
    assert_eq!(f(33, 4), 0);
    assert_eq!(f(33, 3), 0);
    assert_eq!(f(34, 3), 1);
    assert_eq!(f(100, 30), 13); // row 1, column 3
    assert_eq!(f(300, 140), 59); // row 5, column 9
    assert_eq!(f(10, 141), -1);
    assert_eq!(f(10, 200), -1);
    // x = 313..319 is column 10: the first sector of the next row; in the last row it caps at 59.
    assert_eq!(f(313, 4), 10);
    assert_eq!(f(319, 30), 20);
    assert_eq!(f(319, 140), 59);
}

#[test]
fn map_frame_writes_pixels() {
    use std::ffi::CString;
    let Some((iso, exe)) = rom() else { return };
    let (iso, exe) = (CString::new(iso).unwrap(), CString::new(exe).unwrap());
    let mut screen = vec![0u8; 64000];
    let r = unsafe { ja1front::ja1f_map_frame(iso.as_ptr(), exe.as_ptr(), 11, 0xFF, screen.as_mut_ptr()) };
    assert_eq!(r, 0);
    // Sector 11: x0 = 36, y0 = 26; the corner pixel is drawn, nothing else far away.
    assert!(screen.iter().any(|&b| b != 0));
    assert_eq!(screen[26 * 320 + 36], 0xFF);
    assert!(screen[..20 * 320].iter().all(|&b| b == 0));
    let r = unsafe { ja1front::ja1f_map_frame(iso.as_ptr(), exe.as_ptr(), 60, 0xFF, screen.as_mut_ptr()) };
    assert!(r < 0);
}

/// DayEndScreen 0x65BD4: ReplayScreenInit 0x64274 draws the replay screen with no test of the count, DayReplay 0x652B8 skips
/// its map at count 0, and DayEndVerdict 0x60680 draws the rating word with no test either. A day with no record (no merc
/// in the sector at entry and at sunset, no enemy move) still has the screen that the word goes on.
#[test]
fn day_with_no_replay_records_keeps_the_screen() {
    use ja1front::play::Play;
    let Some((iso, exe)) = rom() else { return };
    let save = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../build/DAY_day3.SAV");
    if !save.exists() {
        eprintln!("build/DAY_day3.SAV missing: skipped");
        return;
    }
    let mut e = ja1front::enter::enter(&iso, &exe, &save.to_string_lossy(), 1).unwrap();
    let mut play = Play::new(e.seed);
    play.host.log = false;
    play.set_exe(&exe).unwrap();
    play.files = Some((iso.clone(), exe.clone()));
    play.host.replay_events.clear();
    play.day_replay(&mut e.world);
    let start = play.host.replay_start.clone().expect("the start picture");
    let (shot, _) = play.host.replay_last.as_ref().expect("the verdict needs the screen at count 0");
    assert_eq!(shot, &start);
}

#[test]
fn abort_and_restart_the_day() {
    // docs/spec/world_load.md section 8 "Abort or restart the day" and the script line `abort R`.
    use ja1front::play::{parse_script, run_script, soldier_tile, Play};
    let Some((iso, exe)) = rom() else { return };
    let save = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../build/DAY_day3.SAV");
    if !save.exists() {
        eprintln!("build/DAY_day3.SAV missing: skipped");
        return;
    }
    let mut e = ja1front::enter::enter(&iso, &exe, &save.to_string_lossy(), 1).unwrap();
    let entry_tile = soldier_tile(&e.world, 1);
    let mut play = Play::new(e.seed);
    play.host.log = false;
    play.set_exe(&exe).unwrap();
    play.files = Some((iso.clone(), exe.clone()));
    play.day.tileset = e.tileset;
    play.day.day_save = Some(e.loaded.clone());
    let w = &mut e.world;
    let mut inv = |_: &mut ja1front::world::World, _: &mut Play, _: &[ja1front::invscreen::InvPoll]| {};
    // Merc 1 walks away (two clicks: the save has the confirm-move option on); the restart puts the world back to the day start and enters the sector again.
    run_script(w, &mut play, &parse_script("wait 20\nkey Alt+2\nclick 1145\nclick 1145\nwait 200").unwrap(), &mut inv).unwrap();
    assert_eq!(soldier_tile(w, 1), 1145);
    let passes = play.passes;
    run_script(w, &mut play, &parse_script("abort 1").unwrap(), &mut inv).unwrap();
    assert_eq!(play.passes, passes + 1, "the abort pass counts");
    assert!(!play.left);
    assert_eq!(soldier_tile(w, 1), entry_tile);
    assert_eq!(play.day.lines.last().unwrap(), &format!("abort 1 day {} sector {} events 0 -> 0", e.loaded.day, e.loaded.cur_sector));
    // No office copy yet: abort works as a restart.
    run_script(w, &mut play, &parse_script("abort 0").unwrap(), &mut inv).unwrap();
    assert!(!play.left);
    // With an office copy: the world becomes it, play stays stopped, the next `day` runs the sunrise only.
    let mut office = e.loaded.clone();
    office.day = 40;
    play.day.office = Some(office);
    run_script(w, &mut play, &parse_script("abort 0").unwrap(), &mut inv).unwrap();
    assert!(play.left);
    assert_eq!(w.day, 40);
    assert_eq!(play.day.lines.last().unwrap(), &format!("abort 0 day 40 sector {} events 0 -> 0", e.loaded.cur_sector));
    run_script(w, &mut play, &parse_script("wait 5\nday").unwrap(), &mut inv).unwrap();
    assert_eq!(play.day.lines.last().unwrap(), "day morning - day 41");
}

#[test]
fn map_panel_tabs_and_wage_box() {
    // docs/spec/strategic.md "The map panel": tab click, mouse sector number, the native wage box and its question.
    use ja1front::mapui::{Key, MapUi, WAGE_ASK};
    let Some((iso, exe)) = rom() else { return };
    let save = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../build/DAY_day3.SAV");
    if !save.exists() {
        eprintln!("build/DAY_day3.SAV missing: skipped");
        return;
    }
    let mut e = ja1front::enter::enter(&iso, &exe, &save.to_string_lossy(), 1).unwrap();
    let w = &mut e.world;
    let wage = |w: &ja1front::world::World| i32::from_le_bytes(w.accounts[0x54..0x58].try_into().unwrap());
    let mut m = MapUi::new(&iso, &exe, 1);
    m.show(w, (300, 190)).unwrap();
    let panel0 = m.img[146 * 320..].to_vec();
    assert!(panel0.iter().any(|&v| v != 0), "the panel shows");
    // Tab 2 (guards): map mode 2, the panel changes; tab 0 again gives the first picture back.
    m.track(w, (90, 160)).unwrap();
    m.click(w, 1).unwrap();
    assert_eq!(m.mode(), 2);
    assert_ne!(m.img[146 * 320..], panel0[..]);
    m.track(w, (30, 160)).unwrap();
    m.click(w, 1).unwrap();
    assert_eq!(m.mode(), 0);
    assert_eq!(m.img[146 * 320..], panel0[..]);
    // The mouse over sector 0: number field 15 shows "^1".
    m.track(w, (10, 10)).unwrap();
    assert_ne!(m.img[146 * 320..], panel0[..]);
    // The wage box: press, two right clicks, left click: the question; N puts the wage back, Y keeps it.
    let w0 = wage(w);
    for answer in [b'N', b'Y'] {
        m.track(w, (150, 165)).unwrap();
        m.click(w, 1).unwrap();
        m.click(w, 2).unwrap();
        m.click(w, 2).unwrap();
        assert_eq!(wage(w), (w0 + 2).min(100));
        m.click(w, 1).unwrap();
        if w0 < 100 {
            assert_eq!(m.key(w, b'T').unwrap(), Key::Nothing, "other keys wait for the answer");
            assert_eq!(m.key(w, answer).unwrap(), Key::Changed);
            if answer != b'N' {
                // panel.md "The travel animation": the yes row shows until it ends or a key or a click comes. The
                // button of the last click is still down, so KeyOrClick 0x67E44 waits for its release and reads no
                // key: the key stays queued and goes to the map after the release.
                assert_eq!(m.key(w, 27).unwrap(), Key::Nothing, "a key in the release wait ended the yes row");
                m.release(w, 3).unwrap();
                assert_eq!(m.take_kept(), vec![27], "the key in the release wait did not stay queued");
            }
        }
        assert_eq!(wage(w), if answer == b'N' { w0 } else { (w0 + 2).min(100) });
    }
    let _ = WAGE_ASK;
    // The same with the mouse on the question box's buttons (panel.md "Question box"): NO puts the wage back.
    let w1 = wage(w);
    if w1 < 100 {
        for (x, keep) in [(150, false), (215, true)] {
            m.track(w, (150, 165)).unwrap();
            m.click(w, 1).unwrap();
            m.click(w, 2).unwrap();
            m.click(w, 1).unwrap();
            assert!(m.asking());
            let before = m.img.clone();
            m.track(w, (x, 100)).unwrap();
            m.click(w, 1).unwrap();
            assert_ne!(m.img, before, "the pressed face shows");
            m.release(w, 1).unwrap();
            assert!(!m.asking());
            assert_eq!(wage(w), if keep { (w1 + 1).min(100) } else { w1 });
            if keep {
                break;
            }
        }
    }
    eprintln!("map panel test: wage {} -> {}", w0, wage(w));
}

#[test]
fn map_finance_page_and_plants() {
    // docs/spec/strategic.md "The finance page" and "The processing plants".
    use ja1front::mapui::{Key, MapUi};
    let Some((iso, exe)) = rom() else { return };
    let save = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../build/DAY_day3.SAV");
    if !save.exists() {
        eprintln!("build/DAY_day3.SAV missing: skipped");
        return;
    }
    let mut e = ja1front::enter::enter(&iso, &exe, &save.to_string_lossy(), 1).unwrap();
    let w = &mut e.world;
    let mut m = MapUi::new(&iso, &exe, 1);
    m.show(w, (300, 190)).unwrap();
    let pic0 = m.img.clone();
    // Tab 3 held: the plants show on the map (rows 0..142 change); up: the picture comes back.
    m.track(w, (200, 160)).unwrap();
    m.click(w, 1).unwrap();
    assert_ne!(m.img[..143 * 320], pic0[..143 * 320]);
    assert_eq!(m.key(w, b'G').unwrap(), Key::Nothing, "keys wait while the button is held");
    m.release(w, 1).unwrap();
    assert_eq!(m.img, pic0);
    // The field 4 box: pressed, then up: the old screen and three flip pictures queue, then the finance page.
    m.track(w, (240, 184)).unwrap();
    m.click(w, 1).unwrap();
    assert_ne!(m.img, pic0, "the box is pressed");
    m.release(w, 1).unwrap();
    assert!(m.finance);
    assert_eq!(m.anim.len(), 4);
    while !m.anim.is_empty() {
        assert_ne!(m.shown()[146 * 320..], m.img[146 * 320..]);
        m.tick();
    }
    let page = m.img.clone();
    assert_eq!(page[..146 * 320], pic0[..146 * 320], "the map stays");
    // Other clicks do nothing on the page; the back button closes it.
    m.track(w, (30, 160)).unwrap();
    m.click(w, 1).unwrap();
    m.release(w, 1).unwrap();
    assert!(m.finance && m.img == page);
    m.track(w, (80, 160)).unwrap();
    m.click(w, 1).unwrap();
    assert_ne!(m.img, page, "the back button is pressed");
    m.release(w, 1).unwrap();
    assert!(!m.finance);
    assert_eq!(m.anim.len(), 4);
    // The panel again; row 199 keeps the last row of MAPFLIP1.PCC.
    assert_eq!(m.img[146 * 320..199 * 320], pic0[146 * 320..199 * 320]);
    assert!(m.img[199 * 320..].iter().any(|&v| v != 0));
}

#[test]
fn payroll_face_fires_a_merc() {
    // docs/spec/strategic.md "The day-end screen" part 2: the faces of present mercs take the tooltip and the
    // "Fire NICK?" question; yes = Fire the merc (balloon at (11 + 37 k, 20)) and Close up the team, then the payroll
    // picture again with the team slot marks.
    use ja1front::mapui::MapUi;
    let Some((iso, exe)) = rom() else { return };
    let save = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../build/DAY_day3.SAV");
    if !save.exists() {
        eprintln!("build/DAY_day3.SAV missing: skipped");
        return;
    }
    let mut e = ja1front::enter::enter(&iso, &exe, &save.to_string_lossy(), 1).unwrap();
    let w = &mut e.world;
    let team = |w: &ja1front::world::World| (0..8).filter(|&k| w.soldiers[k * 0x200 + 0x182] != 0).count();
    let mut m = MapUi::new(&iso, &exe, 1);
    m.payroll_steps(w, None).unwrap();
    let pay = m.payroll.clone().unwrap();
    let mut img = pay.clone();
    let mut tip = ja1front::assign::Tooltip::default();
    m.payroll_tip(&mut img, &mut tip, Some(0)).unwrap();
    assert!(tip.n == 1 && img != pay);
    m.payroll_tip(&mut img, &mut tip, None).unwrap();
    assert!(tip.n == 0 && img == pay, "the tooltip goes: the screen comes back");
    let q = m.payroll_fire_imgs(w, 0, &pay).unwrap();
    assert_eq!(q.len(), 3);
    let (n0, p1) = (team(w), w.soldiers[0x200 + 0x179]);
    assert!(n0 >= 2 && w.soldiers[0x179] < 60, "DAY_day3 has a merc in slot 0");
    let said = m.payroll_fire(w, 0).expect("a fired merc says AIM quote 9");
    assert_eq!(&said[..3], &[0, 11, 20]);
    assert_eq!((team(w), w.soldiers[0x179]), (n0 - 1, p1), "the team closed up");
    let again = m.payroll_again(w).unwrap();
    assert_ne!(again[320..143 * 320], pay[320..143 * 320], "the faces are drawn again");
    assert_eq!(again[146 * 320..], pay[146 * 320..], "the panel numbers stay");
}

#[test]
fn fire_merc_from_the_duty_menu() {
    // docs/spec/strategic.md "Firing a merc": FIRE in the duty menu of slot 0 asks "Fire <nickname>?"; N keeps the
    // merc, Y fires it: the team closes up and its items go to the store.
    use ja1front::mapui::{Key, MapUi};
    let Some((iso, exe)) = rom() else { return };
    let save = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../build/DAY_day3.SAV");
    if !save.exists() {
        eprintln!("build/DAY_day3.SAV missing: skipped");
        return;
    }
    let mut e = ja1front::enter::enter(&iso, &exe, &save.to_string_lossy(), 1).unwrap();
    let w = &mut e.world;
    let team = |w: &ja1front::world::World| (0..8).filter(|&k| w.soldiers[k * 0x200 + 0x182] != 0).count();
    let mut m = MapUi::new(&iso, &exe, 1);
    m.show(w, (300, 190)).unwrap();
    // The field 0 box opens the assignment screen only on the day map (not the map view of the tactical screen).
    m.track(w, (30, 184)).unwrap();
    m.click(w, 1).unwrap();
    m.release(w, 1).unwrap();
    assert!(!m.assigning());
    m.day = true;
    m.click(w, 1).unwrap();
    m.release(w, 1).unwrap();
    assert!(m.assigning());
    let (n0, p0, items0) = (team(w), w.soldiers[0x179], w.stash_records.len());
    let mut before = ja1front::screen::Screen::new(&iso, &exe, w).unwrap();
    for answer in [b'N', b'Y'] {
        m.track(w, (20, 10)).unwrap();
        m.click(w, 1).unwrap();
        m.release(w, 1).unwrap();
        m.track(w, (20, 65)).unwrap();
        m.click(w, 1).unwrap();
        m.release(w, 1).unwrap();
        assert!(m.firing(), "FIRE asks first");
        assert_eq!(m.key(w, b'T').unwrap(), Key::Nothing, "other keys wait for the answer");
        assert_eq!(m.key(w, answer).unwrap(), Key::Changed);
        assert!(!m.firing() && m.assigning());
        if answer == b'N' {
            assert_eq!((team(w), w.soldiers[0x179]), (n0, p0), "N keeps the merc");
        }
    }
    assert_eq!(team(w), n0 - 1, "Y fires the merc: the team closes up");
    assert_ne!(w.soldiers[0x179], p0);
    if p0 < 60 {
        assert_eq!(w.profiles[p0 as usize * 0x5A + 0x3D], 1, "firedByYou");
    }
    // docs/spec/panel.md "The front-end's tactical screen": the empty place is not drawn (stays black).
    // docs/spec/panel.md "Face buffer": the place the team left (not in the sector) keeps a cleared face (the
    // original's face_clear), both in the screen of the sector entry and in the screen kept from before.
    let gone = n0 - 1;
    assert_eq!(w.soldiers[gone * 0x200 + 0x182], 0);
    before.update_faces(&iso, &exe, w).unwrap();
    let after = ja1front::screen::Screen::new(&iso, &exe, w).unwrap();
    // 0x46AC4 clears 0x380 bytes (smallrows_diff faces); rows 28..31 keep their old bytes, and the empty place is not drawn.
    assert!(before.faces[gone * 1024..gone * 1024 + 0x380].iter().all(|&b| b == 0), "no face in the empty place");
    // A sector entry draws frame picture 5 on all 8 buffers (0x2B127), the empty place too: only the frame is there.
    for r in 2..27 {
        assert!(after.faces[gone * 1024 + r * 32 + 2..gone * 1024 + r * 32 + 30].iter().all(|&b| b == 0), "no face in the empty place");
    }
    assert!(after.faces[gone * 1024..gone * 1024 + 32].iter().any(|&b| b != 0), "the frame of the empty place");
    for f in [&before.faces, &after.faces] {
        assert!(f[..1024].iter().any(|&b| b != 0), "slot 0 keeps its face");
    }
    eprintln!("fire test: profile {} fired, team {} -> {}, store {} -> {} records", p0, n0,
              team(w), items0, w.stash_records.len());
}

#[test]
fn guide_offer_asks_the_window() {
    // docs/spec/strategic.md "The guide's advice" 13: with 7 mercs, no guide and a long wait the guide offers a
    // native; the host's question (op 319, the window's merc-face box) answers, yes adds profile 60 to the team.
    use ja1front::play::Play;
    use std::sync::{Arc, Mutex};
    let Some((iso, exe)) = rom() else { return };
    let save = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../build/DAY_day3.SAV");
    if !save.exists() {
        eprintln!("build/DAY_day3.SAV missing: skipped");
        return;
    }
    let mut e = ja1front::enter::enter(&iso, &exe, &save.to_string_lossy(), 1).unwrap();
    let mut play = Play::new(e.seed);
    play.host.log = false;
    play.set_exe(&exe).unwrap();
    play.files = Some((iso.clone(), exe.clone()));
    play.day.tileset = e.tileset;
    play.day.day_save = Some(e.loaded.clone());
    let w = &mut e.world;
    assert!((0..8).all(|k| w.soldiers[k * 0x200 + 0x179] < 60), "no guide in the team");
    w.soldiers[7 * 0x200 + 0x182] = 0;
    w.guides_hired = 0;
    w.guide_wait = 20;
    // The question picture (JA1_GUIDE_PPM = a file name: the picture as PPM, to look at).
    let mut m = ja1front::mapui::MapUi::new(&iso, &exe, 1);
    m.show(w, (300, 190)).unwrap();
    let img = m.guide_offer_img(w, 60, Some(1)).unwrap();
    assert_ne!(img, m.img, "the box shows");
    if let Ok(f) = std::env::var("JA1_GUIDE_PPM") {
        let mut out = b"P6 320 200 255\n".to_vec();
        for &v in &img {
            out.extend(m.pal[3 * v as usize..3 * v as usize + 3].iter());
        }
        std::fs::write(f, out).unwrap();
    }
    let asked = Arc::new(Mutex::new(Vec::new()));
    let a2 = asked.clone();
    play.host.guide_ask = Some(Box::new(move |p| {
        a2.lock().unwrap().push(p);
        true
    }));
    w.sector_state[9 * 3 + 5] = 2;
    play.day_steps(w);
    assert_eq!(w.sector_state[9 * 3 + 5], 0, "guards still on the way are lost at the day end");
    assert_eq!(*asked.lock().unwrap(), vec![60], "the window is asked once, for profile 60");
    assert_eq!(w.guides_hired, 1);
    assert!((0..8).any(|k| w.soldiers[k * 0x200 + 0x179] == 60 && w.soldiers[k * 0x200 + 0x182] != 0),
            "the guide joins");
    // strategic.md "A merc joins the team": record 7 (the team size) gets the profile's name, health, pay and kit.
    let (r, pr) = (&w.soldiers[7 * 0x200..8 * 0x200], &w.profiles[60 * 0x5A..61 * 0x5A]);
    assert_eq!((r[0x179], r[0x17A], r[0x182], r[0x17E], r[0x18A]), (60, 7, 1, 0, 100));
    let n = pr.iter().position(|&c| c == 0).unwrap();
    assert_eq!(&r[0x16E..0x16E + n + 1], &pr[..n + 1], "nickname");
    assert_eq!((r[0x183], r[0x184], &r[0x110..0x112]), (pr[0x2A], pr[0x2A], &pr[0x28..0x2A]));
    let kit: Vec<u8> = pr[0x50..0x5A].iter().copied().filter(|&t| t != 0).collect();
    let got: Vec<u8> = (0..12).map(|i| r[12 * i]).filter(|&t| t != 0).collect();
    assert_eq!(got, kit, "the profile's kit");
}

#[test]
fn guard_screen_sends_a_guard() {
    // docs/spec/strategic.md "The guard screen" in the window's code: a right click takes a guard of sector 59, the
    // exit test with a guard in the hand gives message 20, a left click on sector 58 sends it, Enter closes and the
    // guard sets off (b5 of 58 + 1, guard_eta = 30 minutes from now).
    use ja1front::play::Play;
    let Some((iso, exe)) = rom() else { return };
    let save = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../build/DAY_day3.SAV");
    if !save.exists() {
        eprintln!("build/DAY_day3.SAV missing: skipped");
        return;
    }
    let mut e = ja1front::enter::enter(&iso, &exe, &save.to_string_lossy(), 1).unwrap();
    let mut play = Play::new(e.seed);
    play.host.log = false;
    let w = &mut e.world;
    assert_eq!(w.cur_sector, 59);
    let (g59, b58) = (w.sector_state[59 * 9 + 4], w.sector_state[58 * 9 + 5]);
    let mut m = ja1front::mapui::MapUi::new(&iso, &exe, 1);
    assert!(m.guard_show(w).unwrap() && m.guarding());
    assert_eq!(m.warp, Some((9 * 31 + 3, 5 * 23 + 3)), "the mouse goes to the cell of cur_sector");
    let shown = m.img.clone();
    m.track(w, (297, 129)).unwrap();
    m.click(w, 2).unwrap();
    assert_eq!(w.sector_state[59 * 9 + 4], g59 - 1, "the guard is in the hand");
    assert_ne!(m.img, shown);
    m.key(w, 13).unwrap();
    assert!(m.box_shown() && m.guard_take_closing().is_none(), "message 20: the screen stays");
    m.dismiss().unwrap();
    m.track(w, (266, 129)).unwrap();
    m.guard_pass(w).unwrap();
    m.guard_pass(w).unwrap();
    m.click(w, 1).unwrap();
    m.key(w, 13).unwrap();
    let mut g = m.guard_take_closing().expect("Enter closes");
    assert_eq!(g.moves_n[58], 1);
    assert!(!m.open && !m.guarding());
    let now = w.clock.minutes;
    play.guard_close(w, &mut g);
    assert_eq!(w.sector_state[58 * 9 + 5], b58 + 1, "the guard is on the way");
    assert_eq!(i16::from_le_bytes([w.clock.guard_eta[16 * 58], w.clock.guard_eta[16 * 58 + 1]]) as i32, 30 + now);
    assert_eq!(g.moves_n, [0; 60]);
}

/// panel.md "The mouse pointer": the five shapes come from the panel sheet; each draws some pixels and keeps its
/// hot spot pixel on the shape.
#[test]
fn pointer_shapes() {
    let Some((iso, exe)) = rom() else { return };
    let d = ja1front::panel::panel_data(&iso, &exe).unwrap();
    for s in [0, 200, 201, 202, 203] {
        let sh = ja1front::pointer::shape(&d, s).unwrap();
        let used: std::collections::BTreeSet<u8> = sh.pix.iter().copied().filter(|&p| p != 0).collect();
        eprintln!("shape {} {}x{} colours {:?}", s, sh.w, sh.h, used);
        for y in 0..sh.h {
            eprintln!("  {}", sh.pix[(y * sh.w) as usize..((y + 1) * sh.w) as usize].iter().map(|&p| if p == 0 { ".".to_string() } else { format!("{:x}", p % 16) }).collect::<String>());
        }
        assert!(!used.is_empty(), "shape {}", s);
    }
    assert!(ja1front::pointer::shape(&d, 5).is_none());
}


#[test]
fn weapon_text_stays_on_the_face() {
    // panel.md "Speech balloon" step 7: the wait text is centred on the face and only the 56 x 50 face changes.
    let Some((iso, exe)) = rom() else { return };
    let spot = ja1front::faceanim::FaceSpot { sheet: vec![0; 148 * 69], x: 20, y: 30, b: 0 };
    let mut buf = vec![7u8; 64000];
    ja1front::balloon::wait_text(&iso, &exe, &mut buf, &spot, b"ITEM BROKEN").unwrap();
    let changed: Vec<usize> = (0..64000).filter(|&i| buf[i] != 7).collect();
    assert!(!changed.is_empty());
    for i in changed {
        let (x, y) = ((i % 320) as i32, (i / 320) as i32);
        assert!((20..76).contains(&x) && (30..80).contains(&y), "pixel ({}, {}) outside the face", x, y);
    }
    assert!(spot.wait_rect_has(19, 29) && spot.wait_rect_has(75, 79) && !spot.wait_rect_has(76, 79));
}

#[test]
fn jammed_gun_report_carries_its_text() {
    // soldier_update.md "Merc sighting" step 2: a merc with a radio who reacts (how 0) with a jammed gun (ammo byte
    // -1) gets the merc message with the text JAMMED, shown in its speech balloon.
    let Some((iso, exe)) = rom() else { return };
    let save = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../build/DAY_fight48d.SAV");
    if !save.exists() {
        eprintln!("build/DAY_fight48d.SAV missing: skipped");
        return;
    }
    let mut e = ja1front::enter::enter(&iso, &exe, &save.to_string_lossy(), 1).unwrap();
    let mut host = ja1front::play::PlayHost::new(e.seed);
    host.log = false;
    let w = &mut e.world;
    w.soldiers[5 * 512 + 1] = 0xFF;
    host.sight = ja1front::sighting::Sight { slot: 5, how: 0, near: 0x40 };
    {
        let mut s = ja1front::play::Session::new(w, &mut host);
        s.loop_head();
    }
    let r = host.reactions.first().expect("no reaction");
    assert_eq!((r.slot, r.text.as_str(), r.balloon.is_some()), (5, "JAMMED", true));
}

#[test]
fn program_start_world() {
    // world_load.md section 12: Continue Game before any game = a new game (0, 0) with the A.I.M. link 0 and the
    // Subtitles option of SOUND.CFG (the GOG file holds 1).
    let Some((iso, exe)) = rom() else { return };
    let n = ja1front::world::World::new_game(&iso, &exe, 0, 0).unwrap();
    let w = ja1front::world::World::program_start(&iso, &exe).unwrap();
    assert_eq!(n.accounts[0xA3], 1);
    assert_eq!(w.accounts[0xA3], 0);
    assert_eq!(n.update.opt_step_text, 0);
    assert_eq!(w.update.opt_step_text, 1);
}
