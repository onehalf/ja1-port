//! savegame.md "Quick save": `QUICK.SAV` from the world ("Quick Save") and back ("Quick Restore").

use crate::world::{World, SAVE_FIXED_END};

/// The fixed part: 143 blocks, 85402 bytes.
/// QuickRestore 0x2D823 puts the mouse here, whatever the place before.
pub const RESTORE_MOUSE: (i32, i32) = (230, 110);
pub const FIXED: usize = 85402;
/// The question boxes of steps 1 (panel.md question box, picture `q_mark.pcc`).
pub const SAVE_ASK: (&[u8], &str) = (b"Save day in progress?", "Q_MARK.PCC");
pub const RESTORE_ASK: (&[u8], &str) = (b"Quick Restore?", "Q_MARK.PCC");
/// Step 2 of each: the message box with no wait (savegame.md "Quick Save and Quick Restore: the steps").
pub const SAVING: &[u8] = b"Saving...";
pub const RESTORING: &[u8] = b"Restoring...";
/// world_load.md section 12 "Continue last day in progress?": the two question boxes at the program start.
pub const CONTINUE_ASK: (&[u8], &str) = (b"Continue last day in progress?", "Q_MARK.PCC");
pub const DELETE_ASK: (&[u8], &str) = (b"Quick save about to be deleted!", "!MARK.PCC");
/// The files that the deletion removes (the original's DeleteDaySaves), in the save folder.
pub const DAY_FILES: [&str; 3] = ["OFFICE.SAV", "DAY.SAV", "QUICK.SAV"];

/// The palette of the start question's black screen: the palette of AIMSCRN.PCX (MAIN.DAT), 6-bit.
pub fn start_palette(iso: &str, exe: &str) -> Result<[u8; 768], i32> {
    let main = crate::invdraw::inv_data(iso, exe)?;
    let m = crate::panel::archive_member(&main.main, "AIMSCRN.PCX").map_err(|_| crate::E_FILE)?;
    let tail = m.get(m.len().saturating_sub(768)..).filter(|t| t.len() == 768).ok_or(crate::E_FILE)?;
    let mut pal = [0u8; 768];
    for (i, v) in tail.iter().enumerate() {
        pal[i] = v >> 2;
    }
    Ok(pal)
}
/// ContinueDayPrompt 0x4FF00: what the prompt asks of its screen. `ask` shows a question box and waits for the answer.
pub trait ContinueIo {
    fn fade(&mut self, pal: &[u8; 768], n: i32, from: i32, to: i32);
    /// FlushKeys 0x67F90: keys and clicks from before the box do not count.
    fn flush(&mut self);
    /// SetMouseShape 0 + ShowMouse.
    fn show_pointer(&mut self);
    fn ask(&mut self, q: (&[u8], &str)) -> bool;
    fn hide_pointer(&mut self);
    /// DeleteDaySaves 0x37FF0.
    fn delete_day_files(&mut self);
}

/// ContinueDayPrompt 0x4FF00 on a black screen with the colours `pal` (`start_palette`): a fade in, the question
/// "Continue last day in progress?"; no asks "Quick save about to be deleted!"; yes to that deletes the day files, no
/// asks both again. The last fade is a fade in again (0x4FFC8: levels 0 to 256, the asm's push 0x100), over the black
/// screen. True = the day goes on from QUICK.SAV.
pub fn continue_prompt<I: ContinueIo>(pal: &[u8; 768], io: &mut I) -> bool {
    io.fade(pal, 24, 0, 256);
    io.flush();
    io.show_pointer();
    let go_on = loop {
        if io.ask(CONTINUE_ASK) {
            break true;
        }
        if io.ask(DELETE_ASK) {
            io.delete_day_files();
            break false;
        }
    };
    io.hide_pointer();
    io.fade(pal, 24, 0, 256);
    go_on
}

/// The three screens of a question box of the prompt (no button, NO, OK pressed): the box over the black screen.
pub fn continue_box(iso: &str, exe: &str, q: (&[u8], &str)) -> Result<Vec<Vec<u8>>, i32> {
    crate::question::screens(iso, exe, q.0, Some(q.1), &[0u8; 64000])
}

/// DeleteDaySaves 0x37FF0: each day file of the folder `dir`, if it is there.
pub fn delete_day_files(dir: &std::path::Path) {
    for f in DAY_FILES {
        let _ = std::fs::remove_file(dir.join(f));
    }
}

/// The base of the soldier records' +0x90 value in the file: the anim-script table 0x106C18 at the place where DOS/4GW
/// loads the game (image address + 0x1F1000). QuickRestore 0x2C898 reads the word back as a pointer, so a save with
/// another base crashes the original (save_xcompat). The port itself reads the offset at +0x98.
pub const PTR_BASE: u32 = 0x106C18 + 0x1F1000;

trait Le: Sized {
    const N: usize;
    fn put(&self, o: &mut [u8]);
    fn get(i: &[u8]) -> Self;
}

macro_rules! le {
    ($($t:ty),*) => {$(
        impl Le for $t {
            const N: usize = std::mem::size_of::<$t>();
            fn put(&self, o: &mut [u8]) {
                o[..Self::N].copy_from_slice(&self.to_le_bytes());
            }
            fn get(i: &[u8]) -> Self {
                <$t>::from_le_bytes(i[..Self::N].try_into().unwrap())
            }
        }
    )*};
}
le!(u8, i8, u16, i16, u32, i32);

fn num<T: Le>(v: &mut T, b: &mut [u8], save: bool) {
    if save {
        v.put(b)
    } else {
        *v = T::get(b)
    }
}

fn bytes(v: &mut [u8], b: &mut [u8], save: bool) {
    let m = v.len().min(b.len());
    if save {
        b[..m].copy_from_slice(&v[..m])
    } else {
        v[..m].copy_from_slice(&b[..m])
    }
}

/// Block 5: the world fields that the day save does not list ("The blocks in the port's world"), at block offsets.
fn block5(w: &mut World, b: &mut [u8], save: bool) {
    num(&mut w.tileset, &mut b[152..], save);
    num(&mut w.cur_sector, &mut b[153..], save);
    num(&mut w.update.merc_tire, &mut b[185..], save);
    num(&mut w.update.new_round, &mut b[301..], save);
    num(&mut w.clock.night_warned, &mut b[307..], save);
    num(&mut w.clock.hours, &mut b[340..], save);
    num(&mut w.clock.minute, &mut b[341..], save);
    num(&mut w.clock.hour, &mut b[342..], save);
    num(&mut w.clock.minutes, &mut b[343..], save);
    num(&mut w.clock.bar, &mut b[347..], save);
    num(&mut w.update.alone_warned, &mut b[351..], save);
    num(&mut w.update.weather_tire, &mut b[352..], save);
    num(&mut w.idle_day, &mut b[362..], save);
    num(&mut w.update.squad_lost, &mut b[363..], save);
    num(&mut w.clock.hard_day, &mut b[364..], save);
}

/// msgtext-style test view for quickrestore_diff and loadgame_diff (#39): the fields of `w` by name, each with its bytes
/// as the original keeps them. The tests compare them with the original's memory at the field's address, so a field
/// that the reader puts in the wrong place shows up (a file round trip cannot see that: the writer undoes it).
/// `sees_text` as the 64 bytes of block 83.
fn sees_field(w: &World) -> [u8; 64] {
    let mut t = [0u8; 64];
    let n = w.sees_text.len().min(64);
    t[..n].copy_from_slice(&w.sees_text[..n]);
    t
}

pub fn field_table(w: &World) -> Vec<(&'static str, Vec<u8>)> {
    let mut out: Vec<(&'static str, Vec<u8>)> = Vec::new();
    macro_rules! sc {
        ($v:expr) => {{
            let mut b = vec![0u8; std::mem::size_of_val(&$v)];
            $v.put(&mut b);
            out.push((stringify!($v), b));
        }};
    }
    macro_rules! bf {
        ($v:expr) => {
            out.push((stringify!($v), $v[..].to_vec()))
        };
    }
    sc!(w.tileset);
    sc!(w.cur_sector);
    sc!(w.update.merc_tire);
    sc!(w.update.new_round);
    sc!(w.clock.night_warned);
    sc!(w.clock.hours);
    sc!(w.clock.minute);
    sc!(w.clock.hour);
    sc!(w.clock.minutes);
    sc!(w.clock.bar);
    sc!(w.update.alone_warned);
    sc!(w.update.weather_tire);
    sc!(w.idle_day);
    sc!(w.update.squad_lost);
    sc!(w.clock.hard_day);
    sc!(w.update.turn_timer);
    sc!(w.update.turn_timer2);
    sc!(w.santino_timer);
    sc!(w.show_ai);
    sc!(w.active_slot);
    sc!(w.anim_signal);
    sc!(w.update.in_flight);
    sc!(w.bomb_count);
    sc!(w.update.merc_to_show);
    sc!(w.update.interrupt_turn);
    sc!(w.update.handed_over);
    sc!(w.update.speech_wait);
    sc!(w.update.turn_moves);
    sc!(w.update.interrupt_go);
    sc!(w.update.itq_len);
    sc!(w.update.interrupter);
    out.push(("w.sight_queue", w.sight_queue.to_vec()));
    sc!(w.update.fx_id);
    sc!(w.busy);
    sc!(w.update.turn_flag);
    sc!(w.update.turn_clock);
    sc!(w.prev_balance);
    sc!(w.start_trees);
    sc!(w.start_workable);
    sc!(w.start_tapped);
    sc!(w.start_sectors);
    sc!(w.update.dead_mercs);
    sc!(w.update.dead_allies);
    sc!(w.update.dead_npc);
    sc!(w.update.enemy_deaths);
    sc!(w.start_cap);
    sc!(w.start_plants);
    sc!(w.update.time_compress);
    sc!(w.view_x);
    sc!(w.view_y);
    sc!(w.marker_tile);
    sc!(w.marker_kind);
    sc!(w.cursor_mode);
    sc!(w.cursor_value);
    sc!(w.music_wanted);
    sc!(w.selected);
    sc!(w.update.next_selected);
    sc!(w.update.kept_merc);
    sc!(w.opt_onscreen);
    sc!(w.update.spotted);
    sc!(w.update.view_tile);
    sc!(w.update.quote_slot);
    sc!(w.update.quote_event);
    sc!(w.update.quote_delay);
    sc!(w.update.quote_focus);
    sc!(w.update.quote_tile);
    sc!(w.update.show_timer);
    sc!(w.small_panel);
    sc!(w.full_shown);
    sc!(w.panel_ends);
    sc!(w.strip_flag);
    sc!(w.walter_sector);
    sc!(w.update.sees_slot);
    sc!(w.update.merc_shown);
    sc!(w.update.turn_shown);
    sc!(w.update.screen_drawn);
    sc!(w.update.view_hold);
    sc!(w.path_preview);
    sc!(w.update.marks_n);
    sc!(w.update.interrupted);
    sc!(w.update.speaker);
    sc!(w.bonus_due);
    sc!(w.bonus);
    sc!(w.duty7_merc);
    sc!(w.update.seen_enemies);
    sc!(w.inv.face_left);
    sc!(w.update.shown_slot);
    sc!(w.view_w);
    sc!(w.view_h);
    sc!(w.update.round_lock);
    sc!(w.update.story_finder);
    sc!(w.mouse_saved_x);
    sc!(w.mouse_saved_y);
    sc!(w.blaster);
    sc!(w.update.ambient_count);
    sc!(w.update.fuse_beep_timer);
    sc!(w.update.fuse_beep);
    sc!(w.mission_target);
    sc!(w.mission_slot);
    sc!(w.update.redraw_pending);
    sc!(w.combat);
    sc!(w.guard_skill);
    sc!(w.guides_hired);
    sc!(w.guide_wait);
    sc!(w.enemy_counter);
    sc!(w.santino_spoke);
    sc!(w.purifier);
    sc!(w.difficulty);
    sc!(w.alarm);
    sc!(w.mission_step);
    sc!(w.update.opt_reload_msg);
    sc!(w.last_spotter);
    sc!(w.safe_opened);
    sc!(w.update.opt_step_text);
    sc!(w.update.opt_half_delay);
    sc!(w.update.opt_follow);
    sc!(w.opt_sound);
    sc!(w.opt_confirm_move);
    sc!(w.update.opt_move_fx);
    sc!(w.update.opt_path_marks);
    sc!(w.update.stage);
    sc!(w.update.redraw_next);
    sc!(w.update.replay_m);
    sc!(w.update.first_hit);
    sc!(w.update.last_killer);
    sc!(w.update.opt_ambient);
    sc!(w.update.enemies_warned);
    sc!(w.quote_member);
    sc!(w.hinted);
    sc!(w.loss_slot);
    sc!(w.loss_timer);
    bf!(w.loss_part);
    bf!(w.loss_stat);
    bf!(w.soldiers);
    bf!(w.timed_fx);
    bf!(w.gas_clouds);
    bf!(w.blast_list);
    bf!(w.move_codes);
    bf!(w.clock.guard_eta);
    bf!(w.enemy_watch);
    bf!(w.guide_queue);
    bf!(w.tile_anim_block);
    out.push(("w.sees_text", sees_field(w).to_vec()));
    bf!(w.itq);
    bf!(w.remember_e);
    bf!(w.knowledge);
    bf!(w.remember_m);
    bf!(w.last_seen_m);
    bf!(w.last_seen_e);
    bf!(w.clock.guard_rec);
    bf!(w.fx_anims);
    bf!(w.sector_replay);
    bf!(w.paths);
    bf!(w.marks);
    bf!(w.calls);
    bf!(w.dmg_text);
    bf!(w.ground_items);
    bf!(w.tiles);
    {
        // The bytes of the block that other fields own: Save writes these fields over `accounts` (World::save_bytes).
        let mut a = w.accounts.clone();
        a[142] = w.update.opt_reload_msg; // 0x22A02E
        a[152] = w.tileset; // g_tileset 0x22A038
        a[12..16].copy_from_slice(&w.team_on_duty.to_le_bytes()); // g_teamOnDuty 0x229FAC: a death lowers only the field
        out.push(("w.accounts", a));
    }
    bf!(w.sector_state);
    bf!(w.sectors);
    sc!(w.day);
    sc!(w.team_on_duty);
    sc!(w.santino_word);
    bf!(w.story);
    bf!(w.crate_state);
    bf!(w.crate_shown);
    bf!(w.supplies);
    bf!(w.item_told);
    bf!(w.walter_said);
    bf!(w.walter_next);
    bf!(w.ap_warned);
    bf!(w.plants);
    bf!(w.sector_order);
    bf!(w.profiles);
    bf!(w.stat_progress);
    bf!(w.mission_done);
    sc!(w.update.weather_tire);
    bf!(w.sector_spoken);
    bf!(w.notes);
    bf!(w.last_quote);
    out
}

/// Blocks 9..143 (and the 48 soldier records of block 1) between the world and `f` (the fixed part).
fn walk(w: &mut World, f: &mut [u8], save: bool) {
    let mut at = 2usize;
    macro_rules! n {
        ($v:expr) => {{
            let k = std::mem::size_of_val(&$v);
            num(&mut $v, &mut f[at..at + k], save);
            at += k;
        }};
    }
    macro_rules! b {
        ($v:expr, $len:expr) => {{
            bytes(&mut $v, &mut f[at..at + $len], save);
            at += $len;
        }};
    }
    macro_rules! skip {
        ($len:expr) => {
            at += $len
        };
    }
    b!(w.soldiers[..], 24576); // 1
    skip!(5400 + 540 + 10620 + 854 + 20 + 61 + 30); // 2..8: the day-save blocks
    skip!(32); // 9
    debug_assert_eq!(at, 0xA497);
    n!(w.update.turn_timer); // 10
    if !save {
        w.update.turn_timer2 = i16::from_le_bytes([f[at], f[at + 1]]);
    }
    n!(w.santino_timer); // 11
    n!(w.show_ai);
    n!(w.active_slot);
    n!(w.anim_signal);
    n!(w.update.in_flight); // 15
    n!(w.bomb_count);
    b!(w.timed_fx[..], 65);
    b!(w.gas_clouds[..], 30);
    n!(w.update.merc_to_show);
    n!(w.update.interrupt_turn); // 20
    n!(w.update.handed_over);
    n!(w.update.speech_wait);
    n!(w.update.turn_moves);
    n!(w.update.interrupt_go);
    n!(w.update.itq_len); // 25
    n!(w.update.interrupter);
    debug_assert_eq!(at, 0xA50D);
    b!(w.blast_list[..], 60);
    b!(w.move_codes[..], 6272);
    b!(w.sight_queue[..], 3); // 29: 0x229E00, the queued sighting reaction
    n!(w.update.fx_id); // 29, byte 3
    n!(w.busy); // 30
    n!(w.update.turn_flag);
    n!(w.update.turn_clock);
    b!(w.clock.guard_eta[..], 960); // 33
    // Block 33 bytes 920..959 are also enemy_watch bytes 0..39 (one variable in the original); the port keeps two
    // fields. The original writes enemy_watch only for slots 0x28..0x2F (bytes 40..47), so those bytes are always
    // the guard_eta entries of sectors 57..59: the write takes guard_eta, the restore sets both. (#20 I: the write
    // took enemy_watch, all 0, so after a restore guards "arrived" in sector 58 from nowhere.)
    if !save {
        let e = &mut f[at - 40..at];
        bytes(&mut w.enemy_watch[..40], e, save);
    }
    {
        // 34: 0x229D8C, guard counts of the last guard screen (crate::island::guard_marks).
        let e = &mut f[at..at + 60];
        if save {
            for (d, v) in e.iter_mut().zip(crate::island::guard_marks()) {
                *d = v as u8;
            }
        } else {
            let mut m = [0i8; 60];
            for (d, v) in m.iter_mut().zip(e.iter()) {
                *d = *v as i8;
            }
            crate::island::set_guard_marks(m);
        }
    }
    skip!(60 + 480); // 34, 35
    if save {
        // 36: the replay log's record count (the records go to the tail).
        f[at..at + 2].copy_from_slice(&(w.replay_log.len().min(0xFFFF) as u16).to_le_bytes());
    }
    skip!(2);
    debug_assert_eq!(at, 0xC3B4);
    let b37 = at; // 37
    {
        let b = &mut f[b37..b37 + 60];
        num(&mut w.prev_balance, &mut b[0..], save);
        num(&mut w.start_trees, &mut b[4..], save);
        num(&mut w.start_workable, &mut b[12..], save);
        num(&mut w.start_tapped, &mut b[20..], save);
        num(&mut w.start_sectors, &mut b[24..], save);
        num(&mut w.update.dead_mercs, &mut b[32..], save);
        num(&mut w.update.dead_allies, &mut b[36..], save);
        num(&mut w.update.dead_npc, &mut b[40..], save);
        num(&mut w.update.enemy_deaths, &mut b[48..], save);
        num(&mut w.start_cap, &mut b[52..], save);
        num(&mut w.start_plants, &mut b[56..], save);
    }
    skip!(60);
    n!(w.update.time_compress); // 38
    n!(w.view_x);
    n!(w.view_y); // 40
    n!(w.marker_tile); // 41
    n!(w.marker_kind);
    debug_assert_eq!(at, 0xC401);
    n!(w.selected);
    n!(w.update.next_selected);
    n!(w.update.kept_merc); // 45
    n!(w.opt_onscreen);
    b!(w.guide_queue[..], 7);
    skip!(4); // 48
    n!(w.update.spotted);
    n!(w.update.view_tile); // 50
    n!(w.update.quote_slot);
    n!(w.update.quote_event);
    n!(w.update.quote_delay);
    n!(w.update.quote_focus);
    n!(w.update.quote_tile); // 55
    n!(w.update.show_timer);
    skip!(1); // 57
    debug_assert_eq!(at, 0xC423);
    b!(w.itq[..], 50);
    b!(w.remember_e[..], 320); // 59
    if !save {
        let r = w.remember_e[..16].to_vec();
        w.knowledge[48..64].copy_from_slice(&r);
    }
    b!(w.remember_m[..], 128); // 60
    b!(w.knowledge[..48], 48);
    b!(w.last_seen_m[..], 256);
    b!(w.last_seen_e[..], 640);
    n!(w.cursor_mode); // 64
    n!(w.cursor_value); // 65
    skip!(1); // 66: fixed_state
    let theme = at; // 67: the playing theme (written / read by the caller)
    skip!(2); // 67
    n!(w.music_wanted); // 68
    b!(w.enemy_watch[40..48], 8); // 69
    skip!(2 + 1); // 70, 71
    n!(w.small_panel); // 72
    n!(w.full_shown);
    n!(w.panel_ends);
    n!(w.strip_flag); // 75
    debug_assert_eq!(at, 0xC9DE);
    b!(w.clock.guard_rec[..], 480); // 76
    n!(w.walter_sector);
    b!(w.tile_anim_block[..], 110); // 78
    b!(w.fx_anims[..], 288);
    b!(w.sector_replay[..], 540); // 80
    b!(w.paths[..], 3840);
    n!(w.update.sees_slot);
    // 83: the "sees enemies" text 0x10DA50: the line of sees_slot shows it after a restore (TacticalRedraw 0x177E1).
    if save {
        f[at..at + 64].copy_from_slice(&sees_field(w));
    } else {
        let t = &f[at..at + 64];
        w.sees_text = t[..t.iter().position(|&c| c == 0).unwrap_or(64)].to_vec();
    }
    at += 64;
    n!(w.update.merc_shown);
    n!(w.update.turn_shown); // 85
    n!(w.update.screen_drawn);
    n!(w.update.view_hold);
    n!(w.path_preview);
    debug_assert_eq!(at, 0xDEB2);
    b!(w.marks[..], 256); // 89: bytes 0..255 of 512
    skip!(256);
    n!(w.update.marks_n); // 90
    n!(w.update.interrupted);
    skip!(1); // 92
    n!(w.update.speaker);
    n!(w.bonus_due);
    n!(w.bonus); // 95
    n!(w.duty7_merc);
    {
        // 97: the roofs flag (tactical_map.md "Drawing order"), a program-run value outside the world.
        let mut roofs = crate::debugkeys::roofs();
        n!(roofs);
        crate::debugkeys::set_roofs(roofs);
    }
    n!(w.update.seen_enemies);
    skip!(1); // 99
    debug_assert_eq!(at, 0xE0C1);
    n!(w.inv.face_left); // 100
    skip!(4); // 101
    n!(w.update.shown_slot);
    n!(w.view_w);
    n!(w.view_h);
    skip!(4 * 8 + 1); // 105..113
    n!(w.update.round_lock);
    skip!(1); // 115
    n!(w.update.story_finder);
    skip!(1); // 117
    n!(w.mouse_saved_x); // 118: 0x229C40 (MouseSavePos2? 0x67E18)
    n!(w.mouse_saved_y); // 119
    n!(w.blaster); // 120
    b!(w.loss_part[..], 16); // 121
    b!(w.loss_stat[..], 16); // 122
    debug_assert_eq!(at, 0xE120);
    b!(w.calls[7..8], 1); // 123
    n!(w.update.ambient_count);
    b!(w.calls[0..3], 3); // 125, 126
    b!(w.calls[4..7], 3); // 127, 128
    b!(w.dmg_text[..], 240);
    n!(w.update.fuse_beep_timer); // 130
    n!(w.update.fuse_beep);
    n!(w.mission_target);
    n!(w.mission_slot);
    skip!(1); // 134
    n!(w.update.redraw_pending); // 135
    n!(w.combat);
    n!(w.loss_timer); // 137: g_lossTimer 0x92DE8
    {
        // 138..140: the volume numbers 0x963A0 / 0x963A4 (always 127) / 0x963A8, program-run values outside the
        // world. Quick Restore reads them back without sending them to AIL (sound::restore_volumes; port fix P09
        // sends them).
        let mut v = crate::sound::volumes();
        let mut fixed = 127i32;
        n!(v.music);
        n!(fixed);
        n!(v.sound);
        if !save {
            crate::sound::restore_volumes(v);
        }
    }
    skip!(4); // 141
    debug_assert_eq!(at, 0xE23A);
    b!(w.ground_items[..], 2400);
    b!(w.tiles[..], 25088); // 143
    debug_assert_eq!(at, FIXED);
    debug_assert_eq!(theme, 0x0C9C8);
    let _ = theme;
}

/// The day-save blocks inside the fixed part: (quick-save offset, day-save offset, size).
const DAY_BLOCKS: [(usize, usize, usize); 8] = [
    (0x00002, 28, 4096),
    (0x06002, 4124, 5400),
    (0x0751A, 9524, 540),
    (0x07736, 10064, 10620),
    (0x0A0B2, 20684, 854),
    (0x0A408, 21538, 20),
    (0x0A41C, 21558, 61),
    (0x0A459, 21619, 30),
];
/// Block 67 (the playing theme) and the soldiers' pointer bytes.
const THEME: usize = 0x0C9C8;

/// Quick Save step 4 / Quick Restore step 4, for the records whose +0x182 and +0x138 are both not 0. The port keeps
/// +0x90 as the byte offset into `anim_scripts` (port_api.md); the original keeps a pointer into that table (at
/// 0x106C18) there and the offset at +0x98 in the file. Write (`save`): +0x98 = the offset, +0x90 = 0x106C18 + it;
/// restore: +0x90 = +0x98.
fn pointers(soldiers: &mut [u8], save: bool) {
    for r in soldiers.chunks_exact_mut(512).take(48) {
        if r[0x182] != 0 && r[0x138] != 0 {
            if save {
                let v = u32::from_le_bytes(r[0x90..0x94].try_into().unwrap());
                r[0x98..0x9C].copy_from_slice(&v.to_le_bytes());
                r[0x90..0x94].copy_from_slice(&v.wrapping_add(PTR_BASE).to_le_bytes());
            } else {
                let v = u32::from_le_bytes(r[0x98..0x9C].try_into().unwrap());
                r[0x90..0x94].copy_from_slice(&v.to_le_bytes());
            }
        }
    }
}

/// `field_table` as bytes (validate/diff/world_fields.py `parse`).
pub fn field_bytes(w: &World) -> Vec<u8> {
    let mut o = Vec::new();
    for (name, b) in field_table(w) {
        o.push(name.len() as u8);
        o.extend_from_slice(name.as_bytes());
        o.extend_from_slice(&(b.len() as u32).to_le_bytes());
        o.extend_from_slice(&b);
    }
    o
}

/// The blocks that the port's world has no field for, but that QuickRestore 0x2C898 takes as they are: the original
/// has no other step that sets them after a load, so a 0 here gives it an empty view window (black map, then a crash;
/// save_xcompat). The view window is SetViewWindow 0x24880 from the world's view size; the others have one value in all
/// the original's quick saves that we hold (InitProgramWorld? 0x377B0 sets 0x229CD4 and 0x229CCC).
pub(crate) fn fixed_state(w: &World, f: &mut [u8]) {
    let (vw, vh) = (w.view_w, w.view_h);
    let left = if vw < 320 { 48 } else { 0 };
    let top = if w.opt_onscreen == 3 { 0 } else { 14 };
    let window = [0x100, 0xBA, vw / 16, vh / 14, left, top, left - 1 + vw, top - 1 + vh];
    for (i, v) in window.iter().enumerate() {
        f[0xE0D2 + 4 * i..0xE0D6 + 4 * i].copy_from_slice(&(*v as i32).to_le_bytes());
    }
    // 0x229CA4: TacticalRedraw 0x176FA keeps the view's first tile, ViewTile 0x46F3C.
    // ViewTile 0x46F62..0x46F6A multiplies by 56 with shl/sub/shl in 32 bits: it wraps (sticky #1 c12726).
    let first = (w.view_y / 14).wrapping_mul(56).wrapping_add(w.view_x / 16);
    f[0xE0C2..0xE0C6].copy_from_slice(&first.to_le_bytes());
    f[0xC9C7] = 0x4F; // 0x92DD4: only QuickRestore writes it, so it keeps the 0x4F of JA.EXE's data
    f[0xE0F2] = 0x40; // 0x284F0E: the original's restore shows no lit face from this byte (live, issue #31)
    f[0xE0B7] = 0x1C; // 0x284F08
    f[0xE0F6] = 0; // 0x92DE4
    f[0xE220] = 1; // 0x92DB9
    f[0xE236..0xE23A].copy_from_slice(&0x5Ai32.to_le_bytes()); // 0x9639C
}

/// Quick Save 0x2D880 steps 3..5: the bytes of `QUICK.SAV` (`theme` = the playing theme, -1 = none).
/// diff-tested (quicksave_diff): the original writes them on 5 dumps in Unicorn, ja1f_quick_rewrite restores that
/// file on a day-save world and writes it again with every modelled byte equal.
pub fn write(w: &World, theme: i16) -> Vec<u8> {
    write_fill(w, theme, 0)
}

/// `write` with the unmodelled bytes = `fill`.
pub(crate) fn write_fill(w: &World, theme: i16, fill: u8) -> Vec<u8> {
    let mut c = w.clone();
    // Step 3: the current sector's file from the tiles and the ground items.
    if let Some(slot) = c.tmp_files.get_mut(c.cur_sector as usize) {
        *slot = Some(crate::host::make_tmp_file(&w.tiles, &w.ground_items));
    }
    pointers(&mut c.soldiers, true);
    let s = c.save_bytes();
    let mut f = vec![fill; FIXED];
    f[0..2].copy_from_slice(&(c.day as u16).to_le_bytes());
    for &(q, d, n) in &DAY_BLOCKS[1..] {
        f[q..q + n].copy_from_slice(&s[d..d + n]);
    }
    block5(&mut c, &mut f[0x0A0B2..0x0A0B2 + 854], true);
    walk(&mut c, &mut f, true);
    f[THEME..THEME + 2].copy_from_slice(&theme.to_le_bytes());
    fixed_state(w, &mut f);
    // The tail: the sector files and the item stash as in the day save, then the replay log.
    f.extend_from_slice(&s[SAVE_FIXED_END..]);
    for r in w.replay_log.iter().take(0xFFFF) {
        f.extend_from_slice(r);
    }
    f
}

/// What Quick Restore 0x2C898 does after the read (0x2D80B..0x2D872), from the saved playing theme of block 67:
/// over_map 0x284F13 = 1 (BH = 1 from 0x2D80B), the playing theme 0x9635E = -1, then SetMusic(theme, 0) when the
/// theme is above 0. Returns (over_map, playing theme, the theme SetMusic starts or None).
pub fn after_restore(theme: i16) -> (u8, i16, Option<i16>) {
    (1, -1, (theme > 0).then_some(theme))
}

/// Quick Restore 0x2C898 steps 4 and 5 on the world (the other fields keep their values). Returns the saved playing theme
/// (block 67) for step 7. `jagged` = JAGGED.DAT (the sector reset).
pub fn restore(w: &mut World, jagged: &[u8], q: &[u8]) -> Result<i16, String> {
    if q.len() < FIXED {
        return Err(format!("QUICK.SAV: {} bytes, the fixed part needs {}", q.len(), FIXED));
    }
    // The day-save part through the day-save loader: a day-save image of blocks 0, 1 (slots 0..7), 2..8 and the tail.
    let mut d = w.base.clone();
    d.resize(SAVE_FIXED_END, 0);
    d[0..2].copy_from_slice(&q[0..2]);
    for &(o, s, n) in &DAY_BLOCKS {
        d[s..s + n].copy_from_slice(&q[o..o + n]);
    }
    d.extend_from_slice(&q[FIXED..]);
    crate::world::init_temp_files(&mut w.tmp_files, &mut w.replay_log);
    w.load_save(jagged, &d).map_err(|e| format!("QUICK.SAV: {}", e))?;
    let mut f = q[..FIXED].to_vec();
    block5(w, &mut f[0x0A0B2..0x0A0B2 + 854], false);
    walk(w, &mut f, false);
    pointers(&mut w.soldiers, false);
    // The replay log: block 36 records of 3 bytes at the end of the tail.
    let n = u16::from_le_bytes([q[0xC3B2], q[0xC3B3]]) as usize;
    let tail = &q[FIXED..];
    if 3 * n > tail.len() {
        return Err(format!("QUICK.SAV: {} replay records, the tail has {} bytes", n, tail.len()));
    }
    w.replay_log = tail[tail.len() - 3 * n..].chunks(3).map(|c| [c[0], c[1], c[2]]).collect();
    Ok(i16::from_le_bytes([q[THEME], q[THEME + 1]]))
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Restore each of the original's quick saves on a loaded world, write it again: every byte of the fixed part
    /// that a world field holds comes back, and the tail is the same.
    #[test]
    fn original_files_round_trip() {
        let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
        let (iso, exe) = (root.join("rom/JA1.gog"), root.join("rom/JA.EXE"));
        let day = root.join("build/DAY_day3.SAV");
        if !iso.exists() || !day.exists() {
            return;
        }
        let jagged = crate::iso::Iso::open(iso.to_str().unwrap()).unwrap().read("DAT/JAGGED.DAT").unwrap();
        let mut n = 0;
        for name in ["QUICK_t10.SAV", "QUICK_t2.SAV", "QUICK_s59start.SAV"] {
            let Ok(q) = std::fs::read(root.join("build").join(name)) else { continue };
            let mut w = World::load(iso.to_str().unwrap(), exe.to_str().unwrap(), day.to_str().unwrap()).unwrap();
            let theme = restore(&mut w, &jagged, &q).unwrap();
            let out = write(&w, theme);
            // a second round gives the same bytes
            let mut w2 = w.clone();
            restore(&mut w2, &jagged, &out).unwrap();
            assert!(write(&w2, theme) == out, "{}: second round differs", name);
            // the modelled bytes: every byte where the port file is not 0 equals the original's, and the soldiers,
            // tiles and ground items are the same
            // (+0x90 of a moving soldier is a pointer in the original: its value depends on where the program loaded)
            let ptr = |i: usize| (2..2 + 24576).contains(&i) && (0x90..0x94).contains(&((i - 2) % 512))
                && q[2 + (i - 2) / 512 * 512 + 0x182] != 0 && q[2 + (i - 2) / 512 * 512 + 0x138] != 0;
            // the modelled bytes (the same with two fills) are the original's
            let (a, b) = (write_fill(&w, theme, 0xAA), write_fill(&w, theme, 0x55));
            let modelled = (0..FIXED).filter(|&i| a[i] == b[i]).count();
            assert!(modelled > 84000, "{}: only {} modelled bytes", name, modelled);
            for i in 0..FIXED {
                if a[i] == b[i] && !ptr(i) {
                    assert_eq!(out[i], q[i], "{}: fixed byte {:#x}", name, i);
                }
            }
            assert!((2..2 + 24576).all(|i| ptr(i) || out[i] == q[i]), "{}: soldiers", name);
            assert!(out[0x0E23A..FIXED] == q[0x0E23A..FIXED], "{}: ground items + tiles", name);
            // the tail: the sector files, the stash and the replay log as in the original
            assert!(out[FIXED..] == q[FIXED..], "{}: tail", name);
            n += 1;
        }
        assert!(n > 0);
    }

    /// The replay record count at 0xC3B2: records may fill the whole tail; one more record than the tail holds is
    /// an error.
    #[test]
    fn replay_count_bound() {
        let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
        let (iso, exe) = (root.join("rom/JA1.gog"), root.join("rom/JA.EXE"));
        let day = root.join("build/DAY_day3.SAV");
        let Ok(mut q) = std::fs::read(root.join("build/QUICK_t10.SAV")) else { return };
        if !iso.exists() || !day.exists() {
            return;
        }
        let jagged = crate::iso::Iso::open(iso.to_str().unwrap()).unwrap().read("DAT/JAGGED.DAT").unwrap();
        while (q.len() - FIXED) % 3 != 0 {
            q.push(0);
        }
        let all = (q.len() - FIXED) / 3;
        let mut try_n = |n: usize| {
            q[0xC3B2..0xC3B4].copy_from_slice(&(n as u16).to_le_bytes());
            let mut w = World::load(iso.to_str().unwrap(), exe.to_str().unwrap(), day.to_str().unwrap()).unwrap();
            restore(&mut w, &jagged, &q).map(|_| w.replay_log.len())
        };
        assert_eq!(try_n(all), Ok(all));
        assert!(try_n(all + 1).is_err());
    }
}
