//! FLC films (Autodesk FLIC, the public file format; ending.md "Films"): INTRO05, HELILIFT and HELILEAV in
//! DAT\ENDGRAPH.DAT. The decoder keeps one frame buffer and one 256-colour palette (8-bit RGB) and applies the
//! frame chunks in order: COLOR_256 (4), COLOR_64 (11), DELTA_FLC (7), DELTA_FLI (12), BLACK (13), BYTE_RUN (15),
//! FLI_COPY (16); other chunks (PSTAMP 18 ...) are skipped.

pub struct Flc<'a> {
    data: &'a [u8],
    pub width: usize,
    pub height: usize,
    /// Frames in the film (without the ring frame at the end).
    pub frames: usize,
    /// Milliseconds per frame (FLC header).
    pub speed_ms: u32,
    pos: usize,
    shown: usize,
    pub palette: [u8; 768],
    /// Colours that a colour chunk set: only these go to the DAC (the original writes each chunk's colours).
    touched: [bool; 256],
    /// The frame buffer (0x93E64): each frame's bytes after its 16-byte header are read into its start and the chunks
    /// run from there; a chunk that reads past its end (TITLE1's copy frames are 2 bytes short) gets the bytes an
    /// earlier, longer frame left (0 where none did).
    buf: Vec<u8>,
}

fn u16le(d: &[u8], p: usize) -> usize {
    d.get(p..p + 2).map_or(0, |b| u16::from_le_bytes([b[0], b[1]]) as usize)
}

fn u32le(d: &[u8], p: usize) -> usize {
    d.get(p..p + 4).map_or(0, |b| u32::from_le_bytes([b[0], b[1], b[2], b[3]]) as usize)
}

const SW: i32 = 320;
const SH: i32 = 200;

/// The clip of a line piece (0x5B094): None when the row is off the screen or the piece is all left / right of it;
/// a piece that starts left of the screen starts at x 0 with x + n pixels (`put` then skips the cut source bytes),
/// a piece past the right edge is cut.
fn clip(x: i32, y: i32, n: i32) -> Option<(i32, i32)> {
    let end = x + n;
    if !(0..SH).contains(&y) || end < 0 || x >= SW {
        return None;
    }
    let (mut x, mut n) = (x, n);
    if x < 0 {
        n = end;
        x = 0;
    }
    if end > SW {
        n -= end - SW;
    }
    (n >= 0).then_some((x, n))
}

/// n bytes of `src` from its start at (x, y) (0x5B0FC): a piece cut at the left edge skips the source bytes of the cut part.
fn put(screen: &mut [u8], x: i32, y: i32, src: &[u8], n: i32) {
    if let Some((cx, n)) = clip(x, y, n) {
        let at = (y * SW + cx) as usize;
        let skip = (cx - x) as usize;
        for k in 0..n as usize {
            screen[at + k] = src.get(skip + k).copied().unwrap_or(0);
        }
    }
}

/// n x the byte v at (x, y) (0x5B16C).
fn fill(screen: &mut [u8], x: i32, y: i32, v: u8, n: i32) {
    if let Some((x, n)) = clip(x, y, n) {
        let at = (y * SW + x) as usize;
        screen[at..at + n as usize].fill(v);
    }
}

/// n words (low byte first) at (x, y), clipped as 2n bytes; an odd clipped length ends with the low byte (0x5B1C4).
fn fill_words(screen: &mut [u8], x: i32, y: i32, w: u16, n: i32) {
    if let Some((x, n)) = clip(x, y, 2 * n) {
        let at = (y * SW + x) as usize;
        let [lo, hi] = w.to_le_bytes();
        for k in 0..n as usize {
            screen[at + k] = if k % 2 == 0 { lo } else { hi };
        }
    }
}

impl<'a> Flc<'a> {
    /// The film at the start of `data` (FlcOpen 0x5BB30: AF12 seeks to header +0x50, AF11 / FlcPlay's 0x5B920
    /// fallback starts from the in-memory first-frame offset at 128, with the
    /// speed in 1/70 s); None when the header is not a FLIC header (0x5BB30: -5).
    pub fn new(data: &'a [u8]) -> Option<Flc<'a>> {
        let magic = u16le(data, 4);
        if data.len() < 128 || (magic != 0xAF12 && magic != 0xAF11) {
            return None;
        }
        let (width, height) = (u16le(data, 8), u16le(data, 10));
        let speed = u32le(data, 16) as u32;
        // FLI speed is in 1/70 s units.
        let speed_ms = if magic == 0xAF11 { speed * 1000 / 70 } else { speed };
        let first = if magic == 0xAF12 && u32le(data, 80) != 0 { u32le(data, 80) } else { 128 };
        Some(Flc { data, width, height, frames: u16le(data, 6), speed_ms, pos: first, shown: 0, palette: [0; 768],
                   touched: [false; 256], buf: Vec::new() })
    }

    /// The place of the next frame in the data (after `new`: the first frame).
    pub fn pos(&self) -> usize {
        self.pos
    }

    /// Draws the next frame onto `screen` (320 x 200) with the film's top left corner at (x, y), as the original's
    /// frame reader 0x5B990 does: the chunks change only the screen pixels they name, clipped at the screen edges
    /// (not at the film's edges). False at the end of the film or at a frame that is not a frame (0x5B990: -6).
    pub fn next_frame(&mut self, screen: &mut [u8], x: i32, y: i32) -> bool {
        if self.shown >= self.frames {
            return false;
        }
        let p = self.pos;
        let size = u32le(self.data, p);
        if u16le(self.data, p + 4) != 0xF1FA || size < 16 || p + size > self.data.len() {
            return false;
        }
        self.pos += size;
        self.shown += 1;
        let n = size - 16;
        if self.buf.len() < n + 0x10000 {
            self.buf.resize(n + 0x10000, 0);
        }
        self.buf[..n].copy_from_slice(&self.data[p + 16..p + size]);
        let buf = std::mem::take(&mut self.buf);
        // 0x5B810: the chunk loop.
        let chunks = u16le(self.data, p + 6);
        let mut q = 0;
        for _ in 0..chunks {
            let cs = u32le(&buf, q);
            let ct = u16le(&buf, q + 4);
            if cs < 6 || q + cs > n {
                break;
            }
            // The chunk's bytes and what follows it in the buffer.
            let body = &buf[q + 6..];
            match ct {
                4 => self.color(body, 1),
                11 => self.color(body, 4),
                7 => self.delta_flc(body, screen, x, y),
                12 => self.delta_fli(body, screen, x, y),
                13 => {
                    // 0x5B628: w / 2 zero words per row, then the odd last pixel.
                    for r in y..y + self.height as i32 {
                        fill_words(screen, x, r, 0, self.width as i32 / 2);
                        if self.width % 2 == 1 {
                            fill(screen, x + self.width as i32 - 1, r, 0, 1);
                        }
                    }
                }
                15 => self.byte_run(body, screen, x, y),
                16 => {
                    // 0x5B6E4: width bytes per row.
                    let w = self.width;
                    for r in 0..self.height {
                        put(screen, x, y + r as i32, body.get(r * w..).unwrap_or(&[]), w as i32);
                    }
                }
                _ => {}
            }
            q += cs;
        }
        self.buf = buf;
        true
    }

    /// The colours set so far into a DAC copy (4 bytes per colour, 6-bit values: COLOR_256 >> 2, COLOR_64 as is;
    /// 0x5B248 / 0x5B294). PlayFilm 0x5BFF8 runs the colour chunks only with its flag set (door films keep the
    /// tactical palette).
    pub fn set_dac(&self, dac: &mut [u8]) {
        for i in 0..256 {
            if self.touched[i] {
                for k in 0..3 {
                    dac[4 * i + k] = self.palette[3 * i + k] / 4;
                }
            }
        }
    }

    /// 0x5B74C: packets of (skip, count (0 = 256), count x RGB); the colour index is 8-bit (the DAC index port).
    fn color(&mut self, b: &[u8], scale: u8) {
        let packets = u16le(b, 0);
        let (mut i, mut c) = (2, 0usize);
        for _ in 0..packets {
            if i + 2 > b.len() {
                return;
            }
            c += b[i] as usize;
            let n = if b[i + 1] == 0 { 256 } else { b[i + 1] as usize };
            i += 2;
            for k in 0..n {
                let e = (c + k) & 0xFF;
                for j in 0..3 {
                    self.palette[3 * e + j] = b.get(i + j).copied().unwrap_or(0).wrapping_mul(scale);
                }
                self.touched[e] = true;
                i += 3;
            }
            c += n;
        }
    }

    /// 0x5B2D0: per row a packet count byte (not used), then runs until x reaches the film width: count < 0 =
    /// -count bytes, else count x the next byte. A run is clipped only by the screen.
    fn byte_run(&mut self, b: &[u8], screen: &mut [u8], x0: i32, y0: i32) {
        let at = |i: usize| b.get(i).copied().unwrap_or(0);
        let mut i = 0;
        for r in 0..self.height as i32 {
            i += 1;
            let mut x = x0;
            while x < x0 + self.width as i32 {
                let n = at(i) as i8 as i32;
                i += 1;
                if n < 0 {
                    put(screen, x, y0 + r, b.get(i..).unwrap_or(&[]), -n);
                    i += (-n) as usize;
                    x -= n;
                } else {
                    fill(screen, x, y0 + r, at(i), n);
                    i += 1;
                    x += n;
                }
            }
        }
    }

    /// 0x5B3A4: first row (i16, from the film top), row count (i16); per row a packet count byte, packets of
    /// (skip, count i8: >= 0 = count bytes, < 0 = -count x the next byte).
    fn delta_fli(&mut self, b: &[u8], screen: &mut [u8], x0: i32, y0: i32) {
        let at = |i: usize| b.get(i).copied().unwrap_or(0);
        let mut y = u16le(b, 0) as i16 as i32 + y0;
        let lines = u16le(b, 2) as i16;
        let mut i = 4;
        for _ in 0..lines.max(0) {
            let packets = at(i);
            i += 1;
            let mut x = x0;
            for _ in 0..packets {
                x += at(i) as i32;
                let n = at(i + 1) as i8 as i32;
                i += 2;
                if n < 0 {
                    fill(screen, x, y, at(i), -n);
                    i += 1;
                    x -= n;
                } else {
                    put(screen, x, y, b.get(i..).unwrap_or(&[]), n);
                    i += n as usize;
                    x += n;
                }
            }
            y += 1;
        }
    }

    /// 0x5B470: line count (i16; at least one line runs), then per line words: >= 0 = the packet count; 0xC000 set =
    /// skip -w rows; 0x8000 only = the "last pixel" word, which the original gets wrong: it writes the low byte of the
    /// current screen row at column = the word's low byte, row = film x + width - 1 (if on the screen), never the
    /// last pixel; the next word is the packet count (0 = the line ends). Packets: (skip, count i8: >= 0 = count
    /// words, < 0 = -count x the next word).
    fn delta_flc(&mut self, b: &[u8], screen: &mut [u8], x0: i32, y0: i32) {
        let at = |i: usize| b.get(i).copied().unwrap_or(0);
        let mut lines = u16le(b, 0) as i16;
        let stray_row = x0 + self.width as i32 - 1;
        let (mut i, mut y) = (2, y0);
        loop {
            let mut w = u16le(b, i) as u16;
            i += 2;
            if w & 0x8000 != 0 {
                if w & 0x4000 != 0 {
                    y -= w as i16 as i32;
                    continue;
                }
                let col = (w & 0xFF) as i32;
                if stray_row >= 0 && col < SW && stray_row < SH {
                    screen[(stray_row * SW + col) as usize] = y as u8;
                }
                w = u16le(b, i) as u16;
                i += 2;
            }
            let mut x = x0;
            let mut packets = w;
            while packets != 0 {
                x += at(i) as i32;
                let n = at(i + 1) as i8 as i32;
                i += 2;
                if n >= 0 {
                    put(screen, x, y, b.get(i..).unwrap_or(&[]), 2 * n);
                    i += 2 * n as usize;
                    x += 2 * n;
                } else {
                    fill_words(screen, x, y, u16le(b, i) as u16, -n);
                    i += 2;
                    x -= 2 * n;
                }
                packets = packets.wrapping_sub(1);
                if i > b.len() {
                    return;
                }
            }
            lines = lines.wrapping_sub(1);
            y += 1;
            if lines < 1 {
                return;
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Ledger row c2aea50:ja1front:11045 (0x5B2D0 BYTE_RUN): the runs of a row stop when x reaches the film
    /// width; the next row starts with its own packet count byte.
    #[test]
    fn byte_run_rows_stop_at_the_width() {
        let mut d = vec![0u8; 128];
        d[4..6].copy_from_slice(&0xAF12u16.to_le_bytes());
        (d[6], d[8], d[10]) = (1, 2, 2);
        let mut flc = Flc::new(&d).unwrap();
        let mut screen = vec![0u8; 64000];
        flc.byte_run(&[1, 2, 5, 1, 2, 6, 1, 1, 2, 9], &mut screen, 0, 0);
        assert_eq!(&screen[..3], &[5, 5, 0]);
        assert_eq!(&screen[320..323], &[6, 6, 0]);
        // A count of 0 is a fill of 0 pixels: it still takes its value byte.
        let mut screen = vec![0u8; 64000];
        flc.byte_run(&[1, 0, 9, 2, 5, 1, 2, 6], &mut screen, 0, 0);
        assert_eq!(&screen[..3], &[5, 5, 0]);
    }

    #[test]
    fn byte_run_and_palette() {
        // A 4x1 film: one frame with COLOR_256 (colour 1 = 10,20,30) and BYTE_RUN (4 x colour 1).
        let mut d = vec![0u8; 128];
        d[4..6].copy_from_slice(&0xAF12u16.to_le_bytes());
        d[6] = 1;
        d[8] = 4;
        d[10] = 1;
        let color = [1u8, 0, 1, 1, 10, 20, 30];
        let run = [0u8, 4, 1];
        let mut frame = Vec::new();
        for (t, b) in [(4u16, &color[..]), (15u16, &run[..])] {
            frame.extend_from_slice(&((b.len() + 6) as u32).to_le_bytes());
            frame.extend_from_slice(&t.to_le_bytes());
            frame.extend_from_slice(b);
        }
        let mut f = (16 + frame.len() as u32).to_le_bytes().to_vec();
        f.extend_from_slice(&0xF1FAu16.to_le_bytes());
        f.extend_from_slice(&2u16.to_le_bytes());
        f.extend_from_slice(&[0; 8]);
        f.extend_from_slice(&frame);
        d.extend_from_slice(&f);
        let mut flc = Flc::new(&d).unwrap();
        let mut screen = vec![9u8; 64000];
        assert!(flc.next_frame(&mut screen, 2, 1));
        assert_eq!(&screen[320 + 1..320 + 7], &[9, 1, 1, 1, 1, 9]);
        assert_eq!(&flc.palette[3..6], &[10, 20, 30]);
        assert!(!flc.next_frame(&mut screen, 2, 1));
    }
}

#[cfg(test)]
mod film_tests {
    use super::*;

    /// The three end films decode to their last frame (skipped when the game files are missing).
    #[test]
    fn end_films_decode() {
        let Ok(arc) = std::fs::read(concat!(env!("CARGO_MANIFEST_DIR"), "/../../rom/DAT/ENDGRAPH.DAT")) else { return };
        for name in ["INTRO05.FLC", "HELILIFT.FLC", "HELILEAV.FLC"] {
            let m = crate::panel::archive_member(&arc, name).unwrap();
            let mut f = Flc::new(&m).unwrap();
            let mut screen = vec![0u8; 64000];
            let mut n = 0;
            while f.next_frame(&mut screen, 0, 0) {
                n += 1;
            }
            assert_eq!(n, f.frames, "{}", name);
            assert!(screen.iter().any(|&p| p != 0) && f.palette.iter().any(|&c| c != 0), "{}", name);
        }
    }
}

/// One step of the film loop, see [`FilmSteps`].
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum FilmStep {
    /// The film's first frame was read: FlcPlay runs no callback for it.
    First,
    /// Frame counter `j` of the pass was read: the callback runs with it (j = N - 1 is the ring frame).
    Frame(i32),
    /// A pass ended: the cue index goes back to 0.
    PassEnd,
}

/// The frame order of FlcPlay 0x5BC58: the first frame; then per pass the seek to the second frame (+0x54; when that
/// is 0, the first frame's place + its size, FlcFirstFrame 0x5B920) and frames j = 0..N-2, and the ring frame
/// j = N-1 in every pass but the last. `loops` 0 = no last pass. A frame that does not read ends the film.
pub struct FilmSteps<'a> {
    pub flc: Flc<'a>,
    n: i32,
    loops: u32,
    pass: u32,
    j: i32,
    second: usize,
    started: bool,
    in_pass: bool,
}

impl<'a> FilmSteps<'a> {
    pub fn new(mut flc: Flc<'a>, loops: i32) -> FilmSteps<'a> {
        let oframe2 = u32le(flc.data, 84);
        // FlcFirstFrame 0x5B920 reads two 16-byte headers into one buffer, so the second read wins: the place is the first
        // frame + the first 4 bytes after its header (the size of its first chunk), not + the frame's size. No game film
        // comes here: every one has a second-frame place.
        let second = if oframe2 != 0 { oframe2 } else { flc.pos + u32le(flc.data, flc.pos + 16) };
        let n = flc.frames as i32;
        // The ring frame is one more frame in the file.
        flc.frames += 1;
        FilmSteps { flc, n, loops: loops as u32, pass: 0, j: 0, second, started: false, in_pass: false }
    }

    /// Reads the next frame onto `screen` when the step is a frame. `wait` is the spin 0x5BD11 on the film tick, which
    /// comes before every frame read but the first.
    pub fn next(&mut self, screen: &mut [u8], x: i32, y: i32, wait: &mut dyn FnMut()) -> Option<FilmStep> {
        if !self.started {
            self.started = true;
            return self.flc.next_frame(screen, x, y).then_some(FilmStep::First);
        }
        if !self.in_pass {
            if self.loops != 0 && self.pass >= self.loops {
                return None;
            }
            self.flc.pos = self.second;
            self.flc.shown = 1;
            self.j = 0;
            self.in_pass = true;
        }
        if self.j < self.n - 1 || (self.j < self.n && (self.loops == 0 || self.pass < self.loops - 1)) {
            wait();
            if !self.flc.next_frame(screen, x, y) {
                return None;
            }
            self.j += 1;
            return Some(FilmStep::Frame(self.j - 1));
        }
        self.in_pass = false;
        self.pass += 1;
        Some(FilmStep::PassEnd)
    }
}
