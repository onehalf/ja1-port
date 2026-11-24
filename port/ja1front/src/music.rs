//! Music (docs/spec/music.md): XMIDI files from MISC\, the theme state, the tactical theme rule and the sequencer
//! that sends each sequence to the FM driver (fm.rs, timbre bank MADLAB.AD). One music state per process: the window
//! renders it from an SDL audio callback, a play script mixes it into the sound mix (sound.md "Output").
use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::{Arc, Mutex, OnceLock};

use crate::fm::{Driver, Opl, Timbre};
use crate::sound::RATE;

/// Ticks per second of XMIDI time.
const TICK_HZ: f64 = 120.0;
/// The service period of the driver in us (MDI driver +0x10 in a live dump).
const SERVICE_US: i32 = 8333;
/// Output scale of one voice at full level (music.md "Output and script line", Level: a DOSBox-X capture of the original).
const VOICE_SCALE: f32 = 4900.0;

#[derive(Clone, Copy, Debug)]
enum Ev {
    Note { ch: u8, key: u8, vel: u8, dur: u32 },
    Ctl { ch: u8, c: u8, v: u8 },
    Prog { ch: u8, p: u8 },
    Bend { ch: u8, lsb: u8, msb: u8 },
    End,
}

/// One sequence: (delay in ticks before the event, event).
struct Seq {
    evs: Vec<(u32, Ev)>,
    /// The TIMB chunk: the timbres (bank, patch) the sequence installs.
    timb: Vec<(u8, u8)>,
    /// The file offset of the FORM XMID that holds the sequence (what ail_find_XMIDI_sequence? 0x81E50 returns).
    form: usize,
}

fn varlen(b: &[u8], p: &mut usize) -> Option<u32> {
    let mut v = 0u32;
    loop {
        let c = *b.get(*p)?;
        *p += 1;
        v = (v << 7) | (c & 0x7F) as u32;
        if c < 0x80 {
            return Some(v);
        }
    }
}

/// music.md "Events": the EVNT chunk of one sequence.
fn parse_evnt(e: &[u8]) -> Seq {
    let mut evs = Vec::new();
    let mut p = 0;
    let mut delay = 0u32;
    while p < e.len() {
        let c = e[p];
        if c < 0x80 {
            delay += c as u32;
            p += 1;
            continue;
        }
        p += 1;
        let ch = c & 15;
        let ev = match c >> 4 {
            _ if c == 0xFF => {
                let ty = e.get(p).copied().unwrap_or(0x2F);
                p += 1;
                let n = varlen(e, &mut p).unwrap_or(0) as usize;
                p += n;
                if ty == 0x2F {
                    Some(Ev::End)
                } else {
                    None
                }
            }
            _ if c == 0xF0 || c == 0xF7 => {
                let n = varlen(e, &mut p).unwrap_or(0) as usize;
                p += n;
                None
            }
            9 => {
                let (key, vel) = (e.get(p).copied().unwrap_or(0), e.get(p + 1).copied().unwrap_or(0));
                p += 2;
                let dur = varlen(e, &mut p).unwrap_or(0);
                Some(Ev::Note { ch, key, vel, dur })
            }
            0xB => {
                let (cc, v) = (e.get(p).copied().unwrap_or(0), e.get(p + 1).copied().unwrap_or(0));
                p += 2;
                Some(Ev::Ctl { ch, c: cc, v })
            }
            0xE => {
                let (lsb, msb) = (e.get(p).copied().unwrap_or(0), e.get(p + 1).copied().unwrap_or(0x40));
                p += 2;
                Some(Ev::Bend { ch, lsb, msb })
            }
            0xC => {
                let pr = e.get(p).copied().unwrap_or(0);
                p += 1;
                Some(Ev::Prog { ch, p: pr })
            }
            0xD => {
                p += 1;
                None
            }
            _ => {
                p += 2;
                None
            }
        };
        if let Some(ev) = ev {
            evs.push((delay, ev));
            delay = 0;
            if matches!(ev, Ev::End) {
                break;
            }
        }
    }
    if !matches!(evs.last(), Some((_, Ev::End))) {
        evs.push((delay, Ev::End));
    }
    Seq { evs, timb: vec![], form: 0 }
}

/// xmifind_diff: the file offset of sequence `n`'s FORM XMID, -1 when the file has no sequence `n`
/// (ail_find_XMIDI_sequence? 0x81E50 returns 0 there and AIL_init_sequence fails: no music).
pub fn xmi_form(d: &[u8], n: i32) -> i64 {
    usize::try_from(n).ok().and_then(|n| parse_xmi(d).get(n).map(|s| s.form as i64)).unwrap_or(-1)
}

/// music.md "Files": the sequences of an XMI file (index = theme).
fn parse_xmi(d: &[u8]) -> Vec<Arc<Seq>> {
    let mut out = Vec::new();
    let mut timb = vec![];
    let mut form = 0;
    let mut i = 0;
    while i + 8 <= d.len() {
        let id = &d[i..i + 4];
        let n = u32::from_be_bytes([d[i + 4], d[i + 5], d[i + 6], d[i + 7]]) as usize;
        match id {
            b"FORM" | b"CAT " => {
                if id == b"FORM" {
                    form = i;
                }
                i += 12
            }
            b"TIMB" => {
                let b = &d[(i + 8).min(d.len())..(i + 8 + n).min(d.len())];
                let k = if b.len() >= 2 { u16::from_le_bytes([b[0], b[1]]) as usize } else { 0 };
                timb = b[2.min(b.len())..].chunks(2).take(k).filter(|p| p.len() == 2).map(|p| (p[1], p[0])).collect();
                i += 8 + n + (n & 1);
            }
            b"EVNT" => {
                let end = (i + 8 + n).min(d.len());
                let mut s = parse_evnt(&d[i + 8..end]);
                s.timb = std::mem::take(&mut timb);
                s.form = form;
                out.push(Arc::new(s));
                i = end + (n & 1);
            }
            _ => i += 8 + n + (n & 1),
        }
    }
    out
}
struct Player {
    seq: Arc<Seq>,
    idx: usize,
    wait: u32,
    loops: Vec<(usize, u8)>,
    done: bool,
}

/// The sequencer's note queue: 32 entries of (channel, key, ticks left).
const QUEUE: usize = 32;

pub struct Music {
    dir: Option<PathBuf>,
    bank: HashMap<(u8, u8), Timbre>,
    file: String,
    size: usize,
    seqs: Vec<Arc<Seq>>,
    /// The playing theme (-1 = none) and the wanted one (music.md "Theme state").
    playing: i32,
    wanted: i32,
    /// The Control Panel switch "Music" (JA1_MUSIC=0: off).
    pub on: bool,
    player: Option<Player>,
    /// The FM driver (created with the bank, then the AIL install reset) and the chip that plays its writes.
    drv: Option<Driver>,
    opl: Opl,
    queue: [Option<(u8, u8, u32)>; QUEUE],
    /// The last cc64 and cc112 values the sequence sent per channel (-1 = none).
    seq_sus: [i16; 16],
    seq_prot: [i16; 16],
    /// The last cc7 (volume) value the sequence sent per channel, before the scaling (sequence +0x394, -1 = none).
    seq_vol: [i16; 16],
    /// The AIL XMIDI master volume (MDI driver +0x1B0): 127 from the driver install on; only the volume keys set it.
    master: i32,
    /// The sequence volume and its fade (sequence +0x38 current, +0x3C target, +0x40 sum, +0x44 period in us) and the
    /// service count (+0x34). `AIL_API_init_sequence` 0x83EF0 starts them at 127, 127, 0, 0, 0.
    volume: i32,
    target: i32,
    fade_sum: i32,
    fade_period: i32,
    service: u32,
    /// musicfade_diff: the channel messages with their service tick.
    log: Option<Vec<[i32; 4]>>,
    log_tick: i32,
    /// Output samples into the current tick, notes started since the last `music` line.
    sub: f64,
    notes: u32,
    /// Theme starts (`FILE:t`) and stops (`stop`) since the last `music` line.
    changes: Vec<String>,
}

impl Default for Music {
    fn default() -> Music {
        Music {
            dir: None,
            bank: HashMap::new(),
            file: String::new(),
            size: 0,
            seqs: vec![],
            playing: -1,
            wanted: -1,
            on: std::env::var("JA1_MUSIC").map(|v| v != "0").unwrap_or(true),
            player: None,
            drv: None,
            opl: Opl::default(),
            queue: [None; QUEUE],
            seq_sus: [-1; 16],
            seq_prot: [-1; 16],
            seq_vol: [-1; 16],
            master: 127,
            volume: 127,
            target: 127,
            fade_sum: 0,
            fade_period: 0,
            service: 0,
            log: None,
            log_tick: 0,
            sub: 0.0,
            notes: 0,
            changes: vec![],
        }
    }
}

impl Music {
    fn read(&self, name: &str) -> Option<Vec<u8>> {
        let d = self.dir.as_ref()?;
        std::fs::read(d.join(name)).ok().or_else(|| std::fs::read(d.join(name.to_ascii_lowercase())).ok())
    }

    /// "Load a file", PlayXmiMusic 0x5C4C8: MISC\\<name>.XMI (the FM form). `name` is the short name ("INTRO") or the
    /// original's own argument in any case ("MISC\\intro": WinGameSequence?, film op 60).
    pub fn load(&mut self, name: &str) {
        if self.bank.is_empty() {
            if let Some(b) = self.read("MADLAB.AD") {
                self.bank = crate::fm::parse_bank(&b);
            }
        }
        let base = name.rsplit('\\').next().unwrap_or(name).to_ascii_uppercase();
        let d = self.dir.as_ref().map(|d| d.join("MISC")).and_then(|m| {
            std::fs::read(m.join(format!("{}.XMI", base))).ok().or_else(|| std::fs::read(m.join(format!("{}.xmi", base.to_ascii_lowercase()))).ok())
        });
        if let Some(d) = d {
            self.size = d.len();
            self.seqs = parse_xmi(&d);
            self.file = base;
        }
    }

    /// setmusic_diff: the ops on a Music with DAY loaded (see ja1f_music_ops).
    pub fn run_ops(exe: &str, ops: &[i32], out: &mut [i32]) -> i32 {
        let mut m = Music::default();
        m.dir = Some(std::path::Path::new(exe).parent().map(|p| p.to_path_buf()).unwrap_or_default());
        m.load("DAY");
        for (o, r) in ops.chunks(3).zip(out.chunks_mut(3)) {
            let before = m.changes.len();
            match o[0] {
                0 => m.play(o[1], o[2] != 0),
                1 => m.stop_theme(),
                _ => m.on = o[1] != 0,
            }
            let started = m.changes[before.min(m.changes.len())..].iter().rev().find_map(|c| c.split(':').nth(1)).and_then(|t| t.parse().ok());
            r[0] = m.playing();
            r[1] = m.wanted();
            r[2] = started.unwrap_or(-1);
        }
        m.seqs.len() as i32
    }

    /// music_file_diff: the short name and the size of the file that `load` takes.
    pub fn probe(exe: &str, name: &str) -> (String, usize) {
        let mut m = Music::default();
        m.dir = Some(std::path::Path::new(exe).parent().map(|p| p.to_path_buf()).unwrap_or_default());
        m.load(name);
        (m.file, m.size)
    }

    fn driver(&mut self) -> &mut Driver {
        if self.drv.is_none() {
            let mut d = Driver::new(self.bank.clone(), false);
            d.ail_reset();
            self.drv = Some(d);
        }
        self.drv.as_mut().unwrap()
    }

    /// The driver's register writes go to the chip.
    fn flush(&mut self) {
        let Some(d) = self.drv.as_mut() else { return };
        for (r, v) in d.out.drain(..) {
            self.opl.write(r, v);
        }
    }

    /// One channel message to the driver.
    fn send(&mut self, st: u8, d1: u8, d2: u8) {
        let t = self.log_tick;
        if let Some(l) = self.log.as_mut() {
            l.push([t, st as i32, d1 as i32, d2 as i32]);
        }
        let d = self.driver();
        let n0 = d.notes;
        d.msg(st, d1, d2);
        let n = d.notes - n0;
        self.notes += n;
        self.flush();
    }

    /// music.md "The sequencer": a playing sequence stops: a note-off for each queued note (queue order), then
    /// cc64 0 / cc112 0 on each channel where the sequence's last value was >= 64.
    fn stop(&mut self) {
        if self.player.as_ref().is_some_and(|p| !p.done) {
            self.halt();
        }
        self.player = None;
        self.playing = -1;
    }

    fn halt(&mut self) {
        for i in 0..QUEUE {
            if let Some((ch, key, _)) = self.queue[i].take() {
                self.send(0x80 | ch, key, 0);
            }
        }
        for ch in 0..16u8 {
            if self.seq_sus[ch as usize] >= 64 {
                self.send(0xB0 | ch, 64, 0);
            }
            if self.seq_prot[ch as usize] >= 64 {
                self.send(0xB0 | ch, 112, 0);
            }
        }
        if let Some(p) = self.player.as_mut() {
            p.done = true;
        }
    }

    /// "Play theme t" (steps 1..6); `_stop_first` changes nothing because the bar check never waits.
    pub fn play(&mut self, t: i32, _stop_first: bool) {
        if !self.on {
            if t > 0 {
                self.wanted = t;
            }
            return;
        }
        // t = 0 is the driver set-up sequence: SetMusic 0x5C5EC returns at once when word 0x95DC0 = 0 (the FM driver), and the
        // playing theme goes on.
        if t == 0 || t == self.playing {
            return;
        }
        if self.playing != -1 && t < 0 {
            self.changes.push("stop".into());
        }
        self.stop();
        if t <= 0 {
            return;
        }
        if let Some(seq) = self.seqs.get(t as usize).cloned() {
            self.driver().install(&seq.timb);
            self.flush();
            let wait = seq.evs.first().map(|e| e.0).unwrap_or(0);
            self.player = Some(Player { seq, idx: 0, wait, loops: vec![], done: false });
            self.queue = [None; QUEUE];
            self.seq_sus = [-1; 16];
            self.seq_prot = [-1; 16];
            self.seq_vol = [-1; 16];
            self.volume = 127;
            self.target = 127;
            self.fade_sum = 0;
            self.fade_period = 0;
            self.service = 0;
            self.playing = t;
            self.wanted = t;
            self.changes.push(format!("{}:{}", self.file, t));
        }
    }

    /// "Stop the music" (music.md "Who plays what"): the playing theme ends; the wanted one stays.
    pub fn stop_theme(&mut self) {
        if self.playing != -1 {
            self.changes.push("stop".into());
        }
        self.stop();
    }

    /// panel.md "The Control Panel", toggle step 4: the Music switch turned on plays the wanted theme (when > 0);
    /// turned off, it stops the playing theme (when > 0).
    pub fn switch(&mut self, on: bool) {
        self.on = on;
        if on {
            if self.wanted > 0 {
                self.play(self.wanted, false);
            }
        } else if self.playing > 0 {
            self.stop_theme();
        }
    }

    /// The playing theme (-1 = none).
    pub fn playing(&self) -> i32 {
        self.playing
    }

    /// QuickRestore 0x2C898 reads the wanted theme 0x96360 from the file.
    pub fn set_wanted(&mut self, t: i32) {
        self.wanted = t;
    }

    /// The wanted theme (-1 = none).
    pub fn wanted(&self) -> i32 {
        self.wanted
    }

    /// The sequence of the playing theme has not reached its end.
    pub fn still_playing(&self) -> bool {
        self.player.as_ref().is_some_and(|p| !p.done)
    }

    /// music.md "The tactical theme rule": `enemies` = enemies alive in the sector, `seen` = enemies in sight.
    pub fn tactical(&mut self, enemies: u8, seen: u32, santino_spoke: bool) {
        match self.playing {
            1 | 2 => {
                if enemies == 0 {
                    if self.playing == 2 {
                        self.play(4, false);
                    }
                } else if seen == 0 {
                    self.play(2, false);
                } else {
                    self.play(3, false);
                }
            }
            3 => {
                if seen == 0 && !santino_spoke {
                    self.play(if enemies != 0 { 4 } else { 5 }, false);
                }
            }
            4..=6 => {
                if !self.still_playing() {
                    self.play(1, false);
                }
            }
            _ => {}
        }
    }

    /// The `music` script line.
    pub fn line(&mut self) -> String {
        let n = std::mem::take(&mut self.notes);
        let c = std::mem::take(&mut self.changes);
        let f = if self.file.is_empty() { "-" } else { &self.file };
        format!("music {} {} notes {} changes {}", f, self.playing, n, if c.is_empty() { "-".to_string() } else { c.join(" ") })
    }

    /// One event of the sequence (music.md "What the sequencer sends").
    fn event(&mut self, ev: Ev) {
        match ev {
            Ev::Note { ch, key, vel, dur } => {
                self.send(0x90 | ch, key, vel);
                match self.queue.iter().position(|q| q.is_none()) {
                    Some(i) => self.queue[i] = Some((ch, key, dur)),
                    // "Internal note queue overflow": the sequence stops.
                    None => self.halt(),
                }
            }
            Ev::Prog { ch, p } => self.send(0xC0 | ch, p, 0),
            Ev::Bend { ch, lsb, msb } => self.send(0xE0 | ch, lsb, msb),
            Ev::Ctl { ch, c, v } => match c {
                // Kept by the sequencer: branch, channel lock, lock protect, loops, beat/bar clear, callback.
                109..=111 | 118 | 119 => {}
                116 => {
                    if let Some(p) = self.player.as_mut() {
                        if p.loops.len() < 4 {
                            p.loops.push((p.idx + 1, v));
                        }
                    }
                }
                117 => {
                    if v < 64 {
                        return;
                    }
                    if let Some(p) = self.player.as_mut() {
                        if let Some(l) = p.loops.last_mut() {
                            let back = if l.1 == 0 {
                                true
                            } else {
                                l.1 -= 1;
                                l.1 > 0
                            };
                            if back {
                                // The event after the loop start comes next (its delay counts again).
                                p.idx = l.0 - 1;
                            } else {
                                p.loops.pop();
                            }
                        }
                    }
                }
                6 => {
                    for (a, b) in [(100, 0), (101, 0), (38, 0), (6, v)] {
                        self.send(0xB0 | ch, a, b);
                    }
                }
                7 => {
                    self.seq_vol[ch as usize] = v as i16;
                    let v = scaled_cc7(v, self.volume, self.master);
                    self.send(0xB0 | ch, 7, v);
                }
                _ => {
                    if c == 64 {
                        self.seq_sus[ch as usize] = v as i16;
                    } else if c == 112 {
                        self.seq_prot[ch as usize] = v as i16;
                    }
                    self.send(0xB0 | ch, c, v);
                }
            },
            Ev::End => self.halt(),
        }
    }

    /// AIL_set_XMIDI_master_volume (0x845D0): a new master volume sends each channel's last cc7 again, scaled
    /// (0x82C10), while the sequence plays; the same value sends nothing.
    pub fn set_master(&mut self, m: i32) {
        if m == self.master {
            return;
        }
        self.master = m;
        if self.player.as_ref().is_some_and(|p| !p.done) {
            self.send_volumes();
        }
    }

    /// 0x82C10: each channel's last cc7 again, scaled with the volumes of now.
    fn send_volumes(&mut self) {
        for ch in 0..16u8 {
            let v = self.seq_vol[ch as usize];
            if v != -1 {
                let v = scaled_cc7(v as u8, self.volume, self.master);
                self.send(0xB0 | ch, 7, v);
            }
        }
    }

    /// SetSequenceVolume? 0x5C858 + AIL_API_set_sequence_volume 0x84500: the volume goes to `v` over `ms` (0 = at
    /// once). No sound when the Music option is off. The call has no check of the sequence status: after a stop the
    /// last cc7 values go to the driver again.
    pub fn set_volume(&mut self, v: i32, ms: i32) {
        if !self.on {
            return;
        }
        self.changes.push(format!("vol{}:{}", v, ms));
        self.target = v;
        if self.volume == v {
            return;
        }
        if ms == 0 {
            self.volume = v;
        } else {
            self.fade_period = ms.wrapping_mul(1000) / (self.volume - v).abs();
            self.fade_sum = 0;
        }
        self.send_volumes();
    }

    /// The end of the service 0x82C50 (0x832C0): a fade that runs moves the volume; each 8th service tick sends the
    /// channel volumes. A fade that ends between two such ticks does not send its last value.
    fn fade_step(&mut self) {
        if self.volume == self.target {
            return;
        }
        self.fade_sum += SERVICE_US;
        while self.fade_sum >= self.fade_period {
            self.fade_sum -= self.fade_period;
            self.volume += if self.target > self.volume { 1 } else { -1 };
            if self.volume == self.target {
                break;
            }
        }
        if self.service & 7 == 0 {
            self.send_volumes();
        }
    }

    pub fn master(&self) -> i32 {
        self.master
    }

    /// One 120 Hz tick: the queued notes whose time ran out end, then the events that are due.
    fn tick(&mut self) {
        if self.player.as_ref().is_none_or(|p| p.done) {
            return;
        }
        self.service = self.service.wrapping_add(1);
        for i in 0..QUEUE {
            if let Some((ch, key, left)) = self.queue[i] {
                if left <= 1 {
                    self.queue[i] = None;
                    self.send(0x80 | ch, key, 0);
                } else {
                    self.queue[i] = Some((ch, key, left - 1));
                }
            }
        }
        let mut guard = 0;
        loop {
            let (due, ev) = match self.player.as_ref() {
                Some(p) if !p.done && p.wait == 0 => (true, p.seq.evs[p.idx].1),
                _ => (false, Ev::End),
            };
            if !due {
                break;
            }
            self.event(ev);
            guard += 1;
            if let Some(p) = self.player.as_mut() {
                if p.done {
                    break;
                }
                p.idx += 1;
                if p.idx >= p.seq.evs.len() {
                    p.done = true;
                    break;
                }
                p.wait = p.seq.evs[p.idx].0;
            }
            if guard > 100000 {
                // An empty loop: stop instead of turning for ever.
                if let Some(p) = self.player.as_mut() {
                    p.done = true;
                }
                break;
            }
        }
        if let Some(p) = self.player.as_mut() {
            if p.wait > 0 {
                p.wait -= 1;
            }
            if !p.done {
                self.fade_step();
            }
        }
    }

    /// musicfade_diff: theme `t` of `file` runs for `ticks` service ticks. `ops` holds triples (tick, volume, ms):
    /// a set_volume before that tick; volume -4 = a new start of the theme, -3 = stop_theme,
    /// -2 / -1 = Music option off / on, -5 = set_master(ms field). `out` gets (tick, status, d1, d2) per channel message; returns the count.
    pub fn run_fade(exe: &str, file: &str, t: i32, ops: &[i32], ticks: i32, out: &mut [i32]) -> i32 {
        let mut m = Music::default();
        m.dir = Some(std::path::Path::new(exe).parent().map(|p| p.to_path_buf()).unwrap_or_default());
        m.load(file);
        m.play(t, false);
        m.log = Some(vec![]);
        for k in 0..ticks {
            m.log_tick = k;
            for o in ops.chunks(3).filter(|o| o[0] == k) {
                match o[1] {
                    -4 => {
                        if m.on {
                            m.stop_theme();
                            m.play(t, false);
                        }
                    }
                    -3 => m.stop_theme(),
                    -2 => m.on = false,
                    -1 => m.on = true,
                    -5 => m.set_master(o[2]),
                    v => m.set_volume(v, o[2]),
                }
            }
            m.tick();
        }
        let l = m.log.take().unwrap_or_default();
        for (e, o) in l.iter().zip(out.chunks_mut(4)) {
            o.copy_from_slice(e);
        }
        l.len() as i32
    }

    /// Play `n` output samples of time without sound (a play script without JA1_WAV): the sequence and the
    /// theme rule go on as with `render`.
    pub fn advance(&mut self, n: usize) {
        let per_tick = RATE as f64 / TICK_HZ;
        let mut left = n as f64;
        while left > 0.0 {
            if self.sub <= 0.0 {
                self.tick();
                self.sub += per_tick;
            }
            let step = self.sub.min(left);
            self.sub -= step;
            left -= step;
        }
    }

    /// Add `out.len()` output samples of music to `out`.
    pub fn render(&mut self, out: &mut [i32]) {
        if self.player.is_none() && self.opl.silent() {
            return;
        }
        let per_tick = RATE as f64 / TICK_HZ;
        for o in out.iter_mut() {
            if self.sub <= 0.0 {
                self.tick();
                self.sub += per_tick;
            }
            self.sub -= 1.0;
            *o += (self.opl.sample() * VOICE_SCALE) as i32;
        }
    }
}

/// The cc7 value the driver gets (0x822E0, controller 7): v * sequence volume * master / 16129, held to 0..127.
pub fn scaled_cc7(v: u8, volume: i32, master: i32) -> u8 {
    (v as i32 * volume * master / 0x3F01).clamp(0, 127) as u8
}

fn global() -> &'static Mutex<Music> {
    static M: OnceLock<Mutex<Music>> = OnceLock::new();
    M.get_or_init(|| Mutex::new(Music::default()))
}

/// Run `f` on the process's music state.
pub fn with<R>(f: impl FnOnce(&mut Music) -> R) -> R {
    let mut g = global().lock().unwrap_or_else(|e| e.into_inner());
    f(&mut g)
}

/// The game's hard-disk folder: the folder of `exe` (JA.EXE) holds MISC\ and MADLAB.AD.
pub fn set_exe(exe: &str) {
    let d = std::path::Path::new(exe).parent().map(|p| p.to_path_buf()).unwrap_or_default();
    with(|m| {
        if m.dir.as_ref() != Some(&d) {
            m.dir = Some(d);
            m.bank.clear();
        }
    });
}

/// Load `file` (INTRO, MENUS, DAY) when it is not the loaded one, then play `theme` (music.md "Who plays what").
pub fn screen(file: &str, theme: i32) {
    with(|m| {
        if m.file != file {
            m.load(file);
        }
        m.play(theme, false);
    });
}

/// True while a window's audio callback renders the music (a play script mixes it into the sound mix instead).
pub static DEVICE: std::sync::atomic::AtomicBool = std::sync::atomic::AtomicBool::new(false);

/// music.md "Output": the window opens its music device on every real SDL audio driver; on the "dummy" driver (the
/// tests) only with JA1_DUMMY_SCREEN_AUDIO (`screen_audio`).
pub fn opens_device(driver: &str, screen_audio: bool) -> bool {
    driver != "dummy" || screen_audio
}

#[cfg(test)]
mod tests {
    /// Ledger row c2aea50:ja1front:20553 (MIDI variable length): a byte with bit 7 set (0x80 too) goes on, the
    /// first byte below 0x80 ends the number.
    #[test]
    fn varlen_goes_on_through_0x80() {
        let mut p = 0;
        assert_eq!(super::varlen(&[0x81, 0x80, 0x00, 0x7F], &mut p), Some(0x4000));
        assert_eq!(p, 3);
    }
    /// Ledger row c2aea50:ja1front:823: a player with a sound card hears the music.
    #[test]
    fn music_opens_on_a_real_driver() {
        assert!(super::opens_device("pulseaudio", false));
        assert!(super::opens_device("alsa", true));
        assert!(!super::opens_device("dummy", false));
        assert!(super::opens_device("dummy", true));
    }

    use super::*;

    #[test]
    fn master_volume_scale() {
        // At the start masters (127) the cc7 values pass unchanged; 0 mutes; the result stays in 0..127.
        for v in 0..=127u8 {
            assert_eq!(scaled_cc7(v, 127, 127), v);
            assert_eq!(scaled_cc7(v, 127, 0), 0);
            assert_eq!(crate::sound::scaled_volume(v as i32, 127), v as i32);
        }
        assert_eq!(scaled_cc7(100, 127, 100), 78);
        assert_eq!(crate::sound::scaled_volume(127, 70), 70);
        let mut v = crate::sound::Volumes { music: 100, sound: 0 };
        assert_eq!(crate::sound::volume_key(b'+', &mut v), Some(crate::sound::MasterSet::Music(100)));
        assert_eq!(crate::sound::volume_key(b',', &mut v), Some(crate::sound::MasterSet::Digital(0)));
        assert_eq!(crate::sound::volume_key(b'V', &mut v), None);
    }

    #[test]
    fn loop_and_notes() {
        // Theme 1: loop start, a note, a delay of 10, loop end (back), end.
        let evnt = [0xB0, 116, 0, 0x90, 60, 100, 5, 10, 0xB0, 117, 64, 0xFF, 0x2F, 0];
        let mut s = parse_evnt(&evnt);
        assert_eq!(s.evs.len(), 4);
        s.timb = vec![(0, 0)];
        let mut m = Music::default();
        m.on = true;
        m.bank.insert((0, 0), Timbre { transpose: 0, t: [0x21, 0x10, 0xF2, 0x44, 0, 0x0A, 0x21, 0x00, 0xF2, 0x44, 0] });
        m.seqs = vec![Arc::new(parse_evnt(&[0xFF, 0x2F, 0])), Arc::new(s)];
        m.file = "T".into();
        m.play(1, false);
        let mut buf = vec![0i32; (RATE as usize) / 2];
        m.render(&mut buf);
        // 60 ticks: the loop (10 ticks long) went round 6 times.
        assert_eq!(m.line(), "music T 1 notes 6 changes T:1");
        assert!(buf.iter().any(|v| v.abs() > 100));
        assert!(m.still_playing());
        // Tactical rule: theme 1 with enemies in sight goes to 3; 3 with none in sight and none left goes to 5.
        m.seqs.extend((2..7).map(|_| Arc::new(parse_evnt(&[0x90, 60, 100, 5, 30, 0xFF, 0x2F, 0]))));
        m.tactical(2, 1, false);
        assert_eq!(m.playing(), 3);
        m.tactical(0, 0, false);
        assert_eq!(m.playing(), 5);
        let mut buf = vec![0i32; RATE as usize];
        m.render(&mut buf);
        assert!(!m.still_playing());
        m.tactical(0, 0, false);
        assert_eq!(m.playing(), 1);
    }
}

#[cfg(test)]
mod listen {
    #[test]
    #[ignore]
    fn render_theme() {
        let (file, theme): (String, i32) = (std::env::var("MF").unwrap(), std::env::var("MT").unwrap().parse().unwrap());
        let exe = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../rom/JA.EXE");
        super::set_exe(&exe.to_string_lossy());
        super::screen(&file, theme);
        let mut buf = vec![0i32; crate::sound::RATE as usize * std::env::var("MS").map(|s| s.parse().unwrap()).unwrap_or(20)];
        super::with(|m| m.render(&mut buf));
        let pcm: Vec<i16> = buf.iter().map(|v| (*v).clamp(-32768, 32767) as i16).collect();
        let clip = buf.iter().filter(|v| v.abs() > 32767).count();
        let rms = (buf.iter().map(|v| (*v as f64).powi(2)).sum::<f64>() / buf.len() as f64).sqrt();
        eprintln!("{} {} rms {:.0} peak {} clip {} {}", file, theme, rms, buf.iter().map(|v| v.abs()).max().unwrap(), clip, super::with(|m| m.line()));
        std::fs::write(std::env::temp_dir().join("music.wav"), crate::sound::wav_bytes(&pcm)).unwrap();
    }
}
