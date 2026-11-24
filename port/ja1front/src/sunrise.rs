//! The sunrise screen (docs/spec/strategic.md "The sunrise screen", host op 304): the sunrise picture fades in, its
//! colours cycle while "Day N", the funds and the new arrivals appear, the helicopter flies in when mercs arrive,
//! bird calls play (drawing the game's random numbers), then the picture fades out.
//! SunriseScreen 0x2A268; sunrise_screen_diff --port compares every frame and sound.
use crate::common;
use crate::eod::{rotate, scale_palette, sunset, EodSink};
use crate::host::Rng;
use crate::panel::{self, Painter};
use crate::play::PlayHost;

/// The helicopter's x per frame (strategic.md "The sunrise screen" step 5).
const HELI_X: [i32; 32] = [
    0, 0, 0, 0, 10, 20, 30, 40, 50, 60, 70, 80, 90, 95, 100, 105, 109, 112, 115, 117, 119, 120, 121, 123, 124, 125,
    126, 127, 128, 129, 129, 129,
];
const HELI_Y: usize = 89;

/// The screen's front-end state between op 304 calls.
pub struct Sunrise {
    iso: String,
    exe: String,
    table0: [u8; 256],
    /// The window (None in a play script: nothing waits).
    pub sink: Option<Box<dyn EodSink>>,
    /// Script mode: the poll number that skips (`sunrise N`).
    pub poll_at: Option<u32>,
    /// The last frame shown before the fade out: S and the 8-bit palette.
    pub last: Option<(Vec<u8>, [u8; 768])>,
    /// Script lines: `sunrise sound NAME` per sound started, the `sunrise frames ..` line at the end.
    pub lines: Vec<String>,
    /// Test log (sunrise_screen_diff): every frame and sound; no sound plays while it is on.
    pub log: Option<RunLog>,
}

/// What `Sunrise::run` showed and played, in order.
#[derive(Debug, Default)]
pub struct RunLog {
    /// Each frame: the screen and the 6-bit palette.
    pub frames: Vec<(Vec<u8>, [u8; 768])>,
    /// Each sound: [effect, rate, volume, loops, frames shown before it]; a stop: [-1, effect, 0, 0, frames].
    pub sounds: Vec<[i32; 5]>,
}

/// What one run did (the stderr line).
#[derive(Debug, Default, PartialEq)]
pub struct Report {
    pub frames: u32,
    pub texts: u32,
    pub birds: u32,
    pub heli: u32,
    pub skip: bool,
}

/// The three texts: "Day N", the funds and the new arrivals (empty when none).
pub fn texts(day: i32, balance: i32, arrivals: i16) -> [Vec<u8>; 3] {
    let a = match arrivals {
        1 => "1 New Arrival".to_string(),
        n if n > 1 => format!("{} New Arrivals", n),
        _ => String::new(),
    };
    [format!("Day {}", day).into_bytes(), format!("Current Funds : $ {}", balance).into_bytes(), a.into_bytes()]
}

/// A MAIN.DAT member of the game files.
fn member(exe: &str, name: &str) -> Option<Vec<u8>> {
    let main = std::path::Path::new(exe).parent()?.join("DAT").join("MAIN.DAT");
    panel::archive_member(&std::fs::read(main).ok()?, name).ok()
}

/// HELIN6.PCX: 1280 x 40 pixels and its 6-bit palette (None when missing or of another size).
fn heli_sheet(f: Option<&[u8]>) -> Option<(Vec<u8>, [u8; 768])> {
    let f = f?;
    let (w, h, pix) = panel::pcx(f).ok()?;
    if w != 1280 || h < 40 || f.len() < 769 || f[f.len() - 769] != 0x0C {
        return None;
    }
    let mut pal = [0u8; 768];
    for (i, v) in f[f.len() - 768..].iter().enumerate() {
        pal[i] = v >> 2;
    }
    Some((pix[..1280 * 40].to_vec(), pal))
}

/// Palette `w` at level `l` of `n` (each value * l / n, truncated).
pub fn level(w: &[u8; 768], l: u32, n: u32) -> [u8; 768] {
    let mut d = [0u8; 768];
    for i in 0..768 {
        d[i] = (w[i] as u32 * l / n) as u8;
    }
    d
}

/// A bird call (step 4): four draws 5, 2, 15, 4; effect random(4) + 0x13 at 11025.
fn bird(rng: &mut Rng, sound: bool, lines: &mut Vec<String>, log: &mut Option<RunLog>) -> bool {
    if rng.random(5) != 0 {
        return false;
    }
    let loops = rng.random(2) + 1;
    let vol = rng.random(15) + 10;
    let fx = rng.random(4) + 0x13;
    lines.push(format!("sunrise sound {}", crate::sound::main_name(fx)));
    if let Some(l) = log.as_mut() {
        let n = l.frames.len() as i32;
        l.sounds.push([fx, 0x2B11, vol, loops, n]);
    }
    if sound {
        crate::sound::screen(|s| s.sfx(fx, 0x2B11, vol, loops));
    }
    true
}

impl Sunrise {
    pub fn new(iso: &str, exe: &str, table0: [u8; 256]) -> Sunrise {
        Sunrise { iso: iso.to_string(), exe: exe.to_string(), table0, sink: None, poll_at: None, last: None, lines: Vec::new(), log: None }
    }

    fn show(&mut self, s: &[u8], d: &[u8; 768], retrace: bool) {
        if let Some(l) = self.log.as_mut() {
            l.frames.push((s.to_vec(), *d));
        }
        if let Some(k) = self.sink.as_mut() {
            k.show(s, &scale_palette(d), retrace);
        }
    }

    /// A sound of the screen that a later stop ends: effect `fx`, volume 50, once (step 1, step 3).
    fn start(&mut self, fx: i32, rate: i32, sound: bool, chans: &mut Vec<(usize, i32)>) {
        if let Some(l) = self.log.as_mut() {
            let n = l.frames.len() as i32;
            l.sounds.push([fx, rate, 50, 0, n]);
            chans.push((usize::MAX, fx));
        }
        if sound {
            if let Some(k) = crate::sound::screen(|s| s.sfx_chan(fx, rate, 50, 0)) {
                chans.push((k, fx));
            }
            self.lines.push(format!("sunrise sound {}", crate::sound::main_name(fx)));
        }
    }

    /// One poll: the window's input, or the script's poll number.
    fn poll(&mut self, n: &mut u32) -> bool {
        let k = *n;
        *n += 1;
        match self.sink.as_mut() {
            Some(s) => s.poll(),
            None => self.poll_at == Some(k),
        }
    }

    /// The whole screen for `day` (already + 1), `balance` and `arrivals`; bird calls draw from `rng`.
    pub fn run(&mut self, day: i32, balance: i32, arrivals: i16, rng: &mut Rng) -> Report {
        let mut rep = Report::default();
        let sound = !self.iso.is_empty() && std::path::Path::new(&self.exe).exists() && self.log.is_none();
        // Step 1: main bank, the music stops, the morning sound.
        let mut chans = Vec::new();
        if sound {
            // (The music stopped before: Session::sunrise.)
            crate::sound::screen_main(&self.iso, &self.exe);
        }
        self.start(0x12, 0x2B11, sound, &mut chans);
        let (mut p, mut w) = sunset(member(&self.exe, "SUNRISE.PCX").as_deref());
        let heli = heli_sheet(member(&self.exe, "HELIN6.PCX").as_deref());
        let mut s = p.clone();
        let t = texts(day, balance, arrivals);
        let d_data = panel::panel_data(&self.iso, &self.exe).ok();
        let com = common(&self.iso).ok();
        let mut table0 = self.table0;
        let mut draw = |buf: &mut Vec<u8>, k: usize, rep: &mut Report| {
            rep.texts += 1;
            let (Some(d), Some(c)) = (d_data.as_ref(), com.as_ref()) else { return };
            let t29 = *panel::table29(c);
            let mut pt = Painter { d, buf, table0: &mut table0, table29: &t29 };
            pt.text(&t[k], 159, [64, 79, 94][k], true, 0xFF, 1, 2);
        };
        // Step 2: fade in.
        for l in 0..=56 {
            let d = level(&w, l, 56);
            self.show(&s, &d, true);
        }
        // SunriseScreen 0x2A5FB: the helicopter sound starts after the fade in, not with the morning sound.
        if arrivals > 0 {
            self.start(0x10, 10000, sound, &mut chans);
        }
        // Step 4: the colour cycle.
        let mut polls = 0;
        let mut skip = false;
        let mut span: Option<(std::time::Instant, std::time::Instant)> = None;
        for f in 0..90 {
            // strategic.md "The sunrise screen" step 4: a clock wait of 1 ms first (one game clock step)
            if let Some(k) = self.sink.as_mut() {
                k.clock_wait(1);
            }
            rotate(&mut w, 3, 254);
            self.show(&s, &w, true);
            rep.frames += 1;
            if self.sink.is_some() {
                let now = std::time::Instant::now();
                span = Some((span.map_or(now, |(a, _)| a), now));
            }
            match f {
                10 => draw(&mut s, 0, &mut rep),
                35 => draw(&mut s, 1, &mut rep),
                75 => draw(&mut s, 2, &mut rep),
                _ => {}
            }
            if self.poll(&mut polls) {
                for k in 0..3 {
                    draw(&mut s, k, &mut rep);
                }
                if let Some((_, hp)) = heli.as_ref() {
                    w = *hp;
                }
                self.show(&s, &w, true);
                skip = true;
                break;
            }
            if f % 4 == 0 && bird(rng, sound, &mut self.lines, &mut self.log) {
                rep.birds += 1;
            }
        }
        // JA1_SPEED_LOG=1 (world_load.md "Game speed"): the colour cycle's time from the first to the last frame
        if let Some((a, b)) = span {
            if std::env::var("JA1_SPEED_LOG").is_ok_and(|v| v == "1") {
                eprintln!("speed sunrise {} ms {} frames", (b - a).as_millis(), rep.frames);
            }
        }
        // Step 5: the helicopter.
        if arrivals > 0 {
            let mut b = vec![0u8; 40 * 40];
            // step 5: the waits read the game clock (crate::dostime), counted from the end of the previous frame
            let mut t0 = crate::dostime::now_ms();
            for i in 0..32 {
                let at = |x: i32, r: usize| HELI_Y * 320 + r * 320 + x as usize;
                if i > 0 {
                    for r in 0..40 {
                        p[at(HELI_X[i - 1], r)..at(HELI_X[i - 1], r) + 40].copy_from_slice(&b[r * 40..r * 40 + 40]);
                    }
                }
                for r in 0..40 {
                    b[r * 40..r * 40 + 40].copy_from_slice(&p[at(HELI_X[i], r)..at(HELI_X[i], r) + 40]);
                    let src = heli.as_ref().map(|h| &h.0[r * 1280 + 40 * i..r * 1280 + 40 * i + 40]);
                    p[at(HELI_X[i], r)..at(HELI_X[i], r) + 40].copy_from_slice(src.unwrap_or(&[0; 40]));
                }
                for k in 0..3 {
                    draw(&mut p, k, &mut rep);
                }
                s[29 * 320..132 * 320].copy_from_slice(&p[29 * 320..132 * 320]);
                self.show(&s, &w, false);
                rep.heli += 1;
                let wait = match i {
                    0..=14 => 105,
                    15..=19 => 110,
                    20..=24 => 115,
                    25..=28 => 120,
                    _ => 125,
                };
                let mut hit = false;
                if self.sink.is_some() {
                    while heli_waits(crate::dostime::now_ms(), t0, wait) {
                        if self.poll(&mut polls) {
                            hit = true;
                            break;
                        }
                        if let Some(k) = self.sink.as_mut() {
                            k.delay(5);
                        }
                    }
                } else {
                    hit = self.poll(&mut polls);
                }
                if i % 4 == 0 && bird(rng, sound, &mut self.lines, &mut self.log) {
                    rep.birds += 1;
                }
                if hit {
                    skip = true;
                    break;
                }
                t0 = crate::dostime::now_ms();
            }
        }
        rep.skip = skip;
        self.last = Some((s.clone(), scale_palette(&w)));
        // Step 6: the two sounds stop; fade out.
        for (k, fx) in chans {
            if let Some(l) = self.log.as_mut() {
                let n = l.frames.len() as i32;
                l.sounds.push([-1, fx, 0, 0, n]);
            }
            if k != usize::MAX {
                crate::sound::screen(|x| x.chan_stop(k));
            }
        }
        for l in (0..=24).rev() {
            let d = level(&w, l, 24);
            self.show(&s, &d, true);
        }
        rep
    }
}

/// Host op 304 (world_load.md section 9 step 3): the screen with the world's day, balance and arrivals.
pub fn host_op(h: &mut PlayHost) -> Option<Report> {
    if h.world.is_null() {
        return None;
    }
    // SAFETY: `world` is the view of the running library call; `accounts` holds 0xA8 bytes.
    let (day, bal, arr) = unsafe {
        let w = &*h.world;
        if w.accounts.is_null() {
            return None;
        }
        let a = std::slice::from_raw_parts(w.accounts as *const u8, 0xA8);
        (w.day, i32::from_le_bytes([a[0x24], a[0x25], a[0x26], a[0x27]]), i16::from_le_bytes([a[0x8C], a[0x8D]]))
    };
    // Script line `framefree` (test only): the bird calls run per frame, and a click can skip them.
    let main_seed = h.frame_free.map(|f| std::mem::replace(&mut h.rng.seed, f));
    let r = h.sunrise.as_mut().map(|s| s.run(day, bal, arr, &mut h.rng));
    if let Some(seed) = main_seed {
        h.frame_free = Some(std::mem::replace(&mut h.rng.seed, seed));
    }
    if let (Some(r), Some(s)) = (r.as_ref(), h.sunrise.as_mut()) {
        let l = format!("sunrise frames {} texts {} birds {} heli {} skip {}", r.frames, r.texts, r.birds, r.heli, r.skip as i32);
        eprintln!("{}", l);
        s.lines.push(l);
    }
    r
}

/// Step 5: the helicopter frame waits while less than `wait` ms passed since the end of the previous frame (`t0`).
fn heli_waits(now: i64, t0: i64, wait: i64) -> bool {
    now - t0 < wait
}

#[cfg(test)]
mod tests {

    /// Ledger rows ja1front:9975, c2aea50:ja1front:10493: HELIN6.PCX must be 1280 wide and at least 40 rows high,
    /// with its palette.
    #[test]
    fn heli_sheet_size() {
        let pcx = |w: u16, h: u16| {
            let mut d = vec![0u8; 128];
            (d[0], d[1], d[2], d[3], d[0x41]) = (10, 5, 1, 8, 1);
            d[8..10].copy_from_slice(&(w - 1).to_le_bytes());
            d[10..12].copy_from_slice(&(h - 1).to_le_bytes());
            d[0x42..0x44].copy_from_slice(&w.to_le_bytes());
            d.extend(vec![1u8; w as usize * h as usize]);
            d.push(0x0C);
            d.extend([4u8; 768]);
            d
        };
        assert!(super::heli_sheet(Some(&pcx(1280, 40))).is_some());
        assert!(super::heli_sheet(Some(&pcx(1280, 41))).is_some());
        assert!(super::heli_sheet(Some(&pcx(1280, 39))).is_none());
        assert!(super::heli_sheet(Some(&pcx(1279, 40))).is_none());
    }
    /// Ledger row d057aca:ja1front:10597: a helicopter frame stays for its wait, counted from the previous frame's end.
    #[test]
    fn a_helicopter_frame_waits_from_the_previous_frame() {
        assert!(super::heli_waits(1_000_104, 1_000_000, 105));
        assert!(!super::heli_waits(1_000_105, 1_000_000, 105));
        assert!(!super::heli_waits(1_000_300, 1_000_000, 125));
    }

    use super::*;

    #[test]
    fn sunrise_texts() {
        let t = texts(5, -20, 1);
        assert_eq!(t[0], b"Day 5");
        assert_eq!(t[1], b"Current Funds : $ -20");
        assert_eq!(t[2], b"1 New Arrival");
        assert_eq!(texts(1, 0, 3)[2], b"3 New Arrivals");
        assert!(texts(1, 0, 0)[2].is_empty());
        assert!(texts(1, 0, -1)[2].is_empty());
    }

    #[test]
    fn sunrise_random_draws_and_skip() {
        // No files: the frames run; every 4th colour-cycle frame draws random(5) (+3 on a bird call).
        let mut s = Sunrise::new("", "/nonexistent/JA.EXE", [0; 256]);
        let mut rng = Rng::new(7);
        let mut check = Rng::new(7);
        let r = s.run(2, 100, 0, &mut rng);
        assert_eq!((r.frames, r.texts, r.heli, r.skip), (90, 3, 0, false));
        let mut birds = 0;
        for _ in 0..23 {
            if check.random(5) == 0 {
                birds += 1;
                for n in [2, 15, 4] {
                    check.random(n);
                }
            }
        }
        assert_eq!((r.birds, rng.seed, rng.draws), (birds, check.seed, check.draws));
        // A skip at poll 3: frames 0..3, three texts more, no bird call in frame 3; then the helicopter (poll 4 on).
        let mut s = Sunrise::new("", "/nonexistent/JA.EXE", [0; 256]);
        s.poll_at = Some(3);
        let r = s.run(2, 100, 2, &mut Rng::new(1));
        assert_eq!((r.frames, r.texts, r.heli, r.skip), (4, 3 + 32 * 3, 32, true));
        s.poll_at = Some(95);
        let r = s.run(2, 100, 2, &mut Rng::new(1));
        assert_eq!((r.frames, r.heli, r.skip), (90, 6, true));
    }
}
