//! The SAB script player (ending.md): the end shows WINGAME.SAB, LOS1GAME.SAB and LOS2GAME.SAB of DAT\ENDSAB.DAT.
//! The player owns its state (variables, screen, work buffers, sprites, palette slots, flags); everything outside
//! (files, keys, the timer, fades, films, music, sounds) goes through `SabIo`, so the same player runs in the
//! window and in the `--sab` test log (validate/diff/sab_diff.py).

/// Outside calls of the player (ending.md "Outside calls and the test log").
/// Ops 24 / 25 (fade): the DAC at level `l` of `n`, each colour (R, G, B) of `pal` · l / n.
pub fn fade_dac(dac: &mut [u8; 1024], pal: &[u8; 1024], l: i32, n: i32) {
    for (d, &c) in dac.iter_mut().zip(pal.iter()) {
        *d = (c as i32 * l / n) as u8;
    }
}

pub trait SabIo {
    /// The bytes of an archive file (hard disk, else CD); None when it cannot be opened.
    fn file(&mut self, name: &str) -> Option<Vec<u8>>;
    /// A CD file read whole (op 59's cue table; the test log writes `open_cd NAME`).
    fn cd_file(&mut self, name: &str) -> Option<Vec<u8>> {
        self.file(name)
    }
    /// Step 2 of the step loop (the test log writes its `step` line here).
    fn step(&mut self, _s: &Sab) {}
    /// A waiting key (its character code as a signed byte), or None.
    fn key(&mut self, _s: &Sab) -> Option<i8> {
        None
    }
    /// Milliseconds since the last timer start.
    fn elapsed_ms(&mut self) -> i64;
    fn timer_start(&mut self);
    /// WaitMs? 0x67F48: the screen stays until `ms` passed, a key or a click (the window); the answer is not used here.
    fn wait_ms(&mut self, s: &Sab, ms: i64) {
        self.timer_start();
        while self.elapsed_ms() < ms && self.key(s).is_none() {
            self.idle(s);
        }
    }
    /// One pass of a wait loop (the window shows the screen and sleeps a little).
    fn idle(&mut self, _s: &Sab) {}
    /// The screen was copied at a vertical retrace (the credits; the window shows one 1/70 s frame).
    fn retrace(&mut self, _s: &Sab) {}
    /// Fade of palette slot `pal` (1024 bytes RGBX, 6-bit) over `steps` levels from `from`/256 to `to`/256.
    fn fade(&mut self, s: &mut Sab, pal: [u8; 1024], steps: i32, from: i32, to: i32);
    /// Op 8: colours `first`..`last` were set from `src` (4 bytes each; the test log writes a `palette` line).
    fn palette(&mut self, _s: &Sab, _first: usize, _last: usize, _src: &[u8]) {}
    /// Op 30: caption `text` drawn centred at (159, y) on the screen (font 2, colour 0xFF, shadow 1).
    fn draw_caption(&mut self, _screen: &mut [u8], _text: &[u8], _y: i32) {}
    /// Film member `name` of the graphics archive at (x, y) (ending.md "Films").
    fn film(&mut self, s: &mut Sab, name: &str, x: i32, y: i32, flag: i32, loops: i32);
    /// `Sab::play_film`: wait until `after_ms` from `start` (the film timer of FlcPlay 0x5BC58).
    fn film_wait(&mut self, _start: std::time::Instant, _after_ms: u64) {}
    /// `Sab::play_film`: the key of FilmCallback 0x5BE18 after frame counter `j` was read.
    fn film_key(&mut self, _s: &Sab, _j: i32) -> Option<i8> {
        None
    }
    /// `Sab::play_film`: the frame is complete.
    fn film_show(&mut self, _s: &Sab) {}
    fn music_load(&mut self, _name: &str) {}
    fn music(&mut self, _theme: i32, _stop_first: i32) {}
    fn sequence_volume(&mut self, _v: i32, _ms: i32) {}
    fn music_end(&mut self) {}
    /// The playing music theme (-1 = none).
    fn theme(&self) -> i32 {
        -1
    }
    fn sfx_load(&mut self, _name: &str, _slot: i32, _data: Option<&[u8]>) {}
    fn sfx_play(&mut self, _slot: i32, _rate: i32, _vol: u8, _loops: i32) {}
    fn speech(&mut self, _rate: i32, _vol: i32, _data: &[u8]) {}
    fn speaking(&self) -> bool {
        false
    }
    fn speech_stop(&mut self) {}
    /// The speech sample ends without the logged speech stop (Escape in a wait).
    fn speech_end_quiet(&mut self) {}
    fn samples_end(&mut self) {}
    fn voc_stop(&mut self) {}
    fn speech_feed(&mut self) {}
    /// The Subtitles option (op 29).
    fn subtitles(&self) -> bool {
        false
    }
    /// Record q of EDT\INTQUOTE.EDT (200 bytes).
    fn int_quote(&mut self, _q: i32) -> Vec<u8> {
        Vec::new()
    }
    /// Op 29 text drawing (font 2, ending.md op 29 step 3) on the screen.
    fn draw_subtitle(&mut self, _screen: &mut [u8], _text: &[u8], _fg: u8, _shadow: u8) {}
}

#[derive(Clone, Copy, Default)]
struct Sprite {
    buf: usize,
    off: i64,
    w: i16,
    h: i16,
    pitch: i64,
    has_place: bool,
    px: i16,
    py: i16,
}

/// The memory sizes of work buffers 0..3 (ending.md "Player state").
pub const BUF_MEM: [usize; 4] = [64000, 96960, 51200, 96000];
const BUF_START: [(u16, u16); 4] = [(320, 200), (320, 303), (320, 160), (640, 150)];
const REC: usize = 44;

/// An open script archive: the file, the member start and the file position.
struct Script {
    data: Vec<u8>,
    start: usize,
    pos: usize,
    member: String,
}

/// What one film cue step did (ending.md "Film cues").
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Cue {
    None,
    Effect(i32),
    Clear,
    /// A voice name entry that is empty.
    NoVoice,
    /// The voice name and whether the sound archive holds it.
    Voice(String, bool),
    /// The quote and its colour.
    Subtitle(i32, u8),
    Music(i32),
}

pub struct Sab {
    pub vars: [i16; 26],
    pub screen: Vec<u8>,
    pub bufs: [Vec<u8>; 4],
    pub size: [(u16, u16); 4],
    sprites: Vec<Sprite>,
    pub slots: [[u8; 1024]; 4],
    /// The colours shown (RGBX 6-bit), set by the fades and the films.
    pub dac: [u8; 1024],
    /// The DAC copy 0xEDA08 (RGBX 6-bit): the current tile palette, which the game loads (LoadTilePalette
    /// 0x2A990) and op 0x35 reads. The player owns no tiles, so the game front-end seeds it where it must.
    pub dac_copy: [u8; 1024],
    key_colour: i16,
    centre: bool,
    copy_flag: i16,
    rect: [i16; 4],
    cond: bool,
    saved_cond: bool,
    in_call: bool,
    ret_pos: usize,
    last_key: i8,
    pub abort: bool,
    loop_flag: bool,
    speech_vol: i32,
    /// "Any key stops the film" (the win sequence sets it around INTRO05).
    pub film_flag: bool,
    /// Film cues (ending.md op 59, "Film cues"): the cue rate, the cue voice volume, the 200 bytes of the cue table +
    /// the 120 bytes of the cue voice names, the cue index i and the cue voice index j.
    pub cue_rate: i32,
    pub cue_vol: i32,
    pub cues: [u8; 320],
    pub cue_i: usize,
    pub cue_j: usize,
    script: Option<Script>,
    /// Op 30's caption count (0 at program start).
    caption: u8,
    gfx: Option<Vec<u8>>,
    snd: Option<Vec<u8>>,
    voice: (usize, usize),
    last_pal: [u8; 768],
    text_buf: String,
    rec: [u8; REC],
}

impl Default for Sab {
    fn default() -> Self {
        Sab::new()
    }
}

fn i16at(b: &[u8], p: usize) -> i16 {
    i16::from_le_bytes([b[p], b[p + 1]])
}

fn u32at(b: &[u8], p: usize) -> Option<usize> {
    b.get(p..p + 4).map(|x| u32::from_le_bytes([x[0], x[1], x[2], x[3]]) as usize)
}

/// (offset, size) of member `name` of a Sir-tech archive (speech.md "Archives"): the original's ArcSeek 0x534A4.
/// It uppercases the wanted name and leaves the directory as it is, so a lower-case directory name never matches. The
/// records of 0x20 bytes run from index 1 to count - 1 (index 0 is the header). The compare is a strcmp, so a 16-byte
/// name runs on into the offset bytes, and an empty wanted name finds the first empty record.
pub fn find_member(arc: &[u8], name: &str) -> Option<(usize, usize)> {
    let n = u32at(arc, 0x14)? as u32 as i32;
    let want = name.to_ascii_uppercase();
    for i in 1..n.max(1) as usize {
        let e = 0x20 * i;
        let raw = arc.get(e..)?;
        let len = raw.iter().position(|&b| b == 0).unwrap_or(raw.len());
        if &raw[..len] == want.as_bytes() {
            return Some((u32at(arc, e + 16)?, u32at(arc, e + 20)?));
        }
    }
    None
}

fn member<'a>(arc: Option<&'a Vec<u8>>, name: &str) -> Option<&'a [u8]> {
    let arc = arc?;
    let (o, s) = find_member(arc, name)?;
    arc.get(o..o + s)
}

/// Loads PCX `data` into `buf` at offset `o` with pitch `p` (ending.md "PCX pictures"); writes outside the buffer
/// memory are dropped. The original's 0x532BC (ArcSeek + ReadPcx) loads the member and decodes it.
pub fn pcx_into(data: &[u8], buf: &mut [u8], o: i64, p: i64) {
    if data.len() < 128 {
        return;
    }
    let u = |a: usize| u16::from_le_bytes([data[a], data[a + 1]]) as i64;
    let (rows, last, bpl) = (u(10) - u(6) + 1, u(8) - u(4), u(0x42));
    let (mut x, mut r) = (0i64, 0i64);
    let mut i = 128;
    if rows <= 0 || bpl <= 0 {
        return;
    }
    while i < data.len() {
        let b = data[i];
        i += 1;
        let (mut n, v) = if b >= 0xC0 {
            let v = data.get(i).copied().unwrap_or(0);
            i += 1;
            (b as i32 - 0xC0, v)
        } else {
            (1, b)
        };
        loop {
            if x <= last {
                let a = o + r * p + x;
                if a >= 0 && (a as usize) < buf.len() {
                    buf[a as usize] = v;
                }
            }
            x += 1;
            if x == bpl {
                r += 1;
                x = 0;
                if r == rows {
                    return;
                }
                break;
            }
            n -= 1;
            if n == 0 {
                break;
            }
        }
    }
}

impl Sab {
    /// The program-start state.
    pub fn new() -> Sab {
        Sab {
            vars: [0; 26],
            screen: vec![0; 64000],
            bufs: [vec![0; BUF_MEM[0]], vec![0; BUF_MEM[1]], vec![0; BUF_MEM[2]], vec![0; BUF_MEM[3]]],
            size: BUF_START,
            sprites: vec![Sprite::default(); 100],
            slots: [[0; 1024]; 4],
            dac: [0; 1024],
            dac_copy: [0; 1024],
            key_colour: 0,
            centre: false,
            copy_flag: 1,
            rect: [0, 0, 320, 200],
            cond: true,
            saved_cond: true,
            in_call: false,
            ret_pos: 0,
            last_key: 0,
            abort: false,
            loop_flag: false,
            speech_vol: 90,
            film_flag: false,
            cue_rate: 11025,
            cue_vol: 0,
            cues: [0; 320],
            cue_i: 0,
            cue_j: 0,
            script: None,
            caption: 0,
            gfx: None,
            snd: None,
            voice: (0, 0),
            last_pal: [0; 768],
            text_buf: String::new(),
            rec: [0; REC],
        }
    }

    pub fn op(&self) -> u16 {
        u16::from_le_bytes([self.rec[0], self.rec[1]])
    }

    pub fn a0(&self) -> i16 {
        i16at(&self.rec, 2)
    }

    fn set_a0(&mut self, v: i16) {
        self.rec[2..4].copy_from_slice(&v.to_le_bytes());
    }

    /// Word wk, k = 1..10.
    fn w(&self, k: usize) -> i16 {
        i16at(&self.rec, 0x18 + 2 * (k - 1))
    }

    fn set_w(&mut self, k: usize, v: i16) {
        let p = 0x18 + 2 * (k - 1);
        self.rec[p..p + 2].copy_from_slice(&v.to_le_bytes());
    }

    fn name(&self) -> String {
        let n = &self.rec[4..22];
        n[..n.iter().position(|&b| b == 0).unwrap_or(18)].iter().map(|&b| b as char).collect()
    }

    fn var(&self, v: i16) -> i16 {
        if v > 9999 {
            self.vars.get((v as i32 - 10000) as usize).copied().unwrap_or(0)
        } else {
            v
        }
    }

    /// The CRC-32 of the screen and the first 64000 bytes of work buffer 1, as the test log writes it.
    pub fn checksum(&self) -> u32 {
        crc32(crc32(0, &self.screen), &self.bufs[1][..64000])
    }

    /// Reads the next record; false at the end of the file (the record stays as it was).
    fn read(&mut self) -> bool {
        if let Some(s) = &mut self.script {
            if let Some(r) = s.data.get(s.pos..s.pos + REC) {
                self.rec.copy_from_slice(r);
                s.pos += REC;
                return true;
            }
            s.pos = s.data.len();
        }
        false
    }

    fn pos(&self) -> usize {
        self.script.as_ref().map_or(0, |s| s.pos)
    }

    fn set_pos(&mut self, p: usize) {
        if let Some(s) = &mut self.script {
            s.pos = p;
        }
    }

    /// Seed the DAC copy 0xEDA08 (the tile palette). The game front-end passes the loaded tileset palette; the
    /// test player (sab_diff --op35) passes a known pattern.
    pub fn set_dac_copy(&mut self, d: &[u8]) {
        let n = d.len().min(1024);
        self.dac_copy[..n].copy_from_slice(&d[..n]);
    }

    /// "Play a script" (archive, member): abort flag = 0, timer start, open, play; the result (1 = ran to the end).
    pub fn play_script(&mut self, io: &mut dyn SabIo, archive: &str, mem: &str) -> i32 {
        self.abort = false;
        io.timer_start();
        if !self.open(io, archive, mem) {
            return 0;
        }
        self.play(io)
    }

    /// Open (ending.md "Opening a script"); false when the archive or the member is missing.
    pub fn open(&mut self, io: &mut dyn SabIo, archive: &str, mem: &str) -> bool {
        let Some(data) = io.file(archive) else { return false };
        let Some((start, _)) = find_member(&data, mem) else { return false };
        self.script = Some(Script { data, start, pos: start, member: mem.to_string() });
        self.read();
        for v in &mut self.vars[4..] {
            *v = 0;
        }
        true
    }

    fn close(&mut self, io: &mut dyn SabIo) {
        io.speech_stop();
        io.samples_end();
        io.voc_stop();
        self.script = None;
    }

    /// Play (ending.md "The step loop").
    pub fn play(&mut self, io: &mut dyn SabIo) -> i32 {
        self.cond = true;
        self.last_key = 0;
        self.loop_flag = false;
        loop {
            io.step(self);
            if (self.op() as i16) <= 16 {
                self.resolve();
            }
            self.run(io);
            if let Some(k) = io.key(self) {
                self.last_key = k;
            }
            if self.abort {
                io.music_end();
                self.close(io);
                return 0;
            }
            if self.last_key != 0 {
                if self.last_key == 27 {
                    self.abort = true;
                    self.close(io);
                    return 0;
                }
                self.last_key = 0;
            }
            if !self.cond {
                while self.op() != 19 && self.op() != 43 {
                    if !self.read() {
                        break;
                    }
                }
            }
            self.cond = true;
            if !self.read() {
                // Not reached by the end scripts (the original would run the last record again and again).
                self.close(io);
                return 0;
            }
            io.speech_feed();
            if self.op() == 99 {
                let start = self.script.as_ref().map_or(0, |s| s.start);
                self.set_pos(start);
                self.read();
                if !self.loop_flag {
                    self.close(io);
                    return 1;
                }
            }
        }
    }

    /// Operands (ending.md "Operands").
    fn resolve(&mut self) {
        let a0 = self.a0();
        if a0 > 9999 {
            if self.op() <= 7 {
                self.set_a0(a0 - 10000);
            } else {
                self.set_a0(self.var(a0));
            }
        }
        for k in 1..=10 {
            let v = self.var(self.w(k));
            self.set_w(k, v);
        }
    }

    /// A wait of t ms (ending.md "Waits"); false when Escape ended it. `op36` = the Escape rule of op 36.
    fn wait(&mut self, io: &mut dyn SabIo, t: i32, op36: bool) -> bool {
        while io.elapsed_ms() < t as i64 {
            if let Some(k) = io.key(self) {
                self.last_key = k;
                if k == 27 {
                    if !op36 || io.speaking() {
                        io.speech_end_quiet();
                    }
                    return false;
                }
                self.last_key = 0;
            }
            io.speech_feed();
            io.idle(self);
        }
        true
    }

    fn copy0to1(&mut self) {
        let (a, b) = self.bufs.split_at_mut(1);
        b[0][..64000].copy_from_slice(&a[0][..64000]);
    }

    fn draw_sprite(&mut self, n: usize, x: i16, y: i16, key: bool) {
        let s = self.sprites[n];
        let w1 = self.size[1].0 as i64;
        for j in 0..s.h as i64 {
            for i in 0..s.w as i64 {
                let src = s.off + j * s.pitch + i;
                let c = if src >= 0 { self.bufs[s.buf].get(src as usize).copied().unwrap_or(0) } else { 0 };
                if key && c == self.key_colour as u8 {
                    continue;
                }
                let d = (y as i64 + j) * w1 + x as i64 + i;
                if d >= 0 && (d as usize) < BUF_MEM[1] {
                    self.bufs[1][d as usize] = c;
                }
            }
        }
    }

    fn compare(c: i16, l: i16, r: i16, seven: bool) -> bool {
        match c {
            1 => l == r,
            2 => l > r,
            3 => l < r,
            4 => l != r,
            5 => l >= r,
            6 => l <= r,
            7 if seven => r != 0 && l.wrapping_rem(r) == 0,
            _ => false,
        }
    }

    /// Runs the current record's op (ending.md "Ops").
    fn run(&mut self, io: &mut dyn SabIo) {
        let op = self.op();
        let a0 = self.a0();
        match op {
            1..=6 => {
                let n = (a0 as i32).rem_euclid(10000) as usize;
                let (v, w1) = (self.vars.get(n).copied().unwrap_or(0), self.w(1));
                let r = match op {
                    1 => w1,
                    2 => v.wrapping_add(w1),
                    3 => v.wrapping_sub(w1),
                    4 => v.wrapping_mul(w1),
                    5 => if w1 == 0 { v } else { v.wrapping_div(w1) },
                    _ => if w1 == 0 { v } else { v.wrapping_rem(w1) },
                };
                if let Some(x) = self.vars.get_mut(n) {
                    *x = r;
                }
            }
            10 => {
                let name = self.name();
                io.film(self, &name, a0 as i32, self.w(1) as i32, 1, self.w(2) as i32);
            }
            15 | 16 => {
                let n = (a0 as usize).min(self.sprites.len() - 1);
                let (w1, w2) = (self.w(1), self.w(2));
                let s = self.sprites[n];
                if w1 == 9999 {
                    if s.has_place {
                        self.draw_sprite(n, s.px, s.py, op == 16);
                    }
                } else if self.centre {
                    self.draw_sprite(n, w1.wrapping_sub(s.w / 2), w2.wrapping_sub(s.h / 2), op == 16);
                } else {
                    self.draw_sprite(n, w1, w2, op == 16);
                }
            }
            17 => {
                let r = if self.w(1) == 9999 {
                    self.rect
                } else {
                    let r = [self.var(self.w(1)), self.var(self.w(2)), self.var(self.w(3)), self.var(self.w(4))];
                    self.rect = r;
                    r
                };
                if !self.wait(io, a0 as i32, false) {
                    return;
                }
                let w1 = self.size[1].0 as i64;
                for j in 0..r[3] as i64 {
                    for i in 0..r[2] as i64 {
                        let (y, x) = (r[1] as i64 + j, r[0] as i64 + i);
                        let (d, s) = (y * 320 + x, y * w1 + x);
                        if d >= 0 && (d as usize) < 64000 && s >= 0 && (s as usize) < BUF_MEM[1] {
                            self.screen[d as usize] = self.bufs[1][s as usize];
                        }
                    }
                }
                io.timer_start();
                if self.copy_flag != 0 {
                    self.copy0to1();
                }
            }
            18 => {
                let (l, r, l2, r2) = (self.var(a0), self.var(self.w(2)), self.var(self.w(4)), self.var(self.w(6)));
                let r1 = Self::compare(self.w(1), l, r, true);
                let rr2 = Self::compare(self.w(5), l2, r2, false);
                self.cond = match self.w(3) {
                    0 => r1,
                    1 => r1 && rr2,
                    _ => r1 || rr2,
                };
            }
            19 => self.cond = true,
            43 => {
                if self.cond {
                    while self.op() != 19 {
                        if !self.read() {
                            break;
                        }
                    }
                }
            }
            23 => {
                if !self.wait(io, a0 as i32, false) {
                    return;
                }
                self.screen.copy_from_slice(&self.bufs[1][..64000]);
                io.timer_start();
                if self.copy_flag != 0 {
                    self.copy0to1();
                }
            }
            24 | 25 => {
                let pal = self.slots[(a0 as usize).min(3)];
                let (from, to) = if op == 24 { (0, 256) } else { (256, 0) };
                io.fade(self, pal, self.w(1) as i32, from, to);
            }
            26 => {
                let k = (self.w(5) as usize).min(3);
                let wk = self.size[k].0 as i64;
                let (w6, w7) = (self.w(6), self.w(7));
                let sp = Sprite {
                    buf: k,
                    off: self.w(2) as i64 * wk + self.w(1) as i64,
                    w: self.w(3),
                    h: self.w(4),
                    pitch: wk,
                    has_place: w6 != -1 && w7 != -1,
                    px: w6,
                    py: w7,
                };
                if let Some(s) = self.sprites.get_mut(a0 as usize) {
                    *s = sp;
                }
            }
            28 => self.screen[48636..59836].iter_mut().for_each(|b| *b = 0),
            29 => self.subtitle(io, a0 as i32, self.w(1) as u8, self.w(2) as u8),
            36 => {
                if self.wait(io, a0 as i32, true) {
                    io.timer_start();
                }
            }
            38 => {
                if !self.in_call {
                    while self.op() != 39 && self.op() != 99 {
                        if !self.read() {
                            break;
                        }
                    }
                }
                self.cond = self.saved_cond;
            }
            39 => {
                if self.in_call {
                    self.set_pos(self.ret_pos);
                    self.cond = self.saved_cond;
                }
                self.in_call = false;
            }
            40 => {
                self.saved_cond = self.cond;
                self.in_call = true;
                self.ret_pos = self.pos();
                let start = self.script.as_ref().map_or(0, |s| s.start);
                self.set_pos((start as i64 + a0 as i64).max(0) as usize);
            }
            8 => {
                // Palette set: w2 - w1 + 1 colours from colour w1 of slot a0 (past the slot's end: the next slots,
                // then 0 bytes, as in the original's memory) go to the display colours; the colour index wraps at 256.
                let (f, l) = (self.w(1) as i32, self.w(2) as i32);
                if f >= 0 && f <= l {
                    let base = (a0 as usize).min(3) * 1024 + 4 * f as usize;
                    let src: Vec<u8> = (0..4 * (l - f + 1) as usize)
                        .map(|i| { let k = base + i; if k < 4096 { self.slots[k / 1024][k % 1024] } else { 0 } })
                        .collect();
                    for i in 0..src.len() / 4 {
                        let j = (f as usize + i) & 0xFF;
                        self.dac[4 * j..4 * j + 3].copy_from_slice(&src[4 * i..4 * i + 3]);
                    }
                    io.palette(self, f as usize, l as usize, &src);
                }
            }
            30 => {
                // Intro caption: the count + 1 (8-bit); 1: "Metavira Island" at y 85; 2: "Years Later..." at y 100,
                // count = 0; other counts draw nothing.
                self.caption = self.caption.wrapping_add(1);
                let (text, y): (&[u8], i32) = match self.caption {
                    1 => (b"Metavira Island", 85),
                    2 => (b"Years Later...", 100),
                    _ => (b"", 0),
                };
                if self.caption == 2 {
                    self.caption = 0;
                }
                if !text.is_empty() {
                    io.draw_caption(&mut self.screen, text, y);
                }
            }
            37 => {
                // Member seek: member `name` of the script archive becomes the script (start and position at its
                // start); a missing member changes nothing.
                let name = self.name();
                if let Some(sc) = self.script.as_mut() {
                    if let Some((start, _)) = find_member(&sc.data, &name) {
                        sc.start = start;
                        sc.pos = start;
                    }
                }
            }
            41 => {
                // Palette member: the original's 0x53300 (ArcSeek) + 0x53204 (last 0x301 bytes: 0x0C, then
                // 256 x RGB, each >> 2) and its copy 0x52820 into the display palette.
                let name = self.name();
                if let Some(m) = member(self.gfx.as_ref(), &name) {
                    let n = m.len();
                    if n >= 769 && m[n - 769] == 0x0C {
                        for (i, v) in m[n - 768..].iter().enumerate() {
                            self.last_pal[i] = v >> 2;
                        }
                    }
                }
                let slot = &mut self.slots[(a0 as usize).min(3)];
                for e in 0..256 {
                    slot[4 * e..4 * e + 3].copy_from_slice(&self.last_pal[3 * e..3 * e + 3]);
                }
            }
            42 => {
                let name = self.name();
                if let Some(m) = member(self.gfx.as_ref(), &name) {
                    let p = self.size[0].0 as i64;
                    pcx_into(m, &mut self.bufs[0], 0, p);
                }
                if self.copy_flag != 0 {
                    self.copy0to1();
                }
            }
            46 => {
                let v = a0 as u8;
                self.bufs[0][..64000].iter_mut().for_each(|b| *b = v);
                self.bufs[1][..64000].iter_mut().for_each(|b| *b = v);
            }
            47 => self.screen.iter_mut().for_each(|b| *b = 0),
            50 => {
                let name = self.name();
                if name != "TEMPSTR" {
                    self.text_buf = name;
                }
                let k = (a0 as usize).min(3);
                let wk = self.size[k].0 as i64;
                let o = self.w(2) as i64 * wk + self.w(1) as i64;
                if let Some(m) = member(self.gfx.as_ref(), &self.text_buf.clone()) {
                    let m = m.to_vec();
                    pcx_into(&m, &mut self.bufs[k], o, wk);
                }
            }
            52 => {
                let k = (a0 as usize).min(3);
                self.size[k] = (self.w(1) as u16, self.w(2) as u16);
            }
            53 => {
                // Op 0x35 (0x5DB29): MovePaletteEntry 0x528DC moves entry w2 to w1 in palette slot a0 (the entries
                // between shift up by one when w2 > w1); then PaletteSet 0x52870 shows colours 0xA5..0xB2 of the
                // DAC copy 0xEDA08 (the tile palette).
                // SPEC GAP: entries outside slot a0 write the globals around the slots in the original; dropped here.
                let k = (a0 as usize).min(3);
                let (dst, src) = (self.w(1) as i32, self.w(2) as i32);
                let slot = &mut self.slots[k];
                if (0..256).contains(&dst) && (0..256).contains(&src) {
                    let e = [slot[4 * src as usize], slot[4 * src as usize + 1],
                             slot[4 * src as usize + 2], slot[4 * src as usize + 3]];
                    for i in (dst as usize..src as usize).rev() {
                        let prev = [slot[4 * i], slot[4 * i + 1], slot[4 * i + 2], slot[4 * i + 3]];
                        slot[4 * i + 4..4 * i + 8].copy_from_slice(&prev);
                    }
                    slot[4 * dst as usize..4 * dst as usize + 4].copy_from_slice(&e);
                }
                let src = self.dac_copy[4 * 0xA5..4 * 0xB3].to_vec();
                for i in 0..14 {
                    let j = 0xA5 + i;
                    self.dac[4 * j..4 * j + 3].copy_from_slice(&src[4 * i..4 * i + 3]);
                }
                io.palette(self, 0xA5, 0xB2, &src);
            }
            54 => self.gfx = io.file(&self.name()),
            55 => self.gfx = None,
            56 => self.snd = io.file(&self.name()),
            57 => self.snd = None,
            58 => {
                if io.speaking() {
                    io.speech_stop();
                }
                self.voice = self.snd.as_ref().and_then(|a| find_member(a, &self.name())).unwrap_or((0, 0));
            }
            59 => {
                let name = self.name();
                self.cue_load(io, a0 as i32, self.w(1) as i32, &name);
            }
            60 => io.music_load(&self.name()),
            61 => io.music(a0 as i32, 0),
            62 => io.sequence_volume(a0 as i32, self.w(1) as i32),
            63 => io.music_end(),
            67 => {
                let name = self.name();
                let data = member(self.snd.as_ref(), &name).map(|m| m.to_vec());
                io.sfx_load(&name, a0 as i32, data.as_deref());
            }
            68 => io.sfx_play(a0 as i32, self.w(1) as i32, self.w(2) as u8, self.w(3) as i32),
            70 => {
                if self.w(1) != 0 {
                    self.speech_vol = self.w(1) as i32;
                }
                let (o, n) = self.voice;
                let data = self.snd.as_ref().and_then(|a| a.get(o..o + n)).map(|d| d.to_vec()).unwrap_or_default();
                io.speech(a0 as i32, self.speech_vol, &data);
            }
            71 => {
                io.speech_stop();
                io.samples_end();
                io.voc_stop();
            }
            _ => {}
        }
    }

    /// Op 29 (and the win sequence's quote 79): the subtitle text of INTQUOTE record q.
    pub fn subtitle(&mut self, io: &mut dyn SabIo, q: i32, fg: u8, shadow: u8) {
        if !io.subtitles() && (q as i16) < 77 {
            return;
        }
        let rec = io.int_quote(q);
        let mut screen = std::mem::take(&mut self.screen);
        io.draw_subtitle(&mut screen, &rec, fg, shadow);
        self.screen = screen;
    }

    /// flcplay_diff: the speech archive of the voice cues.
    pub fn set_speech_archive(&mut self, arc: Vec<u8>) {
        self.snd = Some(arc);
    }

    /// PlayFilm 0x5BFF8 + FlcPlay 0x5BC58 + FilmCallback 0x5BE18: film `data` at (x, y); colours only with `flag`; `loops`
    /// 0 = until a key. Each later frame waits for its tick: the timer starts before the first frame, so frame i
    /// shows at i x frame time and the film ends right after its last frame (#31). The original drops a tick that
    /// comes during a frame; this does not matter unless a frame takes longer than the frame time.
    /// The key test comes before the cue step: a stop key skips the cue of that frame.
    pub fn play_film(&mut self, io: &mut dyn SabIo, data: &[u8], x: i32, y: i32, flag: i32, loops: i32) {
        let Some(f) = crate::flc::Flc::new(data) else { return };
        let ms = f.speed_ms.max(1) as u64;
        let mut steps = crate::flc::FilmSteps::new(f, loops);
        let start = std::time::Instant::now();
        let mut shown = 0u64;
        while let Some(step) = steps.next(&mut self.screen, x, y, &mut || io.film_wait(start, ms * shown)) {
            let j = match step {
                crate::flc::FilmStep::PassEnd => {
                    self.cue_i = 0;
                    continue;
                }
                crate::flc::FilmStep::First => None,
                crate::flc::FilmStep::Frame(j) => Some(j),
            };
            shown += 1;
            if flag != 0 {
                steps.flc.set_dac(&mut self.dac);
            }
            if let Some(j) = j {
                if let Some(k) = io.film_key(self, j) {
                    if k == 27 || self.film_flag {
                        self.abort = true;
                        io.film_show(self);
                        return;
                    }
                }
                self.film_cue(io, j);
            }
            io.film_show(self);
        }
    }

    /// ending.md "Film cues": the cue step after a film frame with the frame counter `c`. Returns what ran.
    pub fn film_cue(&mut self, io: &mut dyn SabIo, c: i32) -> Cue {
        // SPEC GAP: past entry 79 the front-end reads (9999, 0).
        let (f, a) = match self.cues.get(4 * self.cue_i..4 * self.cue_i + 4) {
            Some(e) => (i16::from_le_bytes([e[0], e[1]]) as i32, i16::from_le_bytes([e[2], e[3]]) as i32),
            None => (9999, 0),
        };
        let cue = if c == f - 1 {
            if a < 99 {
                io.sfx_play(a, self.cue_rate, 50, 1);
                Cue::Effect(a)
            } else if a == 99 {
                self.screen[48636..59836].iter_mut().for_each(|b| *b = 0);
                Cue::Clear
            } else if a < 200 {
                // SPEC GAP: past name 9 the front-end reads an empty name.
                let name: Vec<u8> = self.cues.get(200 + 12 * self.cue_j..212 + 12 * self.cue_j).unwrap_or(&[0])
                    .iter().take_while(|&&b| b != 0).copied().collect();
                if name.is_empty() {
                    Cue::NoVoice
                } else {
                    self.cue_j += 1;
                    let name = String::from_utf8_lossy(&name).into_owned();
                    match self.snd.as_ref().and_then(|d| find_member(d, &name)) {
                        Some((o, n)) => {
                            self.voice = (o, n);
                            let data = self.snd.as_ref().and_then(|d| d.get(o..o + n)).map(|d| d.to_vec()).unwrap_or_default();
                            io.speech(self.cue_rate, self.cue_vol, &data);
                            Cue::Voice(name, true)
                        }
                        None => {
                            self.voice = (0, 0);
                            Cue::Voice(name, false)
                        }
                    }
                }
            } else {
                let fg = match a {
                    231 | 233 | 235 => 0x83,
                    232 | 234 => 0x30,
                    236 | 237 => 0x49,
                    _ => 0xFF,
                };
                self.subtitle(io, a - 200, fg, 0);
                Cue::Subtitle(a - 200, fg)
            }
        } else if f == 9998 {
            io.music(a, 0);
            Cue::Music(a)
        } else {
            return Cue::None;
        };
        self.cue_i += 1;
        cue
    }

    /// Op 59 (ending.md "Film cues"): the film cue rate `rate` and voice volume `vol` (each kept when 0), the
    /// counters to 0, and the cue table from the CD file `name` (empty name: 50 x (9999, 0)).
    pub fn cue_load(&mut self, io: &mut dyn SabIo, rate: i32, vol: i32, name: &str) {
        if rate != 0 {
            self.cue_rate = rate;
        }
        if vol != 0 {
            self.cue_vol = vol;
        }
        (self.cue_i, self.cue_j) = (0, 0);
        self.cues = [0; 320];
        if name.is_empty() {
            for k in 0..50 {
                self.cues[4 * k..4 * k + 2].copy_from_slice(&9999i16.to_le_bytes());
            }
        } else {
            // SPEC GAP: the original ends the program when the file cannot be opened; the front-end plays
            // the films with no cues.
            match io.cd_file(name) {
                Some(d) if d.len() >= 320 => self.cues.copy_from_slice(&d[..320]),
                _ => self.cues[..4].copy_from_slice(&[0x0F, 0x27, 0, 0]),
            }
        }
    }

    /// The graphics archive of op 54 (the films are found in it).
    pub fn set_graphics_archive(&mut self, data: Option<Vec<u8>>) {
        self.gfx = data;
    }

    /// The sound archive of op 56 (the cue voices and the effects are found in it).
    pub fn set_sound_archive(&mut self, data: Option<Vec<u8>>) {
        self.snd = data;
    }

    /// The member name of the open script (for a log line).
    pub fn member(&self) -> &str {
        self.script.as_ref().map_or("", |s| s.member.as_str())
    }

    /// The graphics archive member `name` (the films).
    pub fn graphic(&self, name: &str) -> Option<&[u8]> {
        member(self.gfx.as_ref(), name)
    }

    /// The win sequence's SAB part (ending.md "Game end", win sequence steps 3, 4): WINGAME.SAB with v0..v3, then
    /// (when it ran to the end) quote 79 and INTRO05.FLC at (0, 15) until a key. Returns the script's result.
    pub fn win(&mut self, io: &mut dyn SabIo, v: [i16; 4]) -> i32 {
        self.vars[..4].copy_from_slice(&v);
        io.music_load("MISC\\intro");
        let r = self.play_script(io, "DAT\\ENDSAB.DAT", "WINGAME.SAB");
        if r == 1 {
            self.film_flag = true;
            self.subtitle(io, 0x4F, 0x50, 7);
            io.film(self, "INTRO05.FLC", 0, 15, 1, 0);
            self.film_flag = false;
        }
        r
    }
}

/// CRC-32 (the zlib one) of `data`, continued from `crc`.
pub fn crc32(crc: u32, data: &[u8]) -> u32 {
    static TABLE: std::sync::OnceLock<[u32; 256]> = std::sync::OnceLock::new();
    let t = TABLE.get_or_init(|| {
        let mut t = [0u32; 256];
        for (i, e) in t.iter_mut().enumerate() {
            let mut c = i as u32;
            for _ in 0..8 {
                c = if c & 1 != 0 { 0xEDB8_8320 ^ (c >> 1) } else { c >> 1 };
            }
            *e = c;
        }
        t
    });
    let mut c = !crc;
    for &b in data {
        c = t[((c ^ b as u32) & 0xFF) as usize] ^ (c >> 8);
    }
    !c
}

/// The test log of `--sab` (validate/diff/sab_diff.py): every wait ends at once, no key, the films and sounds only
/// write their log lines.
pub struct LogIo {
    pub dir: std::path::PathBuf,
    pub iso: String,
    pub lines: Vec<String>,
    pub subtitles: bool,
    pub draw: Option<Box<dyn FnMut(&mut [u8], &[u8], u8, u8)>>,
    /// The screen and colours after the last fade to full brightness or before the last fade-out from it (the
    /// script's picture for JA1_END_PNG).
    pub bright: Option<(Vec<u8>, [u8; 1024])>,
    /// The films play (every frame once, no waits, with the cue steps; a `film_frames N` line after the `flc`
    /// line); false = the films are stubbed (the test logs of sab_diff and credits_diff).
    pub films: bool,
}

impl SabIo for LogIo {
    fn file(&mut self, name: &str) -> Option<Vec<u8>> {
        self.lines.push(format!("open {}", name));
        let rel = name.replace('\\', "/");
        std::fs::read(self.dir.join(&rel)).ok().or_else(|| {
            crate::iso::Iso::open(&self.iso).ok().and_then(|mut i| i.read(&rel).ok())
        })
    }
    fn cd_file(&mut self, name: &str) -> Option<Vec<u8>> {
        self.lines.push(format!("open_cd {}", name));
        let rel = name.replace('\\', "/");
        std::fs::read(self.dir.join(&rel)).ok().or_else(|| {
            crate::iso::Iso::open(&self.iso).ok().and_then(|mut i| i.read(&rel).ok())
        })
    }
    fn step(&mut self, s: &Sab) {
        let n = self.lines.iter().filter(|l| l.starts_with("step ")).count();
        self.lines.push(format!("step {} op {} {} {:08x}", n, s.op(), s.a0(), s.checksum()));
    }
    fn elapsed_ms(&mut self) -> i64 {
        i64::MAX
    }
    fn timer_start(&mut self) {}
    fn wait_ms(&mut self, _s: &Sab, ms: i64) {
        self.lines.push(format!("wait_ms {}", ms));
    }
    fn retrace(&mut self, s: &Sab) {
        self.lines.push(format!("frame {:08x}", crc32(0, &s.screen)));
    }
    fn fade(&mut self, s: &mut Sab, pal: [u8; 1024], steps: i32, from: i32, to: i32) {
        self.lines.push(format!("fade 0 {} {} {} {:08x}", steps, from, to, crc32(0, &pal)));
        if from >= 256 && s.screen.iter().any(|&b| b != 0) {
            // The picture shown at full brightness before a fade-out (the test log draws no films: a black screen
            // after a film is not kept).
            let mut full = [0u8; 1024];
            for (d, &c) in full.iter_mut().zip(pal.iter()) {
                *d = (c as i32 * from / 256) as u8;
            }
            self.bright = Some((s.screen.clone(), full));
        }
        fade_dac(&mut s.dac, &pal, to, 256);
        if to >= 256 && s.screen.iter().any(|&b| b != 0) {
            self.bright = Some((s.screen.clone(), s.dac));
        }
    }
    fn film(&mut self, s: &mut Sab, name: &str, x: i32, y: i32, flag: i32, _loops: i32) {
        self.lines.push(format!("flc {} {} {} {}", name, x, y, flag));
        if !self.films {
            return;
        }
        // ending.md "Films": one pass, no key; the cue step after each frame but the first (c = 0..N-2).
        let Some(data) = s.graphic(name).map(|d| d.to_vec()) else { return };
        let Some(mut f) = crate::flc::Flc::new(&data) else { return };
        let mut k = 0;
        while f.next_frame(&mut s.screen, x, y) {
            if flag != 0 {
                f.set_dac(&mut s.dac);
            }
            if k > 0 {
                s.film_cue(self, k - 1);
            } else {
                // The film's first frame (the script's picture).
                self.bright = Some((s.screen.clone(), s.dac));
            }
            k += 1;
        }
        s.cue_i = 0;
        self.lines.push(format!("film_frames {}", k));
    }
    fn music_load(&mut self, name: &str) {
        self.lines.push(format!("music_load {}", name));
    }
    fn music(&mut self, theme: i32, stop_first: i32) {
        self.lines.push(format!("music {} {}", theme, stop_first));
    }
    fn theme(&self) -> i32 {
        // The last theme started in this log (the log has no music stops; -1 before the first).
        self.lines.iter().rev().find_map(|l| l.strip_prefix("music ")?.split(' ').next()?.parse().ok()).unwrap_or(-1)
    }
    fn sfx_load(&mut self, name: &str, slot: i32, _data: Option<&[u8]>) {
        self.lines.push(format!("sfx_load {} {}", name, slot));
    }
    fn sfx_play(&mut self, slot: i32, rate: i32, vol: u8, loops: i32) {
        self.lines.push(format!("sfx_play {} {} {} {}", slot, rate, vol, loops));
    }
    fn speech(&mut self, rate: i32, vol: i32, _data: &[u8]) {
        self.lines.push(format!("speech {} {}", rate, vol));
    }
    fn speech_stop(&mut self) {
        self.lines.push("speech_stop ".into());
    }
    fn voc_stop(&mut self) {
        self.lines.push("voc_stop ".into());
    }
    fn subtitles(&self) -> bool {
        self.subtitles
    }
    fn int_quote(&mut self, q: i32) -> Vec<u8> {
        int_quote(&self.iso, &self.dir.join("JA.EXE").to_string_lossy(), q)
    }
    fn draw_subtitle(&mut self, screen: &mut [u8], text: &[u8], fg: u8, shadow: u8) {
        if let Some(d) = &mut self.draw {
            d(screen, text, fg, shadow);
        }
    }
    fn palette(&mut self, _s: &Sab, first: usize, last: usize, src: &[u8]) {
        self.lines.push(format!("palette {} {} {:08x}", first, last, crc32(0, src)));
    }
    fn draw_caption(&mut self, screen: &mut [u8], text: &[u8], y: i32) {
        draw_caption(&self.iso, &self.dir.join("JA.EXE").to_string_lossy(), screen, text, y);
    }
}

/// Op 29 step 2: record q of EDT\INTQUOTE.EDT (200 bytes; empty past the file's end).
pub fn int_quote(iso: &str, exe: &str, q: i32) -> Vec<u8> {
    let all = crate::aim::edt(iso, exe, "INTQUOTE.EDT");
    all.get(200 * q.max(0) as usize..200 * q.max(0) as usize + 200).unwrap_or(&[]).to_vec()
}

/// Op 29 step 3 on `screen` with the panel fonts of the game files (see `draw_subtitle`).
pub fn draw_subtitle_files(iso: &str, exe: &str, screen: &mut [u8], rec: &[u8], fg: u8, shadow: u8) {
    let (Ok(c), Ok(d)) = (crate::common(iso), crate::panel::panel_data(iso, exe)) else { return };
    let mut t0 = c.remap[0];
    let mut p = crate::panel::Painter { d: &d, buf: screen, table0: &mut t0, table29: crate::panel::table29(&c) };
    draw_subtitle(&mut p, rec, fg, shadow);
}

/// Op 30's caption with the panel fonts (font 2, centred at x 159, colour 0xFF, shadow 1, mode 300).
pub fn draw_caption(iso: &str, exe: &str, screen: &mut [u8], text: &[u8], y: i32) {
    let (Ok(c), Ok(d)) = (crate::common(iso), crate::panel::panel_data(iso, exe)) else { return };
    let mut t0 = c.remap[0];
    let mut p = crate::panel::Painter { d: &d, buf: screen, table0: &mut t0, table29: crate::panel::table29(&c) };
    p.text(text, 159, y, true, 0xFF, 1, 2);
}

/// Op 29 step 3 with the panel fonts: the encoded record `rec` wrapped (font 2, wrap 195) and drawn centred at
/// x 159, y 152 + 11 i.
pub fn draw_subtitle(p: &mut crate::panel::Painter, rec: &[u8], fg: u8, shadow: u8) {
    let t = &rec[..rec.iter().position(|&b| b == 0).unwrap_or(rec.len())];
    let (ls, _) = crate::msgbox::lines_wrap_enc(&p.d.fonts[2].widths, t, 194, true);
    for (i, l) in ls.iter().enumerate() {
        if !l.is_empty() {
            p.text(l, 159, 152 + 11 * i as i32, true, fg, shadow, 2);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    struct WaitIo { keys: Vec<i8>, speaking: bool, quiet: u32, t: i64 }
    impl SabIo for WaitIo {
        fn file(&mut self, _name: &str) -> Option<Vec<u8>> { None }
        fn key(&mut self, _s: &Sab) -> Option<i8> { if self.keys.is_empty() { None } else { Some(self.keys.remove(0)) } }
        fn elapsed_ms(&mut self) -> i64 { self.t += 10; self.t }
        fn timer_start(&mut self) {}
        fn fade(&mut self, _s: &mut Sab, _pal: [u8; 1024], _steps: i32, _from: i32, _to: i32) {}
        fn film(&mut self, _s: &mut Sab, _name: &str, _x: i32, _y: i32, _flag: i32, _loops: i32) {}
        fn speaking(&self) -> bool { self.speaking }
        fn speech_end_quiet(&mut self) { self.quiet += 1; }
    }

    /// Ledger rows ja1front:16227/16229 (ending.md op 18 "if"): code 3 is L < R and code 5 is L >= R.
    #[test]
    fn if_codes_3_and_5() {
        assert!(Sab::compare(3, 1, 2, false));
        assert!(!Sab::compare(3, 2, 2, false));
        assert!(Sab::compare(5, 2, 2, false));
        assert!(!Sab::compare(5, 1, 2, false));
    }

    /// Ledger rows ja1front:16193/16195 (ending.md "Waits"): a wait of t ms ends when the time is >= t, so a wait
    /// whose first clock read is already t reads no key (the Escape stays unread and the wait counts as done).
    #[test]
    fn wait_ends_at_t() {
        let mut sab = Sab::new();
        let mut io = WaitIo { keys: vec![27], speaking: false, quiet: 0, t: 0 };
        assert!(sab.wait(&mut io, 10, false));
        assert_eq!(io.keys, vec![27]);
    }

    /// Ledger rows d057aca:ja1front:16997/16999 (ending.md "Waits"): only Escape ends a wait early; it ends the voice
    /// too, but under op 36's rule only while a voice plays. Another key goes and the wait runs on.
    #[test]
    fn only_escape_ends_a_wait_and_op36_keeps_a_silent_voice() {
        let mut sab = Sab::new();
        let io = |keys: Vec<i8>, speaking| WaitIo { keys, speaking, quiet: 0, t: 0 };
        let mut a = io(vec![97], false);
        assert!(sab.wait(&mut a, 100, false));
        assert_eq!((sab.last_key, a.quiet), (0, 0));
        let mut b = io(vec![27], false);
        assert!(!sab.wait(&mut b, 100, false));
        assert_eq!((sab.last_key, b.quiet), (27, 1));
        let mut c = io(vec![27], false);
        assert!(!sab.wait(&mut c, 100, true));
        assert_eq!(c.quiet, 0);
        let mut d = io(vec![27], true);
        assert!(!sab.wait(&mut d, 100, true));
        assert_eq!(d.quiet, 1);
    }

    /// An io with the default Subtitles option: op 29 records which quotes it draws.
    struct TextIo { drawn: Vec<i32> }
    impl SabIo for TextIo {
        fn file(&mut self, _name: &str) -> Option<Vec<u8>> { None }
        fn elapsed_ms(&mut self) -> i64 { 0 }
        fn timer_start(&mut self) {}
        fn fade(&mut self, _s: &mut Sab, _pal: [u8; 1024], _steps: i32, _from: i32, _to: i32) {}
        fn film(&mut self, _s: &mut Sab, _name: &str, _x: i32, _y: i32, _flag: i32, _loops: i32) {}
        fn int_quote(&mut self, q: i32) -> Vec<u8> { vec![q as u8] }
        fn draw_subtitle(&mut self, _screen: &mut [u8], text: &[u8], _fg: u8, _shadow: u8) { self.drawn.push(text[0] as i32); }
    }

    /// Ledger row 0b4d529:ja1front:17097 (ending.md op 29 step 1): an io that does not give the Subtitles option has
    /// it off, so the end scripts' quotes below 77 draw no text; quote 77 and above (the win sequence's 79) always do.
    #[test]
    fn op29_without_the_subtitles_option_draws_only_quotes_from_77() {
        let mut sab = Sab::new();
        let mut io = TextIo { drawn: Vec::new() };
        for q in [38, 71, 76, 77, 79] {
            sab.subtitle(&mut io, q, 127, 25);
        }
        assert_eq!(io.drawn, vec![77, 79]);
    }

    fn set_rec(sab: &mut Sab, op: u16, a0: i16, ws: &[i16]) {
        sab.rec = [0; REC];
        sab.rec[0..2].copy_from_slice(&op.to_le_bytes());
        sab.set_a0(a0);
        for (k, &v) in ws.iter().enumerate() {
            sab.set_w(k + 1, v);
        }
    }

    /// A PCX of 4 bytes per line, x 1..2 (2 shown pixels + 2 pad), 2 rows, literal bytes 1..8.
    fn small_pcx() -> Vec<u8> {
        let mut d = vec![0u8; 128];
        d[4..6].copy_from_slice(&1u16.to_le_bytes());
        d[8..10].copy_from_slice(&2u16.to_le_bytes());
        d[10..12].copy_from_slice(&1u16.to_le_bytes());
        d[0x42..0x44].copy_from_slice(&4u16.to_le_bytes());
        d.extend(1u8..=8);
        d
    }

    /// Ledger rows c2aea50:ja1front:16791 and ja1front:16087 (ending.md "PCX pictures"): the line width is
    /// xmax - xmin + 1 and the pad bytes of a line (up to bytes-per-line) are not drawn; a picture whose data
    /// ends early draws what it has and stops (no read past the data).
    #[test]
    fn pcx_skips_line_pad_and_stops_at_the_data_end() {
        let mut buf = vec![0u8; 8];
        pcx_into(&small_pcx(), &mut buf, 0, 4);
        assert_eq!(buf, vec![1, 2, 0, 0, 5, 6, 0, 0]);
        let mut d = small_pcx();
        d.truncate(d.len() - 3);
        let mut buf = vec![0u8; 8];
        pcx_into(&d, &mut buf, 0, 4);
        assert_eq!(buf, vec![1, 2, 0, 0, 5, 0, 0, 0]);
    }

    /// Ledger row ja1front:16426 (ending.md op 50): the picture goes to buffer a0 at w2 * width + w1.
    #[test]
    fn op50_loads_the_picture_at_row_w2() {
        let pcx = small_pcx();
        let mut arc = vec![0u8; 0x40];
        arc[0x14..0x18].copy_from_slice(&2u32.to_le_bytes());
        arc[0x20..0x25].copy_from_slice(b"P.PCX");
        arc[0x30..0x34].copy_from_slice(&0x40u32.to_le_bytes());
        arc[0x34..0x38].copy_from_slice(&(pcx.len() as u32).to_le_bytes());
        arc.extend(&pcx);
        let mut sab = Sab::new();
        sab.set_graphics_archive(Some(arc));
        set_rec(&mut sab, 50, 2, &[3, 2]);
        sab.rec[4..9].copy_from_slice(b"P.PCX");
        sab.run(&mut TextIo { drawn: Vec::new() });
        let wk = sab.size[2].0 as usize;
        let o = 2 * wk + 3;
        assert_eq!(&sab.bufs[2][o..o + 3], &[1, 2, 0]);
        assert_eq!(&sab.bufs[2][o + wk..o + wk + 2], &[5, 6]);
    }

    /// Ledger rows ja1front:16187/16189 (ending.md "Operands"): a0 above 9999 is a literal a0 - 10000 for ops up
    /// to 7, a variable for the others; 9999 itself stays a plain number.
    #[test]
    fn a0_above_9999_is_literal_for_ops_to_7() {
        let mut sab = Sab::new();
        sab.vars[2] = 77;
        for (op, a0, want) in [(1, 10003, 3), (1, 9999, 9999), (8, 9999, 9999), (8, 10002, 77)] {
            set_rec(&mut sab, op, a0, &[]);
            sab.resolve();
            assert_eq!(sab.a0(), want, "op {op} a0 {a0}");
        }
    }

    /// Ledger row c2aea50:ja1front:17056 (ending.md op 26): the sprite has a place unless w6 or w7 is -1.
    #[test]
    fn op26_place_unless_minus_one() {
        let mut sab = Sab::new();
        let mut io = TextIo { drawn: Vec::new() };
        for (w6, w7, want) in [(1, 5, true), (-1, 5, false), (1, -1, false), (0, 0, true)] {
            set_rec(&mut sab, 26, 4, &[0, 0, 2, 2, 0, w6, w7]);
            sab.run(&mut io);
            assert_eq!(sab.sprites[4].has_place, want, "w6 {w6} w7 {w7}");
        }
    }

    /// Ledger row ja1front:16218 (ending.md op 15/16): sprite pixels outside work buffer 1 are dropped, the
    /// pixels inside are drawn.
    #[test]
    fn sprite_pixels_outside_buffer_1_are_dropped() {
        let mut sab = Sab::new();
        sab.bufs[0][..4].copy_from_slice(&[9, 9, 9, 9]);
        sab.sprites[0] = Sprite { buf: 0, off: 0, w: 2, h: 2, pitch: 2, has_place: false, px: 0, py: 0 };
        sab.draw_sprite(0, -1, 0, false);
        assert_eq!(sab.bufs[1][0], 9);
        sab.draw_sprite(0, 0, 30000, false);
        assert_eq!(sab.bufs[1].iter().filter(|&&b| b == 9).count(), 3);
    }

    #[test]
    fn crc32_matches_zlib() {
        assert_eq!(crc32(0, b"123456789"), 0xCBF4_3926);
        assert_eq!(crc32(crc32(0, b"1234"), b"56789"), 0xCBF4_3926);
    }

    /// Ledger rows c2aea50:ja1front:2091/2093 (the window's credits fade): level 3 of 8 gives 3/8 of each colour,
    /// rounded down; level 8 of 8 the full colour.
    #[test]
    fn a_fade_level_scales_each_colour() {
        let mut pal = [0u8; 1024];
        (pal[0], pal[1], pal[2], pal[1023]) = (63, 40, 7, 255);
        let mut dac = [9u8; 1024];
        fade_dac(&mut dac, &pal, 3, 8);
        assert_eq!((dac[0], dac[1], dac[2], dac[3], dac[1023]), (23, 15, 2, 0, 95));
        fade_dac(&mut dac, &pal, 8, 8);
        assert_eq!(&dac[..3], &[63, 40, 7]);
    }
}
