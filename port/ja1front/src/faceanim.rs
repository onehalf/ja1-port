//! panel.md "Face animation (eyes and mouth)": the blinking eyes and the moving mouth of a face while it waits for
//! its speech (speech balloon, merc-face box) and the blink of the A.I.M. face.
//!
//! Port note: the original draws these random numbers from the game's generator at times that depend on real time;
//! the port gives each animation its own generator, so the game's numbers stay the same however long a face shows.

/// The face sheet: 148 x 69 (face 56 x 50 at (0, 0), eye blocks at x 56, mouth blocks at x 102).
pub const SHEET_W: usize = 148;
pub const FACE_W: usize = 56;
pub const FACE_H: usize = 50;

/// Eye step: state `e` (0 .. 12), talking flag `d`, mode `b` (1 = the hurt face). `r(n)` = a random number 0 .. n - 1.
pub fn eye_step(e: i32, d: bool, b: i32, r: &mut dyn FnMut(i32) -> i32) -> i32 {
    match e {
        0 => {
            if r(2) != 0 && d && b != 1 {
                3
            } else {
                1
            }
        }
        1 => {
            if b == 1 {
                1
            } else {
                2
            }
        }
        2 => {
            if (r(2) != 0 && d) || b == 2 {
                3
            } else {
                0
            }
        }
        3..=6 | 8..=11 => e + 1,
        7 => {
            if r(2) != 0 && b != 1 {
                8
            } else {
                0
            }
        }
        12 => 0,
        _ => e,
    }
}

/// Mouth step: state `m` (0 .. 3); `silent` = the voice is in a silence.
pub fn mouth_step(m: i32, silent: bool, r: &mut dyn FnMut(i32) -> i32) -> i32 {
    if silent || !(0..=3).contains(&m) {
        return if silent { 0 } else { m };
    }
    loop {
        let v = r(4);
        let ok = match m {
            0 => v == 1,
            1 => v != 1,
            2 => v == 1 || v == 3,
            _ => v == 1 || v == 2,
        };
        if ok {
            return v;
        }
    }
}

/// The sheet block (x, y) of eye frame of state `e` (46 x 23).
pub fn eye_src(e: i32) -> (usize, usize) {
    let f = e.min(3);
    if f <= 0 {
        (5, 5)
    } else {
        (56, 23 * (f as usize - 1))
    }
}

/// The sheet block (x, y) of the mouth frame of state `m` (46 x 20).
pub fn mouth_src(m: i32) -> (usize, usize) {
    if m <= 0 {
        (5, 30)
    } else {
        (102, 20 * (m as usize - 1))
    }
}

fn block(sheet: &[u8], (sx, sy): (usize, usize), w: usize, h: usize, dst: &mut [u8], pitch: usize, x: usize, y: usize) {
    for j in 0..h {
        let s = (sy + j) * SHEET_W + sx;
        let d = (y + j) * pitch + x;
        if s + w <= sheet.len() && d + w <= dst.len() {
            dst[d..d + w].copy_from_slice(&sheet[s..s + w]);
        }
    }
}

/// The eye frame of `e` to `dst` (pitch `pitch`) at (x, y) = the face's (5, 5).
pub fn draw_eyes(sheet: &[u8], e: i32, dst: &mut [u8], pitch: usize, x: usize, y: usize) {
    block(sheet, eye_src(e), 46, 23, dst, pitch, x, y);
}

/// The mouth frame of `m` to `dst` at (x, y) = the face's (5, 30).
pub fn draw_mouth(sheet: &[u8], m: i32, dst: &mut [u8], pitch: usize, x: usize, y: usize) {
    block(sheet, mouth_src(m), 46, 20, dst, pitch, x, y);
}

/// The 56 x 50 face of the sheet with the eye frame of `e` and the mouth frame of `m`.
pub fn face(sheet: &[u8], e: i32, m: i32) -> Vec<u8> {
    let mut f = vec![0u8; FACE_W * FACE_H];
    block(sheet, (0, 0), FACE_W, FACE_H, &mut f, FACE_W, 0, 0);
    draw_eyes(sheet, e, &mut f, FACE_W, 5, 5);
    draw_mouth(sheet, m, &mut f, FACE_W, 5, 30);
    f
}

/// The silences of a voice (8-bit unsigned samples, 22050 Hz): (start ms, end ms) pairs, at most 100.
pub fn silences(pcm: &[u8]) -> Vec<(u32, u32)> {
    let mut out = Vec::new();
    let (mut p, mut run, mut start) = (0u32, 0u16, 0u32);
    for &s in pcm {
        if (0x7B..=0x85).contains(&s) {
            if run == 0 {
                start = p / 1000;
            }
            run = run.wrapping_add(1);
        } else {
            if run >= 1100 && out.len() < 100 {
                out.push((start, p / 1000));
            }
            run = 0;
        }
        p = p.wrapping_add(45);
    }
    out
}

/// The lip-sync clock: one `tick` per 2500 µs of playback.
#[derive(Clone, Default)]
pub struct LipClock {
    pub t: u32,
    pub i: usize,
    pub silent: bool,
}

impl LipClock {
    pub fn tick(&mut self, sil: &[(u32, u32)]) {
        self.t = self.t.wrapping_add(2500);
        let ms = self.t / 1000;
        if self.silent {
            if ms > sil[self.i].1 {
                self.silent = false;
                self.i += 1;
            }
        } else if self.i < sil.len() && ms >= sil[self.i].0 {
            self.silent = true;
        }
    }
}

/// A small generator for the animation (not the game's numbers; see the port note above).
#[derive(Clone)]
pub struct AnimRng(u32);

impl AnimRng {
    pub fn new(seed: u32) -> AnimRng {
        AnimRng(seed | 1)
    }
    pub fn random(&mut self, n: i32) -> i32 {
        self.0 = self.0.wrapping_mul(1103515245).wrapping_add(12345);
        if n <= 0 {
            0
        } else {
            ((self.0 >> 16) % n as u32) as i32
        }
    }
}

/// The animation of one wait (panel.md "Face animation", "The wait"); times in ms.
pub struct FaceAnim {
    pub e: i32,
    pub m: i32,
    pub b: i32,
    pub talking: bool,
    te: i64,
    tm: i64,
    eye_t: i64,
    mouth_t: i64,
    pub rng: AnimRng,
}

impl FaceAnim {
    /// Step 1 at time `now`; `b` = 1 for the hurt face (merc message with quote 9).
    pub fn start(b: i32, now: i64, mut rng: AnimRng) -> FaceAnim {
        let tm = rng.random(150) as i64 + 25;
        let te = rng.random(1000) as i64 + 500;
        FaceAnim { e: if b == 1 { 1 } else { 0 }, m: 0, b, talking: true, te, tm, eye_t: now, mouth_t: now, rng }
    }

    /// One pass at time `now`: `talking` = the voice (or the text time) still runs, `silent` = the lip-sync clock.
    /// The answer: the face must be drawn again.
    pub fn pass(&mut self, now: i64, talking: bool, silent: bool) -> bool {
        let mut redraw = false;
        if now - self.eye_t >= self.te {
            let mut rng = self.rng.clone();
            self.e = eye_step(self.e, self.talking, self.b, &mut |n| rng.random(n));
            self.eye_t = now;
            redraw = true;
            self.te = if self.e == 0 && !self.talking { rng.random(2000) + 1000 } else { rng.random(300) + 100 } as i64;
            self.rng = rng;
        }
        if !self.talking {
            if self.m != 0 {
                self.m = 0;
                redraw = true;
            }
        } else {
            self.talking = talking;
            if self.talking && now - self.mouth_t >= self.tm {
                let mut rng = self.rng.clone();
                self.m = mouth_step(self.m, silent, &mut |n| rng.random(n));
                self.mouth_t = now;
                self.tm = rng.random(100) as i64 + 50;
                self.rng = rng;
                redraw = true;
            }
        }
        redraw
    }
}

/// The A.I.M. face blink (panel.md "Face animation", A.I.M. screen).
pub struct AimBlink {
    pub e: i32,
    te: i64,
    t: i64,
    pub rng: AnimRng,
}

impl AimBlink {
    /// The opening: eye state 0 and the eye time `te` (R(1000) + 500, drawn with the game's numbers).
    pub fn new(te: i64, now: i64, rng: AnimRng) -> AimBlink {
        AimBlink { e: 0, te, t: now, rng }
    }

    /// One pass with the mouse off the face: the answer = the eye frame changed.
    pub fn pass(&mut self, now: i64) -> bool {
        if now - self.t < self.te {
            return false;
        }
        let mut rng = self.rng.clone();
        self.e = eye_step(self.e, false, 0, &mut |n| rng.random(n));
        self.t = now;
        self.te = if self.e == 0 { rng.random(2000) + 1000 } else { rng.random(300) + 100 } as i64;
        self.rng = rng;
        true
    }
}

/// The lip-sync clock over one voice: `at(us)` = in a silence after `us` µs of playback.
pub struct LipSync {
    sil: Vec<(u32, u32)>,
    clock: LipClock,
    ticks: u64,
    released: bool,
}

impl LipSync {
    pub fn new(pcm: &[u8]) -> LipSync {
        LipSync { sil: silences(pcm), clock: LipClock::default(), ticks: 0, released: false }
    }
    /// ReleaseTacticalTimer? 0x66D94: the timer stops (0x284F1E = 0) and the silence byte 0x337C9F goes to 0: `at` answers false.
    pub fn release(&mut self) {
        self.released = true;
    }
    pub fn at(&mut self, us: u64) -> bool {
        if self.released {
            return false;
        }
        while self.ticks < us / 2500 {
            self.clock.tick(&self.sil);
            self.ticks += 1;
        }
        self.clock.silent
    }
}

/// A face on the screen that waits for its speech: the sheet, the screen point of its top-left pixel and the mode b.
#[derive(Clone)]
pub struct FaceSpot {
    pub sheet: Vec<u8>,
    pub x: i32,
    pub y: i32,
    pub b: i32,
}

impl FaceSpot {
    /// The face rectangle of the wait (SpeechWait's MouseInRect: one pixel up and left of the face, 57 x 51, the
    /// ends included).
    pub fn wait_rect_has(&self, x: i32, y: i32) -> bool {
        (self.x - 1..=self.x + 55).contains(&x) && (self.y - 1..=self.y + 49).contains(&y)
    }

    /// The face with eye state `e` and mouth state `m` onto the 320 x 200 screen `buf` (clipped).
    pub fn paint(&self, buf: &mut [u8], e: i32, m: i32) {
        let f = face(&self.sheet, e, m);
        for j in 0..FACE_H as i32 {
            for i in 0..FACE_W as i32 {
                let (x, y) = (self.x + i, self.y + j);
                if (0..320).contains(&x) && (0..200).contains(&y) {
                    buf[(y * 320 + x) as usize] = f[j as usize * FACE_W + i as usize];
                }
            }
        }
    }
}

/// The window's animated wait of one face: `step` answers a new screen when the face changed.
pub struct WaitFace {
    pub spot: FaceSpot,
    pub anim: FaceAnim,
    lip: Option<LipSync>,
    pub redraws: u32,
}

impl WaitFace {
    /// `voice` = the samples of the voice that plays (None: no voice).
    pub fn new(spot: FaceSpot, voice: Option<&[u8]>, seed: u32) -> WaitFace {
        let anim = FaceAnim::start(spot.b, 0, AnimRng::new(seed));
        WaitFace { spot, anim, lip: voice.map(LipSync::new), redraws: 0 }
    }

    /// One pass `now` ms after the wait started; `talking` as FaceAnim::pass, `played_us` = the voice's playback
    /// time. `frame` = the screen of the wait.
    pub fn step(&mut self, frame: &[u8], now: i64, talking: bool, played_us: u64) -> Option<Vec<u8>> {
        if !self.pass(now, talking, played_us) {
            return None;
        }
        let mut f = frame.to_vec();
        self.spot.paint(&mut f, self.anim.e, self.anim.m);
        Some(f)
    }

    /// As `pass` with the silence given.
    pub fn pass_silent(&mut self, now: i64, talking: bool, silent: bool) -> bool {
        if !self.anim.pass(now, talking, silent) {
            return false;
        }
        self.redraws += 1;
        true
    }

    /// As `step` without a screen: true when the face changed (`anim.e` / `anim.m` hold its frames).
    pub fn pass(&mut self, now: i64, talking: bool, played_us: u64) -> bool {
        if !talking {
            if let Some(l) = self.lip.as_mut() {
                l.release();
            }
        }
        let silent = self.lip.as_mut().is_some_and(|l| l.at(played_us));
        if !self.anim.pass(now, talking, silent) {
            return false;
        }
        self.redraws += 1;
        true
    }
}

/// How a speech wait ended (SpeechWait? 0x28034).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum WaitEnd {
    /// The talk ended and the hold time passed (never with Subtitles on: only the player ends that wait).
    Hold = 1,
    /// The right button: the sighting does not move the view (byte 0x92F65).
    Right = 2,
    /// The left button on the face (byte 0x92F64).
    LeftFace = 3,
    /// The left button off the face, more than 1000 ms into the wait's timer.
    LeftLate = 4,
    /// Enter, Escape or Space.
    Key = 5,
}

/// The fixed part of one wait (the arguments of SpeechWait?).
#[derive(Clone, Copy, Debug, Default)]
pub struct WaitRules {
    /// The text time (EBX): the talk ends when more ms than this passed. Without a voice only.
    pub text_ms: i64,
    /// A voice plays: its end ends the talk.
    pub voiced: bool,
    /// The time after the talk (stack arg): the wait's timer starts again at the talk's end.
    pub hold: i64,
    /// Subtitles on (byte 0x1163BD).
    pub subtitles: bool,
    /// The wait has a text to show on the face.
    pub has_text: bool,
}

/// One pass's input: the time since the wait started, the mouse (button levels), a key that ends the wait, the voice.
#[derive(Clone, Copy, Debug, Default)]
pub struct WaitInput {
    pub now: i64,
    pub x: i32,
    pub y: i32,
    pub left: bool,
    pub right: bool,
    pub key: bool,
    pub playing: bool,
    pub played_us: u64,
    /// The test's silence byte; None = the voice's lip-sync clock.
    pub silent: Option<bool>,
}

impl WaitInput {
    /// A key or either mouse button in this pass: the end of a wait without a face (SpeechWait? 0x28682).
    pub fn pressed(&self) -> bool {
        self.key || self.left || self.right
    }
}

/// What a pass did: the face must be drawn (with the text when `text`), or the wait ended.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct WaitStep {
    pub redraw: bool,
    pub text: bool,
    pub end: Option<WaitEnd>,
}

/// SpeechWait? 0x28034: the rules of one wait over a `WaitFace`.
pub struct SpeechWait {
    pub face: WaitFace,
    rules: WaitRules,
    /// The start of the wait's timer ([esp] in the original).
    timer: i64,
    hold_on: bool,
    /// The last redraw was on the face ([esp + 0x40]).
    hover: bool,
}

/// The keys that end the wait (0x28182 ..).
pub fn wait_key_ends(k: u8) -> bool {
    matches!(k, 13 | 27 | 32)
}

impl SpeechWait {
    pub fn new(face: WaitFace, rules: WaitRules) -> SpeechWait {
        SpeechWait { face, rules, timer: 0, hold_on: false, hover: false }
    }

    /// The time the hold after the talk started (the wait's timer), None while the face talks or with no hold.
    pub fn quiet_since(&self) -> Option<i64> {
        self.hold_on.then_some(self.timer)
    }

    /// One pass of the loop at 0x28178.
    pub fn pass(&mut self, i: &WaitInput) -> WaitStep {
        let r = self.rules;
        let talk = if r.voiced { i.playing } else { i.now - self.timer <= r.text_ms };
        let mut redraw = match i.silent {
            Some(q) => self.face.pass_silent(i.now, talk, q),
            None => self.face.pass(i.now, talk, i.played_us),
        };
        let mut step = WaitStep::default();
        if !r.subtitles && !self.face.anim.talking {
            if self.hold_on || r.hold == 0 {
                if i.now - self.timer > r.hold {
                    step.end = Some(WaitEnd::Hold);
                    return step;
                }
            } else {
                self.hold_on = true;
                self.timer = i.now;
            }
        }
        if i.right {
            step.end = Some(WaitEnd::Right);
            return step;
        }
        if self.face.spot.wait_rect_has(i.x, i.y) {
            if r.has_text && !self.hover {
                redraw = true;
            }
            if redraw {
                self.hover = true;
                step.text = r.has_text;
            }
            step.redraw = redraw;
            if i.left {
                step.end = Some(WaitEnd::LeftFace);
            }
        } else {
            if self.hover {
                redraw = true;
                self.hover = false;
            }
            step.redraw = redraw;
            if i.left && i.now - self.timer > 1000 {
                step.end = Some(WaitEnd::LeftLate);
            }
        }
        if step.end.is_none() && i.key {
            step.end = Some(WaitEnd::Key);
        }
        step
    }
}

#[cfg(test)]
mod tests {
    /// Ledger row c2aea50:ja1front:2355: a key, a left click or a right click alone ends a wait without a face.
    #[test]
    fn one_key_or_button_ends_a_plain_wait() {
        let w = |key, left, right| super::WaitInput { key, left, right, ..Default::default() }.pressed();
        assert!(w(true, false, false));
        assert!(w(false, true, false));
        assert!(w(false, false, true));
        assert!(!w(false, false, false));
    }

    use super::*;

    /// Ledger rows c2aea50:ja1front:9801/9807: the A.I.M. face blinks only after its eye time since the last change;
    /// an open eye waits R(2000) + 1000 ms, a closing eye R(300) + 100 ms.
    #[test]
    fn the_aim_face_blinks_after_its_eye_time() {
        let mut b = AimBlink::new(500, 1000, AnimRng::new(7));
        assert!(!b.pass(1499));
        assert!(b.pass(1500));
        let mut now = 1500;
        for _ in 0..200 {
            let (e, te) = (b.e, b.te);
            if e == 0 {
                assert!((1000..3000).contains(&te), "open {te}");
            } else {
                assert!((100..400).contains(&te), "closing {te}");
            }
            assert!(!b.pass(now + te - 1));
            now += te;
            assert!(b.pass(now));
        }
    }

    #[test]
    fn a_quiet_face_blinks() {
        let mut z = |_n: i32| 1;
        let mut e = 0;
        let mut seen = vec![e];
        for _ in 0..3 {
            e = eye_step(e, false, 0, &mut z);
            seen.push(e);
        }
        assert_eq!(seen, vec![0, 1, 2, 0]);
    }

    #[test]
    fn a_long_quiet_run_is_a_silence() {
        let mut pcm = vec![0xF0u8; 100];
        pcm.extend(vec![0x80u8; 1200]);
        pcm.push(0x10);
        assert_eq!(silences(&pcm), vec![(4, (1300 * 45) / 1000)]);
    }

    /// The silence scan 0x66DB8 keeps at most 100 silences (count 0x337970: cmp 0x64, jge at 0x66E26).
    #[test]
    fn at_most_100_silences() {
        let mut pcm = Vec::new();
        for _ in 0..102 {
            pcm.extend(vec![0x80u8; 1100]);
            pcm.push(0x10);
        }
        assert_eq!(silences(&pcm).len(), 100);
    }

    /// The lip timer 0x66CC4 ticks every 2.5 ms of playback: after `us` µs exactly us / 2500 ticks ran. A silence
    /// from 4 ms is seen at the second tick (5 ms), not before.
    #[test]
    fn the_lip_clock_ticks_every_2500_us() {
        let mut pcm = vec![0xF0u8; 100];
        pcm.extend(vec![0x80u8; 1200]);
        pcm.push(0x10);
        let mut l = LipSync::new(&pcm);
        assert!(!l.at(4999));
        assert!(l.at(5000));
    }

    /// A face drawn near the right edge is clipped: no pixel wraps to the next row.
    #[test]
    fn a_face_at_the_right_edge_is_clipped() {
        let spot = FaceSpot { sheet: vec![7u8; 148 * 69], x: 300, y: 10, b: 0 };
        let mut buf = vec![0u8; 320 * 200];
        spot.paint(&mut buf, 0, 0);
        assert_eq!(buf[10 * 320 + 300], 7);
        assert_eq!(buf[10 * 320 + 319], 7);
        assert_eq!(buf[11 * 320], 0);
    }

    /// WaitFace: the lip sync stops (ReleaseTacticalTimer? 0x66D94) only when the voice no longer runs, and a
    /// talking face changes within its first second.
    #[test]
    fn the_wait_face_moves_and_stops_its_lip_clock_at_the_end() {
        let spot = FaceSpot { sheet: vec![7u8; 148 * 69], x: 0, y: 0, b: 0 };
        let pcm = vec![0xF0u8; 22050];
        let mut w = WaitFace::new(spot, Some(&pcm), 3);
        let changed = (1..=100).filter(|&t| w.pass(t * 10, true, t as u64 * 10_000)).count();
        assert!(changed > 0);
        assert!(!w.lip.as_ref().unwrap().released);
        w.pass(1010, false, 1_010_000);
        assert!(w.lip.as_ref().unwrap().released);
    }
}
