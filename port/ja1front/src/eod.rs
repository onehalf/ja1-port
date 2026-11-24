//! The end-of-day screen (docs/spec/world_load.md section 8 "End-of-day screen"): the host ops 254..260, 206, 207,
//! 37, 57 and 215 while the screen is open. It draws the sunset picture, the texts and the palette, and shows
//! them through an output (a window, or nothing in a play script).
use crate::data::Common;
use crate::panel::{self, Painter};
use crate::play::PlayHost;
use crate::soldier::REC;
use crate::common;
use std::sync::Arc;

/// Where the screen's frames go. A play script has none (`EodHost::sink` = None): nothing waits.
pub trait EodSink {
    /// Show S with the 8-bit palette (6-bit value v scaled to v * 255 / 63). `retrace` = the frame of op 207:
    /// wait until 1/70 s after the previous such frame.
    fn show(&mut self, s: &[u8], pal: &[u8; 768], retrace: bool);
    /// Wait `ms` milliseconds (taking no input).
    fn delay(&mut self, ms: u32);
    /// Op 255: a clock wait of `ms` (crate::dostime) from the end of the previous one; the first one does not wait.
    fn clock_wait(&mut self, _ms: i64) {}
    /// A key press or mouse button press since the last poll.
    fn poll(&mut self) -> bool;
    /// Op 354 replay_record / 355 replay_anim (strategic.md "The day replay"): the original polls
    /// EscPressed? 0x67300 there, so only Escape stops the replay; a click or another key does nothing. That
    /// routine consumes any waiting key, Escape or not. A sink without key detail falls back to `poll`.
    fn poll_escape(&mut self) -> bool {
        self.poll()
    }
    /// Mixed sound of the screen's time (sound.md "Output"); the window queues it.
    fn audio(&mut self, _pcm: &[i16]) {}
}

/// The screen's front-end state: the picture P, the screen S, the work palette W and the shown palette D.
pub struct EodHost {
    iso: String,
    exe: String,
    table0: [u8; 256],
    pub sink: Option<Box<dyn EodSink>>,
    pub open: bool,
    p: Vec<u8>,
    pub s: Vec<u8>,
    w: [u8; 768],
    d: [u8; 768],
    /// Frames shown by op 207, polls (op 257), texts drawn (ops 256 and 57) for this screen.
    pub frames: u32,
    pub polls: u32,
    pub texts: u32,
    /// Script mode: the poll number that answers 1 (`eod N`).
    pub poll_at: Option<u32>,
    /// The last frame shown: S and the 8-bit palette D.
    pub last: Option<(Vec<u8>, [u8; 768])>,
    common: Option<Arc<Common>>,
    /// With a sink: the times of the first and the last op 207 frame (JA1_SPEED_LOG=1 prints the span).
    span: Option<(std::time::Instant, std::time::Instant)>,
    /// The last op 57 picture (S with the text) for JA1_EOD_MSG_PNG.
    pub msg_last: Option<Vec<u8>>,
}

/// 6-bit VGA palette -> 8-bit (v * 255 / 63).
pub fn scale_palette(d: &[u8; 768]) -> [u8; 768] {
    let mut o = [0u8; 768];
    for i in 0..768 {
        o[i] = (d[i] as u32 * 255 / 63) as u8;
    }
    o
}

/// SUNSET.PCX: (P, W). W = the file's last 769 bytes (0x0C, then 256 x RGB) shifted right by 2. A bad or missing
/// file gives all 0 (SPEC GAP in the spec: a missing member).
pub fn sunset(pcx_file: Option<&[u8]>) -> (Vec<u8>, [u8; 768]) {
    let none = (vec![0u8; 320 * 200], [0u8; 768]);
    let Some(f) = pcx_file else { return none };
    let Ok((w, h, pix)) = panel::pcx(f) else { return none };
    if w != 320 || h < 200 || f.len() < 769 || f[f.len() - 769] != 0x0C {
        return none;
    }
    let mut pal = [0u8; 768];
    for (i, v) in f[f.len() - 768..].iter().enumerate() {
        pal[i] = v >> 2;
    }
    (pix[..320 * 200].to_vec(), pal)
}

/// Op 206 / PaletteRotate? 0x528AC: the entries first..last rotate by one toward the lower index. The original
/// rotates 4-byte VGA entries; the port keeps 3-byte RGB entries, so only the visible bytes move.
pub fn rotate(w: &mut [u8; 768], first: i32, last: i32) {
    if first < 0 || last > 255 || first >= last {
        return;
    }
    let (f, l) = (first as usize, last as usize);
    let keep = [w[f * 3], w[f * 3 + 1], w[f * 3 + 2]];
    w.copy_within((f + 1) * 3..(l + 1) * 3, f * 3);
    w[l * 3..l * 3 + 3].copy_from_slice(&keep);
}

/// Op 256: "End of Day " and the day in decimal.
pub fn title_text(day: i32) -> Vec<u8> {
    format!("End of Day {}", day).into_bytes()
}

/// Op 57: the nickname (bytes at +0x16E up to the first 0, at most 9) and the retreat line.
pub fn retreat_text(rec: &[u8], n: i32) -> Option<Vec<u8>> {
    let tail: &[u8] = match n {
        3 => b" is shot retreating!",
        4 => b" dies retreating!",
        _ => return None,
    };
    let nick = &rec[0x16E..0x16E + 9];
    let len = nick.iter().position(|&b| b == 0).unwrap_or(9);
    let mut t = nick[..len].to_vec();
    t.extend_from_slice(tail);
    Some(t)
}

impl EodHost {
    pub fn new(iso: &str, exe: &str, table0: [u8; 256], sink: Option<Box<dyn EodSink>>) -> EodHost {
        EodHost {
            iso: iso.to_string(),
            exe: exe.to_string(),
            table0,
            sink,
            open: false,
            p: vec![0; 320 * 200],
            s: vec![0; 320 * 200],
            w: [0; 768],
            d: [0; 768],
            frames: 0,
            polls: 0,
            texts: 0,
            poll_at: None,
            last: None,
            common: None,
            span: None,
            msg_last: None,
        }
    }

    fn show(&mut self, retrace: bool) {
        let pal = scale_palette(&self.d);
        if let Some(k) = self.sink.as_mut() {
            k.show(&self.s, &pal, retrace);
        }
        self.last = Some((self.s.clone(), pal));
    }

    fn text(&mut self, t: &[u8], x: i32, y: i32, shadow: u8) {
        let (Ok(d), Some(c)) = (panel::panel_data(&self.iso, &self.exe), self.common.clone()) else { return };
        let t29 = *panel::table29(&c);
        let mut p = Painter { d: &d, buf: &mut self.s, table0: &mut self.table0, table29: &t29 };
        p.text(t, x, y, true, 0xFF, shadow, 2);
        self.texts += 1;
    }

    /// Op 57: DrawMercText 0x27B38 on S: plain text with no box (DrawWrappedText? 0x3625C: fg 0xFF, shadow 0, centre x
    /// 161, y 63, wrap 190, font 2, line height 11). MsgRetreatCasualty? 0x30B04 then waits `ms` on the timer 0x92EF8 and
    /// reads no key. The text stays on S until the next op 260.
    fn retreat_line(&mut self, t: &[u8], ms: i32) {
        let (Ok(d), Some(c)) = (panel::panel_data(&self.iso, &self.exe), self.common.clone()) else { return };
        let t29 = *panel::table29(&c);
        let mut p = Painter { d: &d, buf: &mut self.s, table0: &mut self.table0, table29: &t29 };
        let (ls, _) = crate::msgbox::lines_wrap(&p.d.fonts[2].widths, t, 189);
        for (i, l) in ls.iter().enumerate() {
            p.text(l, 161, 63 + 11 * i as i32, true, 0xFF, 0, 2);
        }
        self.texts += 1;
        self.show(false);
        self.msg_last = Some(self.s.clone());
        if let Some(k) = self.sink.as_mut() {
            k.delay(ms.max(0) as u32);
        }
    }

    fn begin(&mut self) {
        let main = std::path::Path::new(&self.exe)
            .parent()
            .map(|p| p.join("DAT").join("MAIN.DAT"))
            .and_then(|f| std::fs::read(f).ok());
        let member = main.and_then(|m| panel::archive_member(&m, "SUNSET.PCX").ok());
        let (p, w) = sunset(member.as_deref());
        self.p = p;
        self.w = w;
        self.d = w;
        self.s = self.p.clone();
        self.frames = 0;
        self.polls = 0;
        self.texts = 0;
        self.open = true;
        self.common = common(&self.iso).ok();
    }

    /// Mixed sound for the sink (the window's audio queue).
    pub fn audio(&mut self, pcm: &[i16]) {
        if let Some(k) = self.sink.as_mut() {
            k.audio(pcm);
        }
    }

    /// The stderr line when the screen closes.
    pub fn summary(&self) -> String {
        format!("eod frames {} polls {} texts {}", self.frames, self.polls, self.texts)
    }

    /// One host op. None = not an end-of-day op now (the generic answer 0 follows).
    pub fn op(&mut self, op: i32, a: &[i32], soldiers: Option<&[u8]>) -> Option<u32> {
        let arg = |i: usize| a.get(i).copied().unwrap_or(0);
        if op == 254 {
            self.begin();
            return Some(0);
        }
        if !self.open {
            return None;
        }
        match op {
            206 => rotate(&mut self.w, arg(0), arg(1)),
            207 => {
                let (f, l) = (arg(0), arg(1));
                if (0..=255).contains(&f) && (0..=255).contains(&l) && f <= l {
                    let (f, l) = (f as usize * 3, l as usize * 3 + 3);
                    let src = self.w;
                    self.d[f..l].copy_from_slice(&src[f..l]);
                }
                self.frames += 1;
                self.show(true);
                if self.sink.is_some() {
                    let now = std::time::Instant::now();
                    self.span = Some((self.span.map_or(now, |(a, _)| a), now));
                }
            }
            255 => {
                if let Some(k) = self.sink.as_mut() {
                    k.clock_wait(2);
                }
            }
            256 => {
                let t = title_text(arg(0));
                self.text(&t, 159, 64, 1);
            }
            257 => {
                let n = self.polls;
                self.polls += 1;
                self.show(false);
                let hit = match self.sink.as_mut() {
                    Some(k) => k.poll(),
                    None => self.poll_at == Some(n),
                };
                return Some(hit as u32);
            }
            260 => {
                self.s = self.p.clone();
                self.show(false);
            }
            37 => {
                self.show(false);
                let ms = arg(0).max(0) as u32;
                if let Some(k) = self.sink.as_mut() {
                    k.delay(ms);
                }
            }
            57 => {
                let (k, n, ms) = (arg(0), arg(1), arg(2));
                if n != 3 && n != 4 {
                    return None;
                }
                if let Some(s) = soldiers {
                    let k = k.clamp(0, 47) as usize;
                    if let Some(t) = s.get(k * REC..(k + 1) * REC).and_then(|r| retreat_text(r, n)) {
                        self.retreat_line(&t, ms);
                    }
                }
            }
            215 => {
                self.show(false);
                // EndOfDayScreen 0x2A20D / 0x2A227: FadePalette? (steps, from, to) on the last picture, one level per retrace.
                if let (Some(k), [_, n, from, to, ..]) = (self.sink.as_mut(), a) {
                    for p in crate::office::fade(&self.d, *n, *from, *to) {
                        k.show(&self.s, &scale_palette(&p), true);
                    }
                }
                self.open = false;
                eprintln!("{}", self.summary());
                // JA1_SPEED_LOG=1 (world_load.md "Game speed"): the sunset's time from the first to the last frame
                if let Some((a, b)) = self.span.take() {
                    if std::env::var("JA1_SPEED_LOG").is_ok_and(|v| v == "1") {
                        eprintln!("speed sunset {} ms {} frames", (b - a).as_millis(), self.frames);
                    }
                }
            }
            _ => return None,
        }
        Some(0)
    }
}

/// The host side: ops of the open screen come here from the play host's call.
pub fn host_op(h: &mut PlayHost, op: i32, a: &[i32]) -> Option<u32> {
    let soldiers = if h.world.is_null() {
        None
    } else {
        // SAFETY: `world` is the view of the running library call; its soldiers are 48 records.
        unsafe {
            let sp = (*h.world).soldiers;
            if sp.is_null() {
                None
            } else {
                Some(std::slice::from_raw_parts(sp as *const u8, 48 * REC))
            }
        }
    };
    if op == 254 && h.eod.is_some() {
        h.end_of_day = true;
    }
    let r = h.eod.as_mut()?.op(op, a, soldiers);
    // sound.md "Output": the screen's frames and waits are sound time (the sound ops came before, in the play
    // host's call). The window gets 0.1 s more at the start, so its audio queue does not run dry between frames.
    let us = sound_us(op, r.is_some(), a, h.sound.live);
    if us > 0 {
        h.sound.pass_us(us);
        if h.sound.live {
            let p = std::mem::take(&mut h.sound.pending);
            if let Some(e) = h.eod.as_mut() {
                e.audio(&p);
            }
        }
    }
    r
}

/// The sound time of one day-end host op (µs); `shown` = the screen took the op.
fn sound_us(op: i32, shown: bool, a: &[i32], live: bool) -> u64 {
    match (op, shown) {
        (254, _) if live => 100_000,
        (207, true) => 1_000_000 / 70,
        // world_load.md section 8 op 255: one clock step per frame (54925 µs) for ops 255 and 207 together
        (255, true) => 54_925 - 1_000_000 / 70,
        (37 | 57, true) => a.get(if op == 37 { 0 } else { 2 }).copied().unwrap_or(0).max(0) as u64 * 1000,
        _ => 0,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// port_api.md: op 37 delay (int ms), op 57 merc_text (slot, int text, int ms). EndOfDayScreen 0x29F44 waits
    /// 200 ms, then shows each retreat line for 1500 ms (live on #31: two deaths = 3.4 s of text).
    #[test]
    fn the_retreat_waits_are_sound_time() {
        assert_eq!(sound_us(37, true, &[200], false), 200_000);
        assert_eq!(sound_us(57, true, &[2, 3, 1500], false), 1_500_000);
        assert_eq!(sound_us(57, false, &[2, 1, 1500], false), 0);
        // Op 207 (a 70 Hz retrace) and op 255 together are one clock step of 54925 µs.
        assert_eq!(sound_us(207, true, &[], false), 14_285);
        assert_eq!(sound_us(207, true, &[], false) + sound_us(255, true, &[], false), 54_925);
    }

    #[test]
    fn title_and_retreat_text() {
        assert_eq!(title_text(7), b"End of Day 7");
        assert_eq!(title_text(-2), b"End of Day -2");
        let mut r = vec![0u8; REC];
        r[0x16E..0x16E + 5].copy_from_slice(b"Fidel");
        assert_eq!(retreat_text(&r, 3).unwrap(), b"Fidel is shot retreating!");
        assert_eq!(retreat_text(&r, 4).unwrap(), b"Fidel dies retreating!");
        assert!(retreat_text(&r, 2).is_none());
        // no 0 within 9 bytes: at most 9
        r[0x16E..0x16E + 10].copy_from_slice(b"ABCDEFGHIJ");
        assert_eq!(retreat_text(&r, 4).unwrap(), b"ABCDEFGHI dies retreating!");
    }

    #[test]
    fn rotation_toward_lower_index() {
        let mut w = [0u8; 768];
        for i in 0..256 {
            w[i * 3] = i as u8;
        }
        rotate(&mut w, 10, 12);
        assert_eq!((w[30], w[33], w[36]), (11, 12, 10));
        assert_eq!((w[27], w[39]), (9, 13));
        let before = w;
        rotate(&mut w, 5, 5);
        assert_eq!(w[..], before[..]);
    }

    #[test]
    fn palette_scaling_and_missing_member() {
        let mut d = [0u8; 768];
        d[0] = 63;
        d[1] = 1;
        let s = scale_palette(&d);
        assert_eq!((s[0], s[1]), (255, 4));
        let (p, w) = sunset(None);
        assert!(p.iter().all(|&b| b == 0) && w.iter().all(|&b| b == 0));
    }

    #[test]
    fn sunset_palette_shift() {
        // A 320 x 200 PCX: header, 200 rows of runs (63 + 63 + ... = 320), then 0x0C and the palette.
        let mut f = vec![0u8; 128];
        f[0] = 10;
        f[2] = 1;
        f[3] = 8;
        f[8..10].copy_from_slice(&319u16.to_le_bytes());
        f[10..12].copy_from_slice(&199u16.to_le_bytes());
        f[65] = 1;
        f[66..68].copy_from_slice(&320u16.to_le_bytes());
        for _ in 0..200 {
            // 320 = 5 x 63 + 5
            for _ in 0..5 {
                f.extend_from_slice(&[0xC0 | 63, 7]);
            }
            f.extend_from_slice(&[0xC0 | 5, 7]);
        }
        f.push(0x0C);
        for i in 0..768 {
            f.push((i % 256) as u8);
        }
        let (p, w) = sunset(Some(&f));
        assert_eq!(p.len(), 64000);
        assert!(p.iter().all(|&b| b == 7));
        assert_eq!((w[0], w[4], w[255], w[767]), (0, 1, 63, 63));
    }

    /// SUNSET.PCX shorter than 200 rows is a bad file: no picture, no palette.
    #[test]
    fn a_short_sunset_is_no_picture() {
        let mut f = vec![0u8; 128];
        f[0] = 10;
        f[2] = 1;
        f[3] = 8;
        f[8..10].copy_from_slice(&319u16.to_le_bytes());
        f[10..12].copy_from_slice(&198u16.to_le_bytes());
        f[65] = 1;
        f[66..68].copy_from_slice(&320u16.to_le_bytes());
        for _ in 0..199 {
            for _ in 0..5 {
                f.extend_from_slice(&[0xC0 | 63, 7]);
            }
            f.extend_from_slice(&[0xC0 | 5, 7]);
        }
        f.push(0x0C);
        f.extend(vec![4u8; 768]);
        let (p, w) = sunset(Some(&f));
        assert!(p.iter().all(|&b| b == 0) && w.iter().all(|&b| b == 0));
    }

    /// Op 207: entries first ..= last of the display palette take the sunset palette (3 bytes each); a range that is
    /// not 0..=255 or with first > last copies nothing.
    #[test]
    fn op_207_copies_the_palette_range() {
        let mut e = EodHost::new("", "", [0; 256], None);
        assert_eq!(e.op(254, &[], None), Some(0));
        e.w = [5; 768];
        e.d = [0; 768];
        e.op(207, &[2, 4], None);
        assert!(e.d[..6].iter().all(|&b| b == 0));
        assert!(e.d[6..15].iter().all(|&b| b == 5));
        assert!(e.d[15..].iter().all(|&b| b == 0));
        e.d = [0; 768];
        e.op(207, &[5, 300], None);
        e.op(207, &[300, 5], None);
        e.op(207, &[-1, 5], None);
        e.op(207, &[6, 5], None);
        assert!(e.d.iter().all(|&b| b == 0));
    }

    /// host_op: the screen's waits are sound time (op 37 = 25 ms: two 10 ms ticks, 5 ms left over).
    #[test]
    fn a_wait_passes_sound_time() {
        let mut h = crate::play::PlayHost::new(1);
        h.eod = Some(EodHost::new("", "", [0; 256], None));
        assert_eq!(host_op(&mut h, 254, &[]), Some(0));
        assert_eq!(h.sound.rest_us, 0);
        assert!(host_op(&mut h, 37, &[25]).is_some());
        assert_eq!(h.sound.rest_us, 5_000);
    }

    /// Op 57 (DrawMercText 0x27B38): "NAME dies retreating!" as plain text on the sunset; it stays until op 260.
    #[test]
    fn retreat_text_plain() {
        let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
        let (iso, exe) = (root.join("rom/JA1.gog"), root.join("rom/JA.EXE"));
        if !iso.exists() {
            return;
        }
        let mut e = EodHost::new(iso.to_str().unwrap(), exe.to_str().unwrap(), [0; 256], None);
        assert_eq!(e.op(254, &[3], None), Some(0));
        let before = e.s.clone();
        let mut sol = vec![0u8; 48 * REC];
        sol[2 * REC + 0x16E..2 * REC + 0x16E + 6].copy_from_slice(b"Grunty");
        assert_eq!(e.op(57, &[2, 4, 1500], Some(&sol)), Some(0));
        let m = e.msg_last.clone().expect("no text");
        assert_eq!(e.texts, 1);
        assert_eq!(m, e.s, "the text stays on the screen");
        assert!((63..74).any(|y| (52..268).any(|x| m[y * 320 + x] != before[y * 320 + x])), "no text rows");
        assert!((60..260).all(|x| m[57 * 320 + x] == before[57 * 320 + x]), "a frame at y 57");
        assert_eq!(e.op(260, &[], None), Some(0));
        assert_eq!(e.s, e.p, "op 260 takes the text away");
    }

    #[test]
    fn script_ops_without_screen() {
        let mut e = EodHost::new("x", "y", [0; 256], None);
        assert_eq!(e.op(257, &[], None), None);
        assert_eq!(e.op(207, &[0, 255], None), None);
        // 254 opens (the files are missing: P and W all 0); polls answer from poll_at.
        assert_eq!(e.op(254, &[3], None), Some(0));
        e.poll_at = Some(1);
        assert_eq!(e.op(257, &[], None), Some(0));
        assert_eq!(e.op(257, &[], None), Some(1));
        assert_eq!(e.op(257, &[], None), Some(0));
        assert_eq!(e.op(207, &[0, 255], None), Some(0));
        assert_eq!((e.frames, e.polls), (1, 3));
        assert_eq!(e.op(215, &[], None), Some(0));
        assert!(!e.open);
    }
}
