//! The day loop (docs/spec/world_load.md section 9): after the sector was left, the morning, the sunrise, the
//! island map with its start checks and the day start, then the sector entry of the new day.
use crate::play::{Play, Session};
use crate::world::World;

/// The day loop's state (kept by `Play`).
#[derive(Default)]
pub struct DayLoop {
    /// playloop_diff: the steps of PlayLoop 0x4FB94 in the order they ran (`ja1f_playloop_run`); None = no trace.
    pub trace: Option<Vec<String>>,
    /// The island map of the day loop is open (steps 1..3 ran, the day did not start yet).
    pub map_open: bool,
    /// The map's process warning flag: 0 at each map opening.
    pub warned: u8,
    /// The message number on the status line (0 = none).
    pub msg: i32,
    /// The text of the last op 305 merc_gone of the strategic screen opening (shown when the map opens).
    pub gone_text: Option<String>,
    /// The copy of the whole world at the day start (the original's DAY.SAV; before the first day start of a run:
    /// the world as loaded, set by the caller), for "Restart this day".
    pub day_save: Option<Box<World>>,
    /// The copy of the whole world after the office (step 2, at Sleep: the original's OFFICE.SAV), for "Abort this
    /// day".
    pub office: Option<Box<World>>,
    /// After an abort: the next steps 1..3 start at the office (no verdict, no morning).
    pub from_office: bool,
    /// The office ended with Leave (world_load.md section 12): the day loop stops, the main menu opens.
    pub to_menu: bool,
    /// The tileset of the sector entered last (drawing only).
    pub tileset: u8,
    /// The day's verdict of the last steps 1..3 (strategic.md The day's verdict): its result (0 = the game goes on;
    /// 2, 3, 4 = the game ends) and the score of op 331 rating_word (None when it did not come or did not run).
    pub verdict: Option<i32>,
    pub rating: Option<i32>,
    /// The game ended (a verdict result 2, 3 or 4): the map does not open.
    pub game_over: Option<i32>,
    /// Script mode after a game end (ending.md "Game end"): the last screen of the end show and the game-over
    /// screen, each with its palette (8-bit), for JA1_END_PNG / JA1_GAMEOVER_PNG.
    pub end_show: Option<(Vec<u8>, [u8; 768])>,
    pub over_screen: Option<(Vec<u8>, [u8; 768])>,
    /// After a win: the rating text the program ends with (ending.md "Game end", code 1).
    pub exit_text: Option<String>,
    /// The directory of the day saves OFFICE.SAV / DAY.SAV (section 10); None = none are written.
    pub save_dir: Option<std::path::PathBuf>,
    /// The stdout lines of the play script's day lines, in order.
    pub lines: Vec<String>,
}

impl DayLoop {
    /// A trace line `WHAT DAY MARK`; MARK = the last byte of profile 59 (no day step writes it, so it tells which world copy this is).
    pub fn trace_at(&mut self, what: &str, w: &World) {
        if let Some(t) = self.trace.as_mut() {
            t.push(format!("{} {} {}", what, w.day, w.profiles.get(59 * 0x5A + 0x59).copied().unwrap_or(0)));
        }
    }

    /// A trace line with no world numbers.
    pub fn trace_line(&mut self, what: String) {
        if let Some(t) = self.trace.as_mut() {
            t.push(what);
        }
    }

    /// SaveGame 0x4EBC8 uses OpenMiscFile 0x38370; the port writes directly to its save directory.
    /// Section 10: writes the day save of `w` as `name` in `save_dir` (nothing when there is no directory). Step 2
    /// (`office`) first deletes OFFICE.SAV, DAY.SAV and QUICK.SAV. An error: one stderr line, the loop goes on.
    pub fn write_save(&mut self, name: &str, w: &World, office: bool) {
        if office {
            self.trace_line("delete".into());
        }
        self.trace_at(&format!("save {}", name), w);
        let Some(dir) = self.save_dir.as_ref() else { return };
        if office {
            for n in ["OFFICE.SAV", "DAY.SAV", "QUICK.SAV"] {
                let p = dir.join(n);
                // SPEC GAP: a file that is not there is no error; any other delete error prints the same line.
                if let Err(e) = std::fs::remove_file(&p) {
                    if e.kind() != std::io::ErrorKind::NotFound {
                        eprintln!("save: {}: {}", p.display(), e);
                    }
                }
            }
        }
        let p = dir.join(name);
        if let Err(e) = std::fs::write(&p, w.save_bytes()) {
            eprintln!("save: {}: {}", p.display(), e);
        }
    }
}

/// Op 292 corpse_ask (strategic.md "Corpse export" step 2): "Dish out $AMOUN for corpse export?" with the cost
/// for AMOUN. The original asks in ConfirmDialog with the travel animation (panel.md "The travel animation",
/// `corpse_rows`); with can_pay = 0 the box is the same, only the yes row differs and the answer is ignored.
pub fn corpse_text(cost: i32) -> Vec<u8> {
    format!("Dish out ${} for corpse export?", cost).into_bytes()
}

/// panel.md "The travel animation", Callers: the corpse export question's archive.
pub const CORPSE_ARC: &str = "SANIM.DAT";

/// The corpse export question's rows (wait, yes, no): wait 8, yes 9 ("It'll be a closed coffin for sure!") or 11
/// with can_pay = 0 ("Sorry, you don't have the funds"), no 10 ("OK...").
pub fn corpse_rows(can_pay: bool) -> (i32, i32, i32) {
    (8, if can_pay { 9 } else { 11 }, 10)
}

/// The question box of op 303 day_ask (panel.md "Question box"): text and picture. Kind 0 = "Start day without
/// anyone assigned to duty?" with Q_MARK.PCC; kind 1 = "Get to work?" (its picture frame shows the travel animation,
/// panel.md "The travel animation", `sanim`).
pub fn ask_question(kind: i32) -> (&'static [u8], Option<&'static str>) {
    if kind == 1 {
        (b"Get to work?", None)
    } else {
        (b"Start day without anyone assigned to duty?", Some("Q_MARK.PCC"))
    }
}

/// The rating word of op 331 for the score −3..3 (strategic.md The day's verdict step 5).
/// strategic.md "The day-end screen": OK button 15 (replay screen) or 16 (payroll screen): its place and hit area
/// (x, y) .. (x + 25, y + 15).
pub fn dayend_ok_at(b: u8) -> (i32, i32) {
    crate::dayend::ok_at(b)
}

/// The face of OK button `b` on screen `s`: up = element 0x66 + overlay 0x3A at (5, 4), pressed = 0x65 + 0x3A at
/// (4, 3).
pub fn dayend_ok_face(s: &mut [u8], iso: &str, exe: &str, b: u8, pressed: bool) {
    let (Ok(c), Ok(d)) = (crate::common(iso), crate::panel::panel_data(iso, exe)) else { return };
    let mut table0 = c.remap[0];
    let mut p = crate::panel::Painter { d: &d, buf: s, table0: &mut table0, table29: crate::panel::table29(&c) };
    for (e, x, y) in dayend_ok_parts(b, pressed) {
        p.element(e, x, y, None);
    }
}

/// The two panel elements of `dayend_ok_face` with their places: the button, then the OK overlay.
pub fn dayend_ok_parts(b: u8, pressed: bool) -> [(usize, i32, i32); 2] {
    let (x, y) = dayend_ok_at(b);
    if pressed {
        [(0x65, x, y), (0x3A, x + 4, y + 3)]
    } else {
        [(0x66, x, y), (0x3A, x + 5, y + 4)]
    }
}

pub fn rating_word(score: i32) -> &'static str {
    const W: [&str; 7] = ["TERRIBLE", "LOUSY", "POOR", "FAIR", "GOOD", "EXCELLENT", "OUTSTANDING"];
    W[(score.clamp(-3, 3) + 3) as usize]
}

/// The nickname of profile p: profile bytes from +0x00 up to the first 0, at most 15.
fn nickname(w: &World, p: i32) -> String {
    let b = w.profiles.get(p.max(0) as usize * 0x5A..).unwrap_or(&[]);
    b.iter().take(15).take_while(|&&c| c != 0).map(|&c| c as char).collect()
}

impl Play {
    /// Step 0 (the day's verdict, only when the morning runs) and steps 1..3: `ja1_morning`, the office (only the world copy), `ja1_sunrise`, `ja1_strategic_open`; then the
    /// island map opens (mode 0, warned 0, no message). After an abort the steps start at the office (step 2). Returns (the result of
    /// `ja1_morning`, None when it did not run; `day` after the sunrise).
    pub fn day_steps(&mut self, w: &mut World) -> (Option<i32>, i32) {
        let office_only = std::mem::take(&mut self.day.from_office);
        self.day.verdict = None;
        self.day.rating = None;
        self.host.day_phase = true;
        // strategic.md "The battle show", At the end of a sector: no day step runs a battle (the window played the
        // show of the Leave path, dark, after the last pass).
        self.host.battle_show.clear();
        self.host.sunrise_shown = false;
        self.host.gone.clear();
        self.host.brenda.clear();
        self.host.offers.clear();
        self.host.pay_msgs.clear();
        self.host.pay_asks.clear();
        self.host.pay_redraw = false;
        self.host.pay_view = Default::default();
        self.host.pay_said.clear();
        self.host.merc_said.clear();
        self.host.office_lines.clear();
        if self.host.quests.is_none() {
            // SPEC GAP: no DAT\QUESTS.DAT in the game folder or on the CD (or no CD path) leaves op 307 at 250 zero bytes.
            // OpenCDFile: the game-folder file (1.13b patch) first, then the CD (iso::game_file).
            if let Some((iso, exe)) = self.files.clone() {
                let disc = crate::iso::Iso::open(&iso).ok();
                self.host.quests = disc.and_then(|mut i| crate::iso::game_file(Some(&exe), &mut i, "DAT\\QUESTS.DAT").ok());
            }
        }
        let m = if office_only {
            None
        } else {
            // Section 9 step 0: a new day allows a new A.I.M. visit (link = 1, strategic.md "The A.I.M. screen").
            if let Some(l) = w.accounts.get_mut(0xA3) {
                *l = 1;
            }
            // strategic.md The guard screen, At the day end: guards still on the way are lost.
            for s in 0..60 {
                if let Some(b5) = w.sector_state.get_mut(9 * s + 5) {
                    *b5 = 0;
                }
            }
            self.host.rating = None;
            self.host.home_lost = false;
            // ending.md "Game end" step 2: an end code set in the sector (1 won, 4 home lost) ends the game; the
            // verdict does not run.
            let code = w.update.leave_reason as i32;
            if code != 0 {
                self.day.game_over = Some(code);
                self.host.day_phase = false;
                return (None, w.day);
            }
            self.day.trace_line("dayend".into());
            // strategic.md "The day-end accounts": death totals, Pay the day, rest of the team, tappers on lost trees.
            Session::new(w, &mut self.host).day_end_accounts();
            // strategic.md "The day replay": after the sunset and before the verdict.
            self.day_replay(w);
            // Step 0: the day's verdict; a result other than 0 ends the game (the map does not open).
            // The verdict's quotes (and the bonus quote) show over the day replay screen until OK button 15.
            self.host.quote_under = self.host.replay_last.clone();
            let r = Session::new(w, &mut self.host).day_verdict();
            self.day.verdict = Some(r);
            self.day.rating = self.host.rating;
            if let Some(score) = self.day.rating {
                self.rating_picture(score);
            }
            if r != 0 {
                self.host.quote_under = None;
                // DayEndScreen? 0x65F9C: a verdict that ends the game skips the waits; the replay screen fades out.
                let shot = self.host.replay_last.clone();
                if let (Some(f), Some((s, p))) = (self.host.dayend_fade.as_mut(), shot.as_ref()) {
                    f(Some((s.as_slice(), p)));
                }
                self.day.game_over = Some(r);
                self.host.day_phase = false;
                return (None, w.day);
            }
            Session::new(w, &mut self.host).day_bonus();
            // strategic.md "The day-end screen", part 1: the replay screen waits for OK button 15.
            let shot = self.host.replay_last.clone();
            let q = self.host.dayend_win.as_mut().map_or(0, |f| f(w, 15, shot.as_ref().map(|(s, p)| (s.as_slice(), p))));
            self.host.quote_under = None;
            if q == 3 {
                return (self.dayend_exit(), w.day);
            }
            let m = Session::new(w, &mut self.host).morning();
            self.day.lines.append(&mut self.host.payroll_lines);
            // strategic.md "The day-end screen", day-end steps 1 and 2 (the window: the OK waits, the keys L / T /
            // Alt-X and the payroll faces; SPEC GAP front-end: a script does not wait for the OK buttons).
            // A morning that the poll stopped (result 1: the script ended inside the funds dialog) never reaches
            // button 16, so the steps do not run.
            if m == 0 {
                // Part 2: the payroll screen waits for OK button 16.
                if self.host.dayend_win.as_mut().map_or(0, |f| f(w, 16, None)) == 3 {
                    self.dayend_exit();
                    return (Some(m), w.day);
                }
                let r = Session::new(w, &mut self.host).day_end();
                self.day.lines.push(format!("day end {}", r));
                // DayEndScreen? 0x65F9C: the screen fades out after DayEndBankrupt?, also before a game over.
                if let Some(f) = self.host.dayend_fade.as_mut() {
                    f(None);
                }
                if r != 0 {
                    self.day.game_over = Some(r);
                    self.host.day_phase = false;
                    return (Some(m), w.day);
                }
            }
            Some(m)
        };
        self.day.trace_at("office", w);
        self.office_step(w);
        if self.day.to_menu {
            // Section 12, Leave: no sunrise; the main menu opens.
            self.host.day_phase = false;
            return (m, w.day);
        }
        self.day.office = Some(Box::new(w.clone()));
        self.day.write_save("OFFICE.SAV", w, true);
        Session::new(w, &mut self.host).sunrise();
        self.day.trace_at("sunrise", w);
        Session::new(w, &mut self.host).strategic_open();
        self.day.trace_at("strategic", w);
        if let Some(sr) = self.host.sunrise.as_mut() {
            self.day.lines.append(&mut sr.lines);
        }
        // music.md "Who plays what": the island map at the start of the day plays theme 2.
        crate::music::with(|m| m.play(2, false));
        self.day.gone_text = self.host.gone.last().map(|&p| format!("{} seems to have disappeared!", nickname(w, p)));
        self.host.battle_show.clear();
        self.host.day_phase = false;
        self.host.replay_fresh = true;
        self.day.map_open = true;
        self.day.warned = 0;
        self.day.msg = 0;
        (m, w.day)
    }

    /// strategic.md "The day replay": `ja1_day_replay` on the day's events, then its sounds and waits in order (the
    /// window waits in real time and Escape ends the rest, as EscPressed? 0x67300 does; a script starts the sounds
    /// with no wait).
    /// Script lines: `replay events N steps S`, `replay sound NAME` / `replay fx E` per sound (a repeat of the
    /// previous name is not written again); the pictures of the replay (replaypic.rs).
    /// A replay wait of `ms` is sound time (sound.md "Output"): the mixed samples of that time for the sink.
    fn replay_wait_sound(&mut self, ms: u32) -> Vec<i16> {
        self.host.sound.pass_us(ms as u64 * 1000);
        if self.host.sound.live { std::mem::take(&mut self.host.sound.pending) } else { Vec::new() }
    }

    pub fn day_replay(&mut self, w: &mut World) {
        use crate::play::ReplayEv;
        self.host.replay_show.clear();
        self.host.replay_last = None;
        self.host.replay_start = None;
        let n = self.host.replay_events.len();
        self.host.replay_pic = self.files.as_ref().and_then(|(iso, exe)| {
            crate::replaypic::ReplayPic::new(iso, exe, &w.sector_replay).ok()
        });
        if let Some(p) = self.host.replay_pic.as_mut() {
            let day = crate::mappanel::replay_day(&w.accounts, [w.update.dead_mercs, w.update.dead_allies, w.update.dead_npc, w.update.enemy_deaths,
                                                                w.start_sectors, w.start_trees, w.start_workable]);
            p.panel_numbers(&w.accounts, day);
        }
        Session::new(w, &mut self.host).day_replay();
        self.host.replay_start = self.host.replay_pic.as_ref().map(|p| p.start.clone());
        let fade = self.host.replay_pic.as_ref().map(|p| (p.start.clone(), p.fade_in()));
        // With no picture shown (Day Replays off, or no record: ReplayScreenInit 0x64274 always draws the screen, DayReplay
        // 0x652B8 skips its map at count 0) the screen is the start picture, and DayEndVerdict 0x60680 draws the word on it.
        if let Some(p) = self.host.replay_pic.as_ref() {
            self.host.replay_last = Some((p.start.clone(), p.palette));
        }
        let (pal, shot) = self.host.replay_pic.take().map_or((None, None), |p| (Some(p.palette), p.battle_shot.or(p.death_shot).or(p.shot)));
        let show = std::mem::take(&mut self.host.replay_show);
        if n == 0 {
            return;
        }
        let frames = show.iter().filter(|e| matches!(e, ReplayEv::Frame(_))).count();
        if let Some((shot, pal)) = shot.zip(pal) {
            self.host.replay_mid = Some((shot, pal));
        }
        self.day.lines.push(format!("replay pictures {}", frames));
        self.day.lines.push(format!("replay events {} steps {}", n, show.len()));
        if self.host.sunrise.as_ref().is_some_and(|s| s.sink.is_some()) && !show.is_empty() {
            eprintln!("window replay {} steps", show.len());
        }
        // strategic.md "The replay pictures": the screen fades in, one level per retrace.
        if let (Some((start, levels)), false) = (fade, show.is_empty()) {
            if let Some(k) = self.host.sunrise.as_mut().and_then(|s| s.sink.as_mut()) {
                for d in &levels {
                    k.show(&start, d, true);
                }
            }
            self.day.lines.push(format!("replay fade {}", levels.len()));
        }
        let mut last = String::new();
        // JA1_SPEED_LOG=1 (world_load.md "Game speed"): `speed replay MS ms WAITS ms waits` when the replay ran
        // to its end in the window (WAITS = the sum of its waits, the original's busy-wait counts).
        let (t0, waits, mut skipped) = (std::time::Instant::now(), show.iter().map(|e| if let ReplayEv::Wait(ms) = e { *ms as u64 } else { 0 }).sum::<u64>(), false);
        let windowed = self.host.sunrise.as_ref().is_some_and(|s| s.sink.is_some());
        let shown = crate::play::replay_shown(&show);
        for (ev, on) in show.into_iter().zip(shown) {
            let line = match ev {
                ReplayEv::Sound(name, vol) => self.host.sound.replay(name, vol).then(|| format!("replay sound {}", name)),
                ReplayEv::Fx(e, vol) => {
                    self.host.sound.sfx(e, 0x2B11, vol, 1);
                    Some(format!("replay fx {}", e))
                }
                ReplayEv::Frame(f) => {
                    if let (true, Some(k), Some(pal)) = (on, self.host.sunrise.as_mut().and_then(|s| s.sink.as_mut()), pal.as_ref()) {
                        k.show(&f, pal, false);
                    }
                    if let Some(pal) = pal {
                        self.host.replay_last = Some((f, pal));
                    }
                    None
                }
                ReplayEv::Wait(ms) => {
                    let pcm = self.replay_wait_sound(ms);
                    if let Some(k) = self.host.sunrise.as_mut().and_then(|s| s.sink.as_mut()) {
                        k.audio(&pcm);
                        k.delay(ms);
                        if k.poll_escape() {
                            self.host.sound.effects_stop();
                            eprintln!("window replay skipped");
                            skipped = true;
                            break;
                        }
                    }
                    None
                }
            };
            if let Some(l) = line {
                if l != last {
                    self.day.lines.push(l.clone());
                    last = l;
                }
            }
        }
        if windowed && !skipped && std::env::var("JA1_SPEED_LOG").is_ok_and(|v| v == "1") {
            eprintln!("speed replay {} ms {} ms waits", t0.elapsed().as_millis(), waits);
        }
    }

    /// strategic.md "The day's verdict" step 5: op 331 draws the rating word on the screen, the last picture of the
    /// day replay, at (276, 161) centred, font 0, colour 0xFF, shadow 1; it stays there while the guide speaks.
    fn rating_picture(&mut self, score: i32) {
        let word = rating_word(score);
        let Some((iso, exe)) = self.files.clone() else { return };
        let Some((mut s, pal)) = self.host.replay_last.take() else {
            self.day.lines.push(format!("rating word {} (no replay picture)", word));
            return;
        };
        if !rating_word_on(&iso, &exe, &mut s, score) {
            return;
        }
        if let Some(k) = self.host.sunrise.as_mut().and_then(|x| x.sink.as_mut()) {
            k.show(&s, &pal, false);
        }
        self.day.lines.push(format!("rating word {} at (276, 161)", word));
        self.host.replay_last = Some((s, pal));
    }

    /// strategic.md "The day-end screen", key Alt-X, yes (panel.md "Exit to DOS", argument 0): the day files go
    /// when there is no QUICK.SAV, the quit line; the program ends (no office, no menu).
    fn dayend_exit(&mut self) -> Option<i32> {
        let exe = self.files.clone().map(|f| f.1).unwrap_or_default();
        let (deleted, line) = crate::question::exit_finish(false, self.day.save_dir.as_deref(), &exe, &mut self.host.rng, || {});
        if deleted {
            eprintln!("exit to dos: day files deleted");
        }
        if self.files.is_some() {
            eprintln!("exit to dos {}", String::from_utf8_lossy(&line));
            println!("{}\n", String::from_utf8_lossy(&line));
        }
        self.host.office_quit = true;
        self.day.to_menu = true;
        self.host.day_phase = false;
        None
    }

    /// Step 2, the office (strategic.md "The office"): the window's loop, or the script's polls until Sleep; no
    /// polls: the step ends at once.
    fn office_step(&mut self, w: &mut World) {
        if let Some(f) = self.host.office_win.as_mut() {
            // The seed: the A.I.M. screen draws randoms (strategic.md "The A.I.M. screen", opening step 1).
            match f(w, &mut self.host.rng.seed) {
                1 => self.host.office_quit = true,
                2 => self.day.to_menu = true,
                // panel.md "Exit to DOS": no office save after the quit.
                3 => {
                    self.host.office_quit = true;
                    self.day.to_menu = true;
                }
                _ => {}
            }
            return;
        }
        if self.host.office_polls.is_empty() {
            return;
        }
        let Some((iso, exe)) = self.files.clone() else { return };
        let mut o = match crate::office::Office::new(&iso, &exe, w) {
            Ok(o) => o,
            Err(c) => {
                eprintln!("office: error {}", c);
                return;
            }
        };
        // strategic.md "Fades of the office" (lines `fade N FROM TO`; the window shows them).
        self.host.office_lines.push("fade 24 0 256".into());
        let mut aim: Option<crate::aim::Aim> = None;
        while let Some((x, y, b, keys)) = self.host.office_polls.pop_front() {
            if x == crate::play::LATCH_POLL {
                crate::invextra::set_latch(y as u8);
                continue;
            }
            if let Some(a) = aim.as_mut() {
                // The A.I.M. screen (strategic.md "The A.I.M. screen"): B = the buttons held.
                let info = a.info_open();
                let r = a.poll(w, x, y, b & 3, &keys);
                self.host.rng.seed = a.rng.seed;
                // The mouse place the window would move the pointer to (script: a line; the next poll gives the place).
                if let Some(m) = a.warp.take() {
                    self.host.office_lines.push(format!("aim warp {} {}", m.0, m.1));
                }
                self.host.office_lines.extend(r.lines);
                if a.info_open() != info && !r.leave {
                    // "The A.I.M. statistics page": out, then in.
                    self.host.office_lines.push("fade 24 256 0".into());
                    self.host.office_lines.push("fade 24 0 256".into());
                }
                // panel.md "Exit to DOS" (argument 0): the day files go when there is no QUICK.SAV, then the
                // program ends (the script stops here, as Leave without the menu).
                if let Some(line) = r.quit {
                    if self.day.save_dir.as_ref().is_some_and(|d| crate::question::exit_deletes(d)) {
                        self.host.office_lines.push("aim quit day files deleted".into());
                    }
                    println!("{}\n", String::from_utf8_lossy(&line));
                    self.host.office_quit = true;
                    self.day.to_menu = true;
                    self.host.office_polls.clear();
                    break;
                }
                if r.leave {
                    aim = None;
                    crate::aim::Aim::leave_music();
                    self.host.office_lines.push("fade 24 256 0".into());
                    crate::music::with(|m| m.stop_theme());
                    // The link, the team and the faces changed: the office is drawn again.
                    match crate::office::Office::new(&iso, &exe, w) {
                        Ok(n) => o = n,
                        Err(c) => eprintln!("office: error {}", c),
                    }
                    self.host.office_lines.push("fade 24 0 256".into());
                } else if let Some(s) = self.host.aim_shot.as_mut() {
                    s(&a.picture(), &a.pal);
                }
                continue;
            }
            if o.slots.is_some() {
                // The slot list (strategic.md "Save Game and Restore Game"): B = the buttons held.
                let r = o.slots_poll(x, y, b & 3, &keys);
                if let Some(s) = self.host.slots_shot.as_mut() {
                    s(&o.picture(), &o.pal);
                }
                match r {
                    Some(crate::saveslots::Outcome::NoSaves) => {
                        self.host.office_lines.push("slots none".into());
                        self.host.office_lines.push("office msg".into());
                    }
                    Some(out) => {
                        let sl = o.close_slots().expect("open list");
                        if let Some(t) = crate::saveslots::busy_text(&sl, out) {
                            self.host.office_lines.push(format!("slots box {}", t));
                            if let Some(s) = self.host.slots_shot.as_mut() {
                                s(&o.busy_picture(t), &o.pal);
                            }
                        }
                        let dir = self.day.save_dir.clone();
                        let line = crate::saveslots::finish(w, &sl, out, dir.as_deref(), &iso, &exe);
                        let restored = line.starts_with("slots restored");
                        self.host.office_lines.push(line);
                        if restored {
                            // Faces, the A.I.M. link, the calendar and the team of the loaded game.
                            match crate::office::Office::new(&iso, &exe, w) {
                                Ok(n) => o = n,
                                Err(c) => eprintln!("office: error {}", c),
                            }
                        }
                    }
                    None => {}
                }
                continue;
            }
            if o.sbox.is_some() {
                // The switch box (strategic.md "The switch box (Control Panel)"): a left press acts once.
                if b & 1 != 0 {
                    let mut steps = vec![];
                    let (t, closed) = o.switchbox_press(w, x, y, &mut steps);
                    self.switchbox_lines(&steps);
                    for (k, v) in t {
                        self.host.office_lines.push(format!("switchbox {} {}", k, v));
                    }
                    if closed {
                        self.host.office_lines.push("switchbox closed".into());
                    } else if let Some(s) = self.host.switchbox_shot.as_mut() {
                        s(&o.picture(), &o.pal);
                    }
                }
                continue;
            }
            if o.ledger.is_some() {
                if o.ledger_again_shown() {
                    // panel.md "Say it again (key L)": a key or a press only ends the box.
                    if b & 0b1111 != 0 || !keys.is_empty() {
                        o.ledger_again_end();
                        self.host.office_lines.push("ledger again end".into());
                    }
                    continue;
                }
                if keys.iter().any(|&k| k.to_ascii_uppercase() == b'L') {
                    match o.ledger_again(&iso, &exe, w) {
                        Ok(Some((k, _))) => self.host.office_lines.push(format!("ledger again {}", k)),
                        Ok(None) => self.host.office_lines.push("ledger again none".into()),
                        Err(c) => eprintln!("ledger again: error {}", c),
                    }
                    if let Some(s) = self.host.ledger_shot.as_mut() {
                        s(&o.picture(), &o.pal);
                    }
                    continue;
                }
                if b & 0b1101 != 0 {
                    // strategic.md "The team ledger", Fades: page out, room in (25 levels each).
                    let out = crate::office::fade(&o.pal, 24, 256, 0);
                    o.close_ledger();
                    let back = crate::office::fade(&o.pal, 24, 0, 256);
                    self.host.office_lines.push(format!("ledger fade out {} {}", out.len(), pal_sum(&out[0])));
                    self.host.office_lines.push("office ledger closed".into());
                    self.host.office_lines.push(format!("ledger fade in {} {}", back.len(), pal_sum(back.last().unwrap())));
                }
                continue;
            }
            // strategic.md "The office": Alt+X (extended key 0x2D) fades out (24 steps) and the program ends with the
            // line "Normal exit." (no question, no file deleted).
            if o.msg.is_none() && keys.windows(2).any(|k| k == [0, 0x2D]) {
                self.host.office_lines.push("office exit".into());
                crate::music::with(|m| m.set_volume(0, 1200));
                self.host.office_lines.push("fade 24 256 0".into());
                println!("{}\n", crate::office::NORMAL_EXIT);
                self.host.office_quit = true;
                self.day.to_menu = true;
                self.host.office_polls.clear();
                break;
            }
            let p = o.poll(x, y, b & 1 != 0, b & 2 != 0);
            for &(_, fx, _) in &p.sounds {
                self.host.office_lines.push(format!("screen sound {}", crate::sound::main_name(fx)));
            }
            if let Some(k) = p.click {
                self.host.office_lines.push(format!("office click {}", k));
            }
            if p.ledger {
                let out = crate::office::fade(&o.pal, 24, 256, 0);
                self.host.office_lines.push(format!("ledger fade out {} {}", out.len(), pal_sum(&out[0])));
                o.open_ledger(w);
                self.host.office_lines.push("office ledger".into());
                let back = crate::office::fade(&o.pal, 24, 0, 256);
                self.host.office_lines.push(format!("ledger fade in {} {}", back.len(), pal_sum(back.last().unwrap())));
                if let Some(s) = self.host.ledger_shot.as_mut() {
                    s(&o.picture(), &o.pal);
                }
            }
            if p.switchbox {
                let steps = o.open_switchbox(w);
                self.host.office_lines.push("office switchbox".into());
                self.switchbox_lines(&steps);
                if let Some(s) = self.host.switchbox_shot.as_mut() {
                    s(&o.picture(), &o.pal);
                }
            }
            if let Some(m) = p.slots {
                let dir = self.day.save_dir.clone();
                o.open_slots(m, dir.as_deref(), w);
                self.host.office_lines.push(format!("slots open {}", m));
            }
            if p.aim {
                // OfficeScreen 0x4F297, 0x4F313: the music volume goes to 0 over 1200 ms.
                crate::music::with(|m| m.set_volume(0, 1200));
                self.host.office_lines.push("fade 24 256 0".into());
                if let Some((p, n)) = self.host.aim_debt.take() {
                    w.profiles[p * 0x5A + 0x3E..p * 0x5A + 0x40].copy_from_slice(&n.to_le_bytes());
                }
                match crate::aim::Aim::new(&iso, &exe, w, self.host.rng.seed) {
                    Ok((a, lines)) => {
                        self.host.rng.seed = a.rng.seed;
                        self.host.office_lines.extend(lines);
                        self.host.office_lines.push("fade 24 0 256".into());
                        if let Some(s) = self.host.aim_shot.as_mut() {
                            s(&a.picture(), &a.pal);
                        }
                        aim = Some(a);
                    }
                    Err(c) => eprintln!("aim: error {}", c),
                }
            }
            if p.msg {
                self.host.office_lines.push("office msg".into());
            }
            if p.sleep {
                self.host.office_lines.push("office sleep".into());
                crate::music::with(|m| m.set_volume(0, 2400));
                self.host.office_lines.push("fade 56 256 0".into());
                break;
            }
            if p.leave {
                self.host.office_lines.push("office leave".into());
                crate::music::with(|m| m.set_volume(0, 1200));
                self.host.office_lines.push("fade 56 256 0".into());
                self.day.to_menu = true;
                break;
            }
        }
        if let Some(s) = self.host.office_shot.as_mut() {
            s(&o.picture(), &o.pal);
        }
    }

    /// Script lines of switch box steps: `screen sound NAME` per heard sound, `music on|off` per music switch.
    fn switchbox_lines(&mut self, steps: &[crate::switchbox::Step]) {
        for s in steps {
            match s {
                crate::switchbox::Step::Sfx(fx, _, _, true) => {
                    self.host.office_lines.push(format!("screen sound {}", crate::sound::main_name(*fx)));
                }
                crate::switchbox::Step::Music(on, _) => {
                    crate::music::with(|m| m.switch(*on));
                    self.host.office_lines.push(format!("music {}", if *on { "on" } else { "off" }));
                }
                _ => {}
            }
        }
    }

    /// A click on the map: `ja1_map_click (sector, mode, buttons, warned)`; the random numbers are play's.
    pub fn day_map_click(&mut self, w: &mut World, sector: i32, mode: i32, buttons: i32) -> i32 {
        // The mouse is on the sector for the click (strategic.md "The map panel", Mouse over the map).
        crate::mapui::mouse_sector(w, sector);
        let mut warned = self.day.warned;
        let n = Session::new(w, &mut self.host).map_click(sector, mode, buttons, &mut warned);
        self.day.warned = warned;
        self.day.msg = n;
        n
    }

    /// Enter (button 0) or the start button (button 1) on the map: `ja1_map_start (button, hinted)`; at 0
    /// `ja1_day_start` with `ask` as the answer of op 303.
    /// Returns (the start checks' result, the day start's result or None when it did not run).
    pub fn day_map_start(&mut self, w: &mut World, button: i32, ask: Box<dyn FnMut([i32; 4]) -> u32>) -> (i32, Option<i32>) {
        let mut hinted = w.hinted;
        let m = Session::new(w, &mut self.host).map_start(button, &mut hinted);
        w.hinted = hinted;
        self.day.msg = m;
        if m != 0 {
            return (m, None);
        }
        self.host.day_ask = Some(ask);
        self.host.day_ask_args = None;
        let r = Session::new(w, &mut self.host).day_start();
        self.host.day_ask = None;
        (m, Some(r))
    }

    /// Day start result 1: keep the day copy, close the map, enter `cur_sector` (section 4; the random numbers go
    /// on), and play goes on in that sector.
    pub fn day_enter(&mut self, w: &mut World, iso: &str, exe: &str) -> Result<(), String> {
        self.day.day_save = Some(Box::new(w.clone()));
        self.day.write_save("DAY.SAV", w, false);
        // #69 recorder (record.rs): a copy of the day save, read only.
        if crate::record::on() {
            crate::record::day_save(w.day, &w.save_bytes());
        }
        self.sector_entry(w, iso, exe)?;
        self.day.trace_at("tactical 0", w);
        Ok(())
    }

    /// world_load.md section 12 "Continue last day in progress?" (PlayLoop 0x4FB94 with EAX 1): the day goes on from
    /// QUICK.SAV (Quick Restore steps 2..7 with no question): no office, no day-start saves. Restart this day and Abort
    /// this day take the day files of the save folder.
    pub fn continue_day(&mut self, w: &mut World, iso: &str, exe: &str) -> Result<i16, String> {
        self.sector_head();
        let t = self.quick_restore(w)?;
        let dir = self.day.save_dir.clone();
        let load = |f: &str| dir.as_ref().and_then(|d| World::load(iso, exe, &d.join(f).to_string_lossy()).ok());
        self.day.day_save = Some(Box::new(load("DAY.SAV").unwrap_or_else(|| w.clone())));
        self.day.office = load("OFFICE.SAV").map(Box::new);
        self.day.trace_at("tactical 1", w);
        Ok(t)
    }

    /// Section 8 "Abort or restart the day", after the pass that `Input::Abort` stopped. Restart: the world becomes
    /// the day start copy, the sector entry runs for `cur_sector`, play goes on. Abort: the world becomes the office
    /// copy and the day loop goes on at step 2, the office (play stays stopped); with no office copy yet it works as a restart.
    /// The random numbers go on.
    pub fn abort_day(&mut self, w: &mut World, restart: bool) -> Result<(), String> {
        // savegame.md "Which loads take Sound FX and Music from the file": a day save load keeps both.
        let keep = crate::ctlpanel::sound_switches(w);
        if !restart {
            if let Some(o) = self.day.office.as_ref() {
                *w = (**o).clone();
                crate::ctlpanel::put_sound_switches(w, keep);
                self.day.trace_at("load OFFICE.SAV", w);
                self.left = true;
                self.day.from_office = true;
                self.day.map_open = false;
                self.day.msg = 0;
                return Ok(());
            }
        }
        let d = self.day.day_save.as_ref().ok_or("abort: no day start copy of the world")?;
        *w = (**d).clone();
        crate::ctlpanel::put_sound_switches(w, keep);
        self.day.trace_at("load DAY.SAV", w);
        let (iso, exe) = self.files.clone().ok_or("abort: no CD image / JA.EXE for the sector entry")?;
        // strategic.md "The day replay": the restarted day runs the sector loader again, which empties the events.
        self.host.replay_fresh = true;
        self.sector_entry(w, &iso, &exe)?;
        self.day.trace_at("tactical 0", w);
        Ok(())
    }

    /// soldier_update.md "Traverse to a neighbour sector", the move, after the pass that a traverse with result 1
    /// stopped; `out` = (entry tile, new sector, facing). The day goes on in the new sector (no clock reset, no
    /// day-start copies).
    pub fn traverse_move(&mut self, w: &mut World, out: [i16; 3]) -> Result<(), String> {
        let (iso, exe) = self.files.clone().ok_or("traverse: no CD image / JA.EXE for the new sector")?;
        let (tile, dest, facing) = (out[0] as i32, out[1] as i32, out[2] as i32);
        // Step 1: Traverse out (op 214 leaves the old sector; its op 219 tmp file goes with the sector files).
        {
            let mut s = crate::play::Session::new(w, &mut self.host);
            s.traverse_out(dest);
        }
        if let Some((sec, f)) = self.host.saved_tmp.take() {
            if let Some(slot) = w.tmp_files.get_mut(sec as usize) {
                *slot = Some(f);
            }
        }
        // Steps 2 and 3: the new map, the sector setup, the view jump to the merc's tile.
        self.load_moved(w, &iso, &exe, tile, facing, None)?;
        Ok(())
    }

    /// Steps 4 and 5 of the move: Traverse in with the playing theme; m != 0: the music changes to theme m. TraverseMove? runs them
    /// behind the fade in (0x316DE, then 0x316EF..0x317A5); the window calls this behind its fade, a script at once.
    pub fn traverse_theme(&mut self, w: &mut World) {
        let (theme, playing) = crate::music::with(|m| (m.playing(), m.still_playing()));
        let m = crate::play::Session::new(w, &mut self.host).traverse_in(library_theme(theme), playing as i32);
        eprintln!("traverse theme {}", m);
        if let Some(m) = music_change(m) {
            crate::music::with(|mu| mu.play(m, false));
        }
    }

    /// The new map of `cur_sector` (world_load.md section 4 step 1), `ja1_setup_sector (tile, facing)` with the
    /// section 7 host (the random numbers go on), the view jump with flag 0 to `view` (None: the selected merc's
    /// tile after the setup, no jump when selected >= 8), the screen drawn and the front-end play state of a new sector (traverse and squad lost).
    fn load_moved(&mut self, w: &mut World, iso: &str, exe: &str, tile: i32, facing: i32, view: Option<i32>) -> Result<(), String> {
        let cur = w.cur_sector as usize;
        if cur >= crate::world::SECTOR_COUNT {
            return Err(format!("traverse: sector {} is not a tactical sector (0..59)", cur));
        }
        crate::strips::clear_lost_shade(); // TraverseMove 0x316B0 reloads the strip.
        let ts = crate::enter::fill_map(w, iso)?;
        w.tileset = ts;
        w.accounts[0x98] = ts; // g_tileset 0x22A038: LoadTerrainTileset in TraverseMove 0x315DC, saved at file 20836
        if let Some(n) = self.host.setup_seed.take() {
            self.host.rng.seed = n;
        }
        let mut disc = crate::iso::Iso::open(iso).map_err(|e| format!("{}: {}", iso, e))?;
        let mut host = crate::host::EnterHost::new(&mut disc, self.host.rng.seed);
        host.exe = Some(exe.to_string());
        host.tmp_files = w.tmp_files.clone();
        crate::host::setup_sector(w, &mut host, tile, facing);
        self.host.rng.seed = host.rng.seed;
        self.host.rng.draws += host.rng.draws;
        let view = view.or_else(|| selected_tile(w));
        if let Some(view) = view {
            let mut v = crate::focus::View { view_x: w.view_x, view_y: w.view_y, view_w: w.view_w, view_h: w.view_h,
                opt_onscreen: w.opt_onscreen as i32, follow_all: w.update.follow_all as i32, focus_slot: w.update.focus_slot as i32,
                shown_slot: w.update.shown_slot, show_timer: w.update.show_timer };
            crate::focus::jump_tile(&mut v, view, 0);
            (w.view_x, w.view_y) = (v.view_x, v.view_y);
        }
        let (c0, r0) = crate::enter::view_tile(w);
        crate::enter::render_world(iso, exe, w, ts, c0, r0, crate::enter::COLS, crate::enter::ROWS)
            .map_err(|c| format!("draw (error {})", c))?;
        w.update.opt_reload_msg = 1;
        self.day.tileset = ts;
        self.day.map_open = false;
        self.day.msg = 0;
        self.wait = None;
        self.host.banner = None;
        self.host.noise_msg = None;
        self.host.sight = Default::default();
        self.host.anims = Default::default();
        // sound.md: the new sector's soldiers give the tactical bank.
        self.host.sound.load_tactical(&w.soldiers);
        Ok(())
    }

    /// savegame.md "Quick Save" steps 2..5: `QUICK.SAV` in the save folder. Returns its size.
    pub fn quick_save(&mut self, w: &mut World) -> Result<usize, String> {
        let dir = self.day.save_dir.clone().ok_or("quick save: no save folder")?;
        let theme = crate::music::with(|m| m.playing()) as i16;
        (w.marker_kind, w.marker_tile) = (self.cursor.marker_kind, self.cursor.marker_tile);
        (w.cursor_mode, w.cursor_value) = (self.cursor.mode, self.cursor.value);
        w.sight_queue = self.host.sight.bytes();
        w.music_wanted = crate::music::with(|m| m.wanted()) as i16;
        (w.mouse_saved_x, w.mouse_saved_y) = crate::pointer::mouse_saved();
        w.replay_log = self.host.replay_events.clone();
        w.tile_anim_block = self.host.anims.to_block();
        self.host.anims.fx_timers_to(&mut w.fx_anims);
        let f = crate::quicksave::write(w, theme);
        // validate/save_xcompat.py: the world that wrote this file, to compare with the original after its load.
        if let Ok(p) = std::env::var("JA1_QUICK_FIELDS") {
            std::fs::write(p, crate::quicksave::field_bytes(w)).map_err(|e| format!("JA1_QUICK_FIELDS: {}", e))?;
        }
        std::fs::write(dir.join("QUICK.SAV"), &f).map_err(|e| format!("QUICK.SAV: {}", e))?;
        Ok(f.len())
    }

    /// savegame.md "Quick Restore" steps 2..7: `QUICK.SAV` from the save folder into the world, then the sector's
    /// tileset, faces and strips load again in the window (`moved`). Returns the saved playing theme.
    pub fn quick_restore(&mut self, w: &mut World) -> Result<i16, String> {
        // QuickRestore 0x2C898 reloads the strip with LoadStripPccs.
        crate::strips::set_shade_last(0);
        let dir = self.day.save_dir.clone().ok_or("quick restore: no save folder")?;
        let (iso, exe) = self.files.clone().ok_or("quick restore: no CD image / JA.EXE")?;
        let q = std::fs::read(dir.join("QUICK.SAV")).map_err(|e| format!("QUICK.SAV: {}", e))?;
        // Step 2.
        crate::music::with(|m| if m.playing() > 0 { m.stop_theme() });
        self.host.sound.stop_all();
        // Steps 4 and 5.
        let jagged = crate::iso::Iso::open(&iso).and_then(|mut d| d.read("DAT/JAGGED.DAT")).map_err(|e| format!("{}: {}", iso, e))?;
        let theme = crate::quicksave::restore(w, &jagged, &q)?;
        (self.cursor.marker_kind, self.cursor.marker_tile) = (w.marker_kind, w.marker_tile);
        (self.cursor.mode, self.cursor.value) = (w.cursor_mode, w.cursor_value);
        crate::music::with(|m| m.set_wanted(w.music_wanted as i32));
        crate::pointer::mouse_save((w.mouse_saved_x, w.mouse_saved_y));
        self.host.replay_events = std::mem::take(&mut w.replay_log);
        // Step 6: the tileset of cur_sector; the window loads its palette, faces and strips and draws the screen.
        let cur = w.cur_sector as usize;
        if cur >= crate::world::SECTOR_COUNT {
            return Err(format!("quick restore: sector {} is not a tactical sector (0..59)", cur));
        }
        self.host.anims = Default::default();
        self.host.anims.from_block(&w.tile_anim_block);
        self.host.anims.fx_timers_from(&w.fx_anims);
        w.tile_frames = self.host.anims.sprite_frames();
        let ts = crate::common(&iso).map_err(|e| format!("TERRAIN.DAT (error {})", e))?.tileset(cur + 1);
        let (c0, r0) = crate::enter::view_tile(w);
        crate::enter::render_world(&iso, &exe, w, ts, c0, r0, crate::enter::COLS, crate::enter::ROWS)
            .map_err(|c| format!("draw (error {})", c))?;
        self.day.tileset = ts;
        self.wait = None;
        self.host.banner = None;
        self.host.noise_msg = None;
        // QuickRestore 0x2CBF5 reads the queue: a reaction that waited for an effect shows after the load.
        self.host.sight = crate::sighting::Sight::from_bytes(w.sight_queue);
        self.host.sound.load_tactical(&w.soldiers);
        self.moved = true;
        self.quick_restored = true;
        // Step 7. Sound FX and Music now have the file's values (savegame.md "Options block").
        self.host.sound.fx_off = crate::ctlpanel::fx_off(w);
        let fx_off = self.host.sound.fx_off;
        crate::sound::screen(|s| s.fx_off = fx_off);
        let on = crate::ctlpanel::get(w, 1) != 0;
        crate::music::with(|m| m.on = on);
        let (over_map, _, start) = crate::quicksave::after_restore(theme);
        self.cursor.over_map = over_map;
        if let Some(t) = start {
            crate::music::with(|m| m.play(t as i32, false));
        }
        // 0x2D823: MouseSetPos(230, 110) after the strips load.
        self.host.mouse_to = Some(crate::quicksave::RESTORE_MOUSE);
        self.mouse_px = crate::quicksave::RESTORE_MOUSE;
        Ok(theme)
    }

    /// soldier_update.md "Squad lost" (host op 188, after the pass that called it): the old sector is left and
    /// sector 59 loads in the same day, the squad's entry tile in view. The music (theme 6) changed at the op.
    pub fn squad_lost_move(&mut self, w: &mut World) -> Result<(), String> {
        let (iso, exe) = self.files.clone().ok_or("no CD image / JA.EXE for sector 59")?;
        let sel = w.selected;
        // Step 1.
        w.combat = 0;
        // Step 3: op 214 leave_sector (0); its op 219 tmp file goes with the sector files.
        crate::play::Session::new(w, &mut self.host).leave_sector(0);
        if let Some((sec, f)) = self.host.saved_tmp.take() {
            if let Some(slot) = w.tmp_files.get_mut(sec as usize) {
                *slot = Some(f);
            }
        }
        // Steps 4 and 5: sector 59, its entry tile, the view jump to that tile.
        w.cur_sector = 59;
        let tile = w.entry_tiles[59];
        self.load_moved(w, &iso, &exe, tile, 1, Some(tile))?;
        // Step 6: the selected merc (First merc when >= 8), cursor mode walk; the window's new strips start
        // with combat seen 0.
        let sel = if sel >= 8 { crate::play::Session::new(w, &mut self.host).first_merc() as u8 } else { sel };
        w.selected = sel;
        // 0x3A419: the selected merc's face gets the lit frame.
        if sel < 8 {
            crate::panel::note_face_light(sel as usize, crate::panel::LIGHT_ON);
        }
        // 0x3A42C..0x3A473: the tactical screen is on (0x92DCC) and the mouse on the map (0x284F13); the view hold
        // (0x92DCE) and the turn banner flag (0x92DCA) end. The cursor starts in walk mode: MoveCursorState 0x1C464
        // gives mode 1 (0xA1) for a living merc and clears the AP number and the cursor tile.
        w.update.screen_drawn = 1;
        w.update.view_hold = 0;
        w.update.turn_shown = 0;
        crate::strips::set_lost_shade();
        self.cursor.over_map = 1;
        self.cursor.mode = 0;
        crate::play::Session::new(w, &mut self.host).cursor_state_move(&mut self.cursor, sel as usize);
        self.moved = true;
        self.day.lines.push(format!("squad lost sector {} tile {} selected {}", w.cur_sector, tile, sel));
        Ok(())
    }

    /// The sector entry of section 4 for `cur_sector` (random numbers go on) and the play state of a new entry.
    fn sector_entry(&mut self, w: &mut World, iso: &str, exe: &str) -> Result<(), String> {
        self.day.map_open = false;
        self.day.msg = 0;
        // strategic.md "The day replay": the first sector entry of a day empties the day's events.
        if std::mem::take(&mut self.host.replay_fresh) {
            self.host.replay_events.clear();
        }
        let (ts, seed, draws) = crate::enter::enter_world(iso, exe, w, self.host.rng.seed)?;
        self.host.rng.seed = seed;
        self.host.rng.draws += draws;
        self.day.tileset = ts;
        self.left = false;
        self.host.sound.bank_stale = true;
        // SPEC GAP: section 9 does not say which front-end play state starts again in the new sector; the
        // end-of-day title, the turn banner, the noise line and the animation timers start empty.
        self.host.end_of_day = false;
        self.host.banner = None;
        self.host.noise_msg = None;
        self.host.sight = Default::default();
        self.host.anims = Default::default();
        self.host.sunrise_shown = false;
        self.sector_start(w);
        Ok(())
    }

    /// The front-end state that LoadTacticalSector? 0x2AD20 starts again on each entry: the tile marker (0x229CA8,
    /// 0x229CAC, 0x229D40), the blink clocks (0x229D4C, 0x229D38), over_map 0x284F13, the view scroll (0x9344C,
    /// 0x9344D, 0x229D28, 0x229D44), the lit face 0x284F0E, the slot hover 0x93448. Then the cursor at 0x2B041 /
    /// 0x2B177: no first merc gives cursor 0xA1 and mode 0; a sector with mercs then runs CursorModeSet (first merc, 1).
    pub fn sector_start(&mut self, w: &mut World) {
        crate::strips::clear_lost_shade(); // LoadTacticalSector? 0x2B1D8 reloads the strip.
        self.sector_head();
        self.sector_cursor(w);
    }

    /// The first part of `sector_start`: LoadTacticalSector? 0x2AD20 writes these with EAX 0 and with EAX 1. With EAX 1
    /// (Continue last day) QuickRestore then runs at its end, so `continue_day` runs this first.
    fn sector_head(&mut self) {
        let c = &mut self.cursor;
        c.marker_kind = 0;
        c.marker_tile = 0xC41;
        c.marker_blink = 200;
        c.blink_timer = 200;
        c.mark_timer = 100;
        c.over_map = 1;
        self.scroll = crate::viewmode::Scroll { dy: 0, dx: 0, hover: 200, delay: 1 };
        self.hover.lit = 0x40;
        self.slots.hover = 0x40;
    }

    /// The end of `sector_start`: the cursor at 0x2B041 / 0x2B177.
    fn sector_cursor(&mut self, w: &mut World) {
        let sel = w.selected as i32;
        if sel >= 8 {
            self.cursor.value = 0xA1;
            self.cursor.mode = 0;
        }
        // Only when the sector holds mercs, also with no first merc (the original then reads record 8).
        if w.sector_state.get(w.cur_sector as usize * 9 + 2).copied().unwrap_or(0) != 0 {
            let mut s = crate::play::Session::new(w, &mut self.host);
            s.cursor_mode_set(&mut self.cursor, sel, 1);
        }
    }
}

/// Section 9 step 1: the text of op 270 pd_message n (strategic.md Not enough funds); other n: none.
pub fn pay_text(n: i32) -> Option<&'static str> {
    match n {
        0 => Some("NOT ENOUGH FUNDS TO PAY ALL MERCENARIES!"),
        1 => Some("Choose someone to... er... be the... ah... charitable one..."),
        2 => Some("The native guide is helping you for free!"),
        _ => None,
    }
}

/// Section 9 step 1: what the "Not enough funds" dialog shows besides its message box (strategic.md "Not enough
/// funds"): the blink of the funds numbers (op 274), the face tooltip (ops 234 / 233), the fresh panel numbers
/// (op 269 after a dismissal, op 278 at the end) and the frames on the paid faces (op 279); `end` = op 280 showed
/// the panel buffer (no message box, no tooltip, no blink).
#[derive(Clone, Default, PartialEq, Debug)]
pub struct PayView {
    pub blink: bool,
    /// Tooltip number (team slot + 1), 0 = none.
    pub tip: i32,
    pub fields: bool,
    pub frames: Vec<i32>,
    pub end: bool,
}

/// Section 9 step 1: the box text of the shown op 270 messages (`msgs`, newest last): the newest one, after
/// message 0 when the newest is message 1 (the dialog's first two messages show together).
pub fn pay_box_text(msgs: &[i32]) -> Option<String> {
    let last = *msgs.last()?;
    let t = pay_text(last)?;
    if last == 1 && msgs.len() >= 2 && msgs[msgs.len() - 2] == 0 {
        return Some(format!("{} {}", pay_text(0)?, t));
    }
    Some(t.to_string())
}

/// Section 9 step 1: the question of op 275 pd_ask for a merc with nickname `nick` (family 1: a dead merc).
pub fn pay_ask_text(nick: &str, family: i32) -> String {
    if family != 0 {
        format!("Don't pay {}'s family?", nick)
    } else {
        format!("Don't pay {}?", nick)
    }
}

impl Play {
    /// Script mode, ending.md "Game end" steps 3, 4 after a game end with code `day.game_over`: the end show (lines
    /// `end show ...`, `end film ...`), the game-over screen (line `game over screen`; it waits for nothing here),
    /// then the new game (line `new game DIFFICULTY`) and the main menu (`day.to_menu`).
    pub fn game_end(&mut self, w: &mut World, iso: &str, exe: &str) -> Result<(), String> {
        let Some(code) = self.day.game_over else { return Ok(()) };
        if code == 1 {
            // End code 1: the win sequence, the rating text, then the program ends (`day.exit_text`).
            self.day.trace_line("win".into());
            let (v, score) = {
                let mut se = Session::new(w, &mut self.host);
                (se.win_vars(), se.performance_rating())
            };
            let (lines, pic) = crate::ending::script_win(iso, exe, v);
            self.day.lines.extend(lines);
            self.day.end_show = pic;
            let t = crate::ending::rating_text(score);
            self.day.lines.push(format!("rating {}", t));
            // gate_endings: the world at the rating, for the original's PerformanceRating? 0x62EB4 in Unicorn.
            if let Ok(path) = std::env::var("JA1_END_SAV") {
                self.day.lines.push(format!("win leave {}", w.update.leave_reason));
                self.day.lines.push(format!("win vars {} {} {} {}", v[0], v[1], v[2], v[3]));
                let _ = std::fs::write(path, w.save_bytes());
            }
            self.day.exit_text = Some(t);
            return Ok(());
        }
        if let Some(m) = crate::ending::show_member(code) {
            self.day.trace_line(format!("show {}", m));
        }
        let (lines, pic) = crate::ending::script_show(iso, exe, code);
        self.day.lines.extend(lines);
        self.day.end_show = pic;
        let mut buf = vec![0u8; 64000];
        let pal = crate::ending::game_over_picture(iso, exe, &mut buf).map_err(|c| format!("game over screen: error {}", c))?;
        self.day.lines.push("game over screen".into());
        self.day.trace_line("over".into());
        self.day.over_screen = Some((buf, crate::eod::scale_palette(&pal)));
        *w = crate::ending::new_game_after(iso, exe, w).map_err(|e| format!("new game: {}", e))?;
        self.day.lines.push(format!("new game {}", w.difficulty));
        self.day.trace_line(format!("new {} link {}", w.difficulty, w.accounts.get(0xA3).copied().unwrap_or(0)));
        self.day.to_menu = true;
        Ok(())
    }
}

/// The playing theme as TraverseMove? passes it to Traverse in: the theme word, 0xFFFF when no theme plays.
fn library_theme(theme: i32) -> i32 {
    if theme < 0 { 0xFFFF } else { theme }
}

/// Step 5: Traverse in's answer m != 0 changes the music to theme m; 0 keeps the playing theme.
fn music_change(m: i32) -> Option<i32> {
    (m != 0).then_some(m)
}

/// The tile of the selected merc (soldier +0x114), None when no merc is selected (selected >= 8).
fn selected_tile(w: &World) -> Option<i32> {
    let k = w.selected as usize;
    (k < 8).then(|| i16::from_le_bytes([w.soldiers[k * 512 + 0x114], w.soldiers[k * 512 + 0x115]]) as i32)
}

/// The sum of a palette's values (a short check value for the fade lines of a script).
fn pal_sum(p: &[u8; 768]) -> u32 {
    p.iter().map(|&v| v as u32).sum()
}

#[cfg(test)]
mod tests {
    /// load_moved: the view jumps to the selected merc's tile; with no merc selected (8) there is no jump.
    #[test]
    fn the_view_jump_needs_a_selected_merc() {
        let (exe, jag, save) = crate::world::tests::files(&[]);
        let mut w = crate::world::World::from_files(&exe, &jag, &save).unwrap();
        w.soldiers.iter_mut().for_each(|b| *b = 0);
        w.soldiers[3 * 512 + 0x114..3 * 512 + 0x116].copy_from_slice(&1234i16.to_le_bytes());
        w.soldiers[8 * 512 + 0x114..8 * 512 + 0x116].copy_from_slice(&999i16.to_le_bytes());
        w.selected = 3;
        assert_eq!(super::selected_tile(&w), Some(1234));
        w.selected = 8;
        assert_eq!(super::selected_tile(&w), None);
    }

    /// TraverseMove? passes the playing theme word (0xFFFF = none) and changes the music only for an answer m != 0.
    #[test]
    fn traverse_theme_word_and_change() {
        assert_eq!(super::library_theme(-1), 0xFFFF);
        assert_eq!(super::library_theme(3), 3);
        assert_eq!(super::library_theme(0), 0);
        assert_eq!(super::music_change(0), None);
        assert_eq!(super::music_change(4), Some(4));
    }

    /// A day replay wait is sound time: 25 ms = two 10 ms ticks and 5 ms left over.
    #[test]
    fn a_replay_wait_passes_sound_time() {
        let mut p = crate::play::Play::new(1);
        assert!(p.replay_wait_sound(25).is_empty());
        assert_eq!(p.host.sound.rest_us, 5_000);
        p.replay_wait_sound(7);
        assert_eq!(p.host.sound.rest_us, 2_000);
    }

    /// Section 9 step 1: message 0 shows before message 1 only; another newest message shows alone.
    #[test]
    fn only_message_1_takes_message_0_along() {
        assert_eq!(super::pay_box_text(&[0, 2]), super::pay_text(2).map(|t| t.to_string()));
        assert_eq!(super::pay_box_text(&[0, 1]), Some(format!("{} {}", super::pay_text(0).unwrap(), super::pay_text(1).unwrap())));
        assert_eq!(super::pay_box_text(&[1]), super::pay_text(1).map(|t| t.to_string()));
    }

    /// Ledger rows c2aea50:ja1front:8737/8744/8746: the corpse export question's rows (JA.EXE strings "It'll be a
    /// closed coffin for sure!" and the no-funds line).
    /// Ledger rows d057aca:ja1front:8672/8916/8925/8926/8930: the texts of the corpse export question and the "Not
    /// enough funds" dialog (JA.EXE strings "Dish out $AMOUN for corpse export?", "NOT ENOUGH FUNDS TO PAY ALL
    /// MERCENARIES!" ...); message 1 shows after message 0 only when 0 came right before it.
    #[test]
    fn corpse_and_pay_texts() {
        assert_eq!(super::corpse_text(1250), b"Dish out $1250 for corpse export?".to_vec());
        assert_eq!(super::pay_text(0), Some("NOT ENOUGH FUNDS TO PAY ALL MERCENARIES!"));
        assert_eq!(super::pay_text(2), Some("The native guide is helping you for free!"));
        assert_eq!(super::pay_text(3), None);
        let (m0, m1, m2) = (super::pay_text(0).unwrap(), super::pay_text(1).unwrap(), super::pay_text(2).unwrap());
        let t = |m: &[i32]| super::pay_box_text(m);
        assert_eq!(t(&[0, 1]), Some(format!("{m0} {m1}")));
        assert_eq!(t(&[1]), Some(m1.to_string()));
        assert_eq!(t(&[2, 1]), Some(m1.to_string()));
        assert_eq!(t(&[0, 2]), Some(m2.to_string()));
        assert_eq!(t(&[0, 0]), Some(m0.to_string()));
        assert_eq!(t(&[2, 2, 0, 2, 1]), Some(m1.to_string()));
        assert_eq!(t(&[2, 2, 2, 0, 1]), Some(format!("{m0} {m1}")));
        assert_eq!(t(&[]), None);
    }

    #[test]
    fn corpse_question_rows() {
        assert_eq!(super::corpse_rows(true), (8, 9, 10));
        assert_eq!(super::corpse_rows(false), (8, 11, 10));
    }

    /// Ledger row c2aea50:ja1front:8973: JA.EXE strings "Don't pay MEMBER 's family?" (a dead merc) and
    /// "Don't pay MEMBER ?".
    #[test]
    fn pay_question_names_the_family_of_a_dead_merc() {
        assert_eq!(super::pay_ask_text("Ivan", 1), "Don't pay Ivan's family?");
        assert_eq!(super::pay_ask_text("Ivan", 0), "Don't pay Ivan?");
    }

    /// Ledger row c2aea50:ja1front:8783: the OK face overlay sits at (5, 4) up and (4, 3) pressed.
    #[test]
    fn dayend_ok_overlay_places() {
        let (x, y) = super::dayend_ok_at(15);
        assert_eq!(super::dayend_ok_parts(15, false), [(0x66, x, y), (0x3A, x + 5, y + 4)]);
        assert_eq!(super::dayend_ok_parts(15, true), [(0x65, x, y), (0x3A, x + 4, y + 3)]);
    }

    /// Ledger row c2aea50:ja1front:8792: the nickname is profile bytes +0x00 up to the first 0 (at most 15).
    #[test]
    fn nickname_reads_the_profile() {
        let (exe, jag, save) = crate::world::tests::files(&[]);
        let mut w = crate::world::World::from_files(&exe, &jag, &save).unwrap();
        w.profiles[0x5A..0x5A + 6].copy_from_slice(b"Ivan\0x");
        assert_eq!(super::nickname(&w, 1), "Ivan");
        w.profiles[2 * 0x5A..2 * 0x5A + 16].copy_from_slice(b"ABCDEFGHIJKLMNOP");
        assert_eq!(super::nickname(&w, 2), "ABCDEFGHIJKLMNO");
    }
    /// Continue last day (LoadTacticalSector? with EAX 1) writes the same head values as a sector entry.
    #[test]
    fn sector_head_values() {
        let mut p = crate::play::Play::new(1);
        p.scroll = crate::viewmode::Scroll { dy: 1, dx: 1, hover: 5, delay: 9 };
        p.hover.lit = 3;
        p.slots.hover = 3;
        p.cursor.marker_tile = 7;
        p.cursor.over_map = 0;
        p.sector_head();
        assert_eq!((p.scroll.hover, p.scroll.delay, p.scroll.dx, p.scroll.dy), (200, 1, 0, 0));
        assert_eq!((p.hover.lit, p.slots.hover, p.cursor.marker_tile, p.cursor.over_map), (0x40, 0x40, 0xC41, 1));
    }
}

/// strategic.md "The day's verdict" step 5: the rating word of `score`, centred at (276, 161), font 0, colour 0xFF,
/// shadow 1, on the day replay picture `s`. False: the files did not load (the picture stays as it was).
pub fn rating_word_on(iso: &str, exe: &str, s: &mut [u8], score: i32) -> bool {
    let (Ok(c), Ok(d)) = (crate::common(iso), crate::panel::panel_data(iso, exe)) else { return false };
    let mut table0 = c.remap[0];
    let mut p = crate::panel::Painter { d: &d, buf: s, table0: &mut table0, table29: crate::panel::table29(&c) };
    p.text(rating_word(score).as_bytes(), 276, 161, true, 0xFF, 1, 0);
    true
}
