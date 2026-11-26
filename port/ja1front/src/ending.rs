//! The end of the game (ending.md "Game end"): the end show of an end code, the game-over screen and the new game
//! after it. The window and the script run the same steps; the window gives the show a `SabIo` with real waits.

use crate::sab::{LogIo, Sab, SabIo};
use crate::world::World;

/// The script of end code `code` in DAT\ENDSAB.DAT (step 3): 2 = LOS1GAME, 3 and above 4 = LOS2GAME, 4 = no show
/// (1 = the win sequence, not this table).
pub fn show_member(code: i32) -> Option<&'static str> {
    match code {
        2 => Some("LOS1GAME.SAB"),
        1 | 4 => None,
        _ => Some("LOS2GAME.SAB"),
    }
}

/// "Playing a script" for end code `code` with `io`, then all sounds stop and the music stops (step 3). Returns the
/// player (its screen and colours) and the script's result; None when the code has no show.
pub fn play_show(io: &mut dyn SabIo, code: i32) -> Option<(Sab, i32)> {
    let mem = show_member(code)?;
    let mut s = Sab::new();
    let r = s.play_script(io, "DAT\\ENDSAB.DAT", mem);
    io.samples_end();
    io.speech_stop();
    io.music_end();
    Some((s, r))
}

/// ending.md "The new-game intro": when the hard-disk file INTRO\SAT.DAT exists in `dir`, play its script
/// island.SAB with `io`. Returns the player and the script's result; None when the file is missing.
pub fn play_intro(io: &mut dyn SabIo, dir: &std::path::Path) -> Option<(Sab, i32)> {
    if !dir.join("INTRO").join("SAT.DAT").exists() {
        return None;
    }
    let mut s = Sab::new();
    let r = s.play_script(io, "INTRO\\SAT.DAT", "island.SAB");
    Some((s, r))
}

/// The game-over screen (ending.md "The game-over screen" steps 2, 3) into `buf` (320 x 200); its palette (6-bit).
pub fn game_over_picture(iso: &str, exe: &str, buf: &mut [u8]) -> Result<[u8; 768], i32> {
    let c = crate::common(iso)?;
    let d = crate::panel::panel_data(iso, exe)?;
    let od = crate::office::OfficeData::load(exe)?;
    let mut table0 = c.remap[0];
    let mut p = crate::panel::Painter { d: &d, buf, table0: &mut table0, table29: crate::panel::table29(&c) };
    Ok(crate::office::game_over(&od, &mut p))
}

/// Step 4: the new game after a game end: NewGameInit with the current difficulty (the quick-save mode stays), the
/// A.I.M. link byte 0, end code 0.
pub fn new_game_after(iso: &str, exe: &str, w: &World) -> std::io::Result<World> {
    let q = w.base.get(21285).copied().unwrap_or(0);
    // NewGameInit keeps the options, the sector order, the notes and the save name of the game before (world_load.md
    // section 11), as after Leave.
    let mut n = World::new_game_after(iso, exe, w.difficulty, q, Some(w))?;
    if let Some(l) = n.accounts.get_mut(0xA3) {
        *l = 0;
    }
    n.update.leave_reason = 0;
    // strategic.md "The day replay": no new game writes replay_carry (the original's byte is not in a save).
    n.replay_carry = w.replay_carry;
    Ok(n)
}

/// The script-mode end: the show with the test log's io (every wait ends at once, the films only log), summed up as
/// script lines `end show MEMBER RESULT steps N` and `end film NAME` per film. Returns the lines and the screen after
/// the last fade to full brightness with its colours (8-bit palette), None when the code has no show.
pub fn script_show(iso: &str, exe: &str, code: i32) -> (Vec<String>, Option<(Vec<u8>, [u8; 768])>) {
    let dir = std::path::Path::new(exe).parent().map(|p| p.to_path_buf()).unwrap_or_default();
    let mut io = LogIo { dir, iso: iso.to_string(), lines: Vec::new(), subtitles: false, draw: None, bright: None, films: false };
    let Some((s, r)) = play_show(&mut io, code) else { return (Vec::new(), None) };
    let steps = io.lines.iter().filter(|l| l.starts_with("step ")).count();
    let mut lines = vec![format!("end show {} {} steps {}", show_member(code).unwrap_or("-"), r, steps)];
    for l in io.lines.iter().filter(|l| l.starts_with("flc ")) {
        lines.push(format!("end film {}", l[4..].split(' ').next().unwrap_or("")));
    }
    let (scr, dac) = io.bright.take().unwrap_or((s.screen.clone(), s.dac));
    (lines, Some((scr, dac_rgb(&dac))))
}

/// The player's colours (RGBX 6-bit) as an 8-bit RGB palette.
pub fn dac_rgb(dac: &[u8; 1024]) -> [u8; 768] {
    let mut p = [0u8; 768];
    for i in 0..256 {
        for k in 0..3 {
            let v = dac[4 * i + k] & 0x3F;
            p[3 * i + k] = (v << 2) | (v >> 4);
        }
    }
    p
}

/// The page buffer P and the brightness buffer B of the credits (ending.md "The credits (after the win)").
struct Credits<'a> {
    io: &'a mut dyn SabIo,
    b: Vec<u8>,
    stop: bool,
}

impl Credits<'_> {
    /// An Escape key sets the stop flag (other keys are dropped here; SPEC GAP: the original leaves them waiting).
    fn esc(&mut self, s: &Sab) -> bool {
        if !self.stop && self.io.key(s) == Some(27) {
            self.stop = true;
        }
        self.stop
    }

    /// B to the screen at a vertical retrace.
    fn show(&mut self, s: &mut Sab) {
        s.screen.copy_from_slice(&self.b);
        self.io.retrace(s);
    }

    /// Fade to P: 16 frames, each byte one level toward 15 (P ≠ 0) or 0. The original looks for Escape before each
    /// byte; here once per frame (a stop drops the frame, which is never shown, so the screen is the same).
    fn fade_to(&mut self, s: &mut Sab, p: &[u8]) {
        for _ in 0..16 {
            if self.esc(s) {
                return;
            }
            for (b, &q) in self.b.iter_mut().zip(p) {
                let v = *b as i8;
                if q != 0 {
                    if v < 15 {
                        *b = (v + 1) as u8;
                    }
                } else if v > 0 {
                    *b = (v - 1) as u8;
                }
            }
            self.show(s);
        }
    }

    /// One scroll step: B up one row, `row` (320 bytes) into row 199, the top rows fade down, the bottom rows up.
    fn scroll(&mut self, s: &mut Sab, row: &[u8]) {
        self.b.copy_within(320.., 0);
        self.b[63680..].copy_from_slice(&row[..320]);
        for j in 0..4480 {
            if (self.b[j] as i8) > 1 {
                self.b[j] -= 1;
            }
            let k = 63680 - j;
            if self.b[k] != 0 {
                self.b[k] = self.b[k].wrapping_add(1);
            }
        }
        self.show(s);
    }

    /// A wait of `ms` (the window shows the screen meanwhile; the test log ends it at once).
    fn wait(&mut self, s: &Sab, ms: i64) {
        self.io.timer_start();
        while self.io.elapsed_ms() < ms {
            self.io.idle(s);
        }
    }
}

/// The credits after the win (ending.md "The credits (after the win)") on `s.screen` with `s.dac`. `theme_playing`:
/// a music theme plays (then theme 1 does not start). `draw(P, line, y)` draws a line into the page buffer P
/// (pitch 320) centred on x 159, colour 1, shadow 0, font 2. Returns true when Escape stopped them.
pub fn credits(io: &mut dyn SabIo, s: &mut Sab, theme_playing: bool, draw: &mut dyn FnMut(&mut [u8], &[u8], i32)) -> bool {
    let arc = io.file("DAT\\TITLE.DAT").unwrap_or_default();
    let bin = crate::sab::find_member(&arc, "credits.bin").and_then(|(o, n)| arc.get(o..o + n)).unwrap_or(&[]).to_vec();
    let pcx = crate::sab::find_member(&arc, "fadepal.pcx").and_then(|(o, n)| arc.get(o..o + n)).unwrap_or(&[]).to_vec();
    let count = |k: usize| bin.get(2 * k..2 * k + 2).map(|x| i16::from_le_bytes([x[0], x[1]]) as i32).unwrap_or(0);
    // Step 1: the text lines after the 15 numbers.
    let mut lines: Vec<&[u8]> = bin.get(30..).unwrap_or(&[]).split(|&c| c == 0).collect();
    lines.reverse();
    let mut next = move || lines.pop().unwrap_or(b"@");
    // Step 2.
    if !theme_playing {
        io.music(1, 1);
    }
    s.screen.fill(0);
    let mut slot = [0u8; 1024];
    if pcx.len() >= 768 {
        let p = &pcx[pcx.len() - 768..];
        for i in 0..256 {
            for c in 0..3 {
                slot[4 * i + c] = p[3 * i + c] >> 2;
            }
        }
    }
    s.dac = slot;
    let mut c = Credits { io, b: vec![0; 64000], stop: false };
    let mut p = vec![0u8; 64000];
    c.esc(s);
    // Step 3: the pages.
    let mut line = next();
    let mut k = 0;
    while line != b"@" && !c.stop {
        p.fill(0);
        let mut y = (195 - 10 * count(k)) / 2;
        while line != b"*" && line != b"@" && !c.stop {
            draw(&mut p, line, y);
            y += 10;
            line = next();
        }
        if c.esc(s) {
            c.io.sequence_volume(0, 3000);
            c.io.fade(s, slot, 56, 256, 0);
        }
        if line == b"*" {
            line = next();
        }
        k += 1;
        if c.stop {
            break;
        }
        c.fade_to(s, &p);
        for _ in 0..260 {
            if c.esc(s) {
                break;
            }
            c.wait(s, 5);
        }
    }
    // A stop before step 4 skips steps 4..7; a stop in the scroll ends only the scroll steps.
    if !c.stop {
        // Step 4.
        p.fill(0);
        c.fade_to(s, &p);
        c.b.fill(0);
        // Step 5: the scroll.
        loop {
            let line = next();
            if line.first() == Some(&b'@') || c.stop {
                break;
            }
            p[..11 * 320].fill(0);
            draw(&mut p, line, 0);
            for r in 0..11 {
                if c.esc(s) {
                    break;
                }
                let row = p[320 * r..320 * r + 320].to_vec();
                c.scroll(s, &row);
            }
        }
        // Step 6.
        p[..320].fill(0);
        let row = p[..320].to_vec();
        for _ in 0..100 {
            if c.esc(s) {
                break;
            }
            c.scroll(s, &row);
        }
        // Step 7.
        c.io.sequence_volume(0, 2000);
        c.io.wait_ms(s, 2000);
        c.io.fade(s, slot, 56, 256, 0);
    }
    // Step 8.
    let stop = c.stop;
    c.io.speech_stop();
    c.io.samples_end();
    c.io.music_end();
    s.screen.fill(0);
    stop
}

/// The title part at the program start (ending.md "The title part (program start)"): the sound archive FX\TITLE
/// and the graphics archive DAT\TITLE.DAT open, three films (sirtech.flc with no cues, a clear screen, title1.flc
/// with the cues of FX\TITLE.BIN at rate 22050 and voice volume 80, title2.flc with no cues), then the credits
/// (they start theme 1 only when no theme plays after the films). An Escape in a film stops only that film. Returns true when
/// Escape stopped the credits.
pub fn title(io: &mut dyn SabIo, s: &mut Sab, draw: &mut dyn FnMut(&mut [u8], &[u8], i32)) -> bool {
    title_films(io, s);
    let playing = io.theme() != -1;
    let stop = credits(io, s, playing, draw);
    s.set_sound_archive(None);
    stop
}

/// The films of `title` (the part before the credits).
pub fn title_films(io: &mut dyn SabIo, s: &mut Sab) {
    let snd = io.file("FX\\TITLE");
    s.set_sound_archive(snd);
    let gfx = io.file("DAT\\TITLE.DAT");
    s.set_graphics_archive(gfx);
    s.film_flag = false;
    s.cue_load(io, 0, 0, "");
    io.film(s, "sirtech.flc", 0, 0, 1, 1);
    s.screen.fill(0);
    s.cue_load(io, 22050, 80, "FX\\TITLE.BIN");
    io.film(s, "title1.flc", 0, 0, 1, 1);
    s.cue_load(io, 0, 0, "");
    io.film(s, "title2.flc", 0, 0, 1, 1);
}

/// The script-mode title part (`title`): lines `title film NAME frames N` per film, `title voice RATE VOL` per cue
/// voice, `title music T` per theme start (in order), `title credits frames N stop S`. The music module plays those
/// themes and ends stopped (music.md). Also returns the first frame of the last film (title2.flc).
pub fn script_title(iso: &str, exe: &str) -> (Vec<String>, (Vec<u8>, [u8; 768])) {
    let dir = std::path::Path::new(exe).parent().map(|p| p.to_path_buf()).unwrap_or_default();
    let mut io = LogIo { dir, iso: iso.to_string(), lines: Vec::new(), subtitles: false, draw: None, bright: None, films: true };
    let mut s = Sab::new();
    title_films(&mut io, &mut s);
    let pic = io.bright.take().map_or_else(|| (s.screen.clone(), dac_rgb(&s.dac)), |(scr, dac)| (scr, dac_rgb(&dac)));
    let playing = io.theme() != -1;
    let n0 = io.lines.len();
    let stop = credits(&mut io, &mut s, playing, &mut credits_draw(iso, exe));
    s.set_sound_archive(None);
    let mut lines = Vec::new();
    let (mut frames, mut film) = (0, "");
    for (i, l) in io.lines.iter().enumerate() {
        let w: Vec<&str> = l.split(' ').collect();
        match w[0] {
            "flc" => film = w[1],
            "film_frames" => lines.push(format!("title film {} frames {}", film, w[1])),
            "speech" => lines.push(format!("title voice {} {}", w[1], w[2])),
            "music" => {
                lines.push(format!("title music {}", w[1]));
                crate::music::with(|m| m.play(w[1].parse().unwrap_or(-1), w[2] == "1"));
            }
            "frame" if i >= n0 => frames += 1,
            _ => {}
        }
    }
    lines.push(format!("title credits frames {} stop {}", frames, stop as i32));
    crate::music::with(|m| m.stop_theme());
    (lines, pic)
}

/// The credits' line drawing (step 3) with the panel fonts: centred on x 159 at `y`, colour 1, shadow 0, font 2.
pub fn credits_draw(iso: &str, exe: &str) -> impl FnMut(&mut [u8], &[u8], i32) {
    let (c, d) = (crate::common(iso).ok(), crate::panel::panel_data(iso, exe).ok());
    move |buf, line, y| {
        let (Some(c), Some(d)) = (&c, &d) else { return };
        let mut t0 = c.remap[0];
        let mut p = crate::panel::Painter { d, buf, table0: &mut t0, table29: crate::panel::table29(c) };
        p.text(line, 159, y, true, 1, 0, 2);
    }
}

/// The rating text of score `s` (ending.md "The performance rating").
pub fn rating_text(s: i32) -> String {
    const W: [&str; 7] = ["TERRIBLE", "LOUSY", "POOR", "FAIR", "GOOD", "EXCELLENT", "OUTSTANDING"];
    let k = [30, 50, 60, 70, 80, 90].iter().take_while(|&&t| s >= t).count();
    format!("Your overall performance rating is {} ({}%)", W[k], s)
}

/// The win sequence (ending.md "Game end", end code 1, win sequence steps 2..5) with variables `v` through `io`:
/// the music ends, WINGAME.SAB (and INTRO05.FLC when it ran to its end), the credits, the music volume 0 over
/// 3000 ms, the sounds stop. Returns the script's result and whether Escape stopped the credits.
pub fn win(io: &mut dyn SabIo, s: &mut Sab, v: [i16; 4], draw: &mut dyn FnMut(&mut [u8], &[u8], i32)) -> (i32, bool) {
    io.music_end();
    let r = s.win(io, v);
    let stop = credits(io, s, false, draw);
    io.sequence_volume(0, 3000);
    io.speech_stop();
    io.samples_end();
    (r, stop)
}

/// The script-mode win: lines `end show WINGAME.SAB RESULT steps N`, `end film NAME` per film, `credits frames N
/// stop S`; the show picture as in `script_show`.
pub fn script_win(iso: &str, exe: &str, v: [i16; 4]) -> (Vec<String>, Option<(Vec<u8>, [u8; 768])>) {
    let dir = std::path::Path::new(exe).parent().map(|p| p.to_path_buf()).unwrap_or_default();
    let mut io = LogIo { dir, iso: iso.to_string(), lines: Vec::new(), subtitles: false, draw: None, bright: None, films: false };
    let mut s = Sab::new();
    let r = s.win(&mut io, v);
    let steps = io.lines.iter().filter(|l| l.starts_with("step ")).count();
    let mut lines = vec![format!("end show WINGAME.SAB {} steps {}", r, steps)];
    for l in io.lines.iter().filter(|l| l.starts_with("flc ")) {
        lines.push(format!("end film {}", l[4..].split(' ').next().unwrap_or("")));
    }
    let (scr, dac) = io.bright.take().unwrap_or((s.screen.clone(), s.dac));
    let n0 = io.lines.len();
    let stop = credits(&mut io, &mut s, false, &mut credits_draw(iso, exe));
    let frames = io.lines[n0..].iter().filter(|l| l.starts_with("frame ")).count();
    lines.push(format!("credits frames {} stop {}", frames, stop as i32));
    (lines, Some((scr, dac_rgb(&dac))))
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A clock that moves 1 ms per read.
    struct Clock(i64, i64);
    impl SabIo for Clock {
        fn file(&mut self, _name: &str) -> Option<Vec<u8>> {
            None
        }
        fn elapsed_ms(&mut self) -> i64 {
            self.0 += 1;
            self.0 - 1
        }
        fn timer_start(&mut self) {
            self.0 = 0;
            self.1 += 1;
        }
        fn fade(&mut self, _s: &mut Sab, _pal: [u8; 1024], _steps: i32, _from: i32, _to: i32) {}
        fn film(&mut self, _s: &mut Sab, _name: &str, _x: i32, _y: i32, _flag: i32, _loops: i32) {}
    }

    struct Keys(Vec<i8>);
    impl SabIo for Keys {
        fn file(&mut self, _name: &str) -> Option<Vec<u8>> {
            None
        }
        fn elapsed_ms(&mut self) -> i64 {
            0
        }
        fn timer_start(&mut self) {}
        fn fade(&mut self, _s: &mut Sab, _pal: [u8; 1024], _steps: i32, _from: i32, _to: i32) {}
        fn film(&mut self, _s: &mut Sab, _name: &str, _x: i32, _y: i32, _flag: i32, _loops: i32) {}
        fn key(&mut self, _s: &Sab) -> Option<i8> {
            if self.0.is_empty() { None } else { Some(self.0.remove(0)) }
        }
    }

    /// The credits stop at an Escape key; another key does not stop them, and after the stop no key is read.
    #[test]
    fn credits_stop_at_escape() {
        let mut io = Keys(vec![65, 27, 65]);
        let s = Sab::new();
        let mut c = Credits { io: &mut io, b: Vec::new(), stop: false };
        assert!(!c.esc(&s));
        assert!(c.esc(&s));
        assert!(c.esc(&s));
        assert_eq!(io.0, vec![65]);
    }

    /// ending.md "The credits (after the win)": the page hold is 260 waits of 5 ms (1.3 s), so one wait reads the
    /// clock until 5 ms passed.
    #[test]
    fn credits_wait_holds_until_the_time_passed() {
        let mut io = Clock(0, 0);
        let s = Sab::new();
        let mut c = Credits { io: &mut io, b: Vec::new(), stop: false };
        c.wait(&s, 5);
        assert_eq!((io.0, io.1), (6, 1), "the wait reads the clock at 0..5 ms after one timer start");
    }

    /// "Game end" step 4: NewGameInit after the game over keeps the switches, the notes and the save name of the game
    /// before (as after Leave), and the driver bytes of InitSound (once per program).
    #[test]
    fn new_game_after_game_over_keeps_the_switches() {
        let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
        let (iso, exe) = (root.join("rom/JA1.gog"), root.join("build/game113b/JA.EXE"));
        if !iso.exists() || !exe.exists() {
            return;
        }
        let (iso, exe) = (iso.to_str().unwrap(), exe.to_str().unwrap());
        let mut w = World::new_game(iso, exe, 2, 1).unwrap();
        let fresh = (crate::ctlpanel::get(&w, 4), crate::ctlpanel::get(&w, 5));
        crate::ctlpanel::set(&mut w, 4, 1 - fresh.0.min(1));
        crate::ctlpanel::set(&mut w, 5, 1 - fresh.1.min(1));
        w.sound_setup.sound_driver = false;
        w.sound_setup.music_driver = false;
        w.walter_sector = 5; // 1.13b NewGameInit does not write 0x92E2D (no day save holds it): it stays
        let n = new_game_after(iso, exe, &w).unwrap();
        assert_eq!(n.walter_sector, 5);
        assert_eq!((crate::ctlpanel::get(&n, 4), crate::ctlpanel::get(&n, 5)), (1 - fresh.0.min(1), 1 - fresh.1.min(1)));
        assert!(!n.sound_setup.sound_driver && !n.sound_setup.music_driver);
        assert_eq!((n.difficulty, n.accounts[0xA3]), (2, 0));
    }

    /// Ledger rows c2aea50:ja1front:9465/9466/9467/9469: the window's win sequence plays WINGAME.SAB to its end
    /// (result 1, 5451 steps as gate_endings) and then the credits (1201 frames, not stopped).
    #[test]
    fn the_win_plays_the_show_and_the_credits() {
        let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
        let (iso, exe) = (root.join("rom/JA1.gog"), root.join("build/game113b/JA.EXE"));
        if !iso.exists() || !exe.exists() {
            return;
        }
        let (iso, exe) = (iso.to_str().unwrap(), exe.to_str().unwrap());
        let dir = std::path::Path::new(exe).parent().unwrap().to_path_buf();
        let mut io = LogIo { dir, iso: iso.to_string(), lines: Vec::new(), subtitles: false, draw: None, bright: None, films: false };
        let mut s = Sab::new();
        let (r, stop) = win(&mut io, &mut s, [0; 4], &mut credits_draw(iso, exe));
        let count = |p: &str| io.lines.iter().filter(|l| l.starts_with(p)).count();
        assert_eq!((r, stop), (1, false));
        assert_eq!(count("step "), 5451);
        assert!(count("frame ") >= 1201, "{}", count("frame "));
    }
}
