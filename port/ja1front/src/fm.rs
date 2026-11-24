//! The music card's FM driver (docs/spec/music.md "The FM driver (SBPRO1.MDI)"): MIDI channel messages in,
//! OPL2 register writes out (`Driver`), and an imitation of the chip that plays one register set (`Opl`).
use std::collections::HashMap;

use crate::sound::RATE;

/// An FM timbre of MADLAB.AD: the transpose byte t[2] and t[3..13].
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Timbre {
    pub transpose: i8,
    pub t: [u8; 11],
}

impl Timbre {
    /// Byte t[i] of the 14-byte entry (i = 3..13).
    fn b(&self, i: usize) -> u8 {
        self.t[i - 3]
    }
}

/// music.md "Timbres": the bank file's entries, by (bank, patch).
pub fn parse_bank(d: &[u8]) -> HashMap<(u8, u8), Timbre> {
    let mut m = HashMap::new();
    let mut i = 0;
    while i + 6 <= d.len() {
        let (patch, bank) = (d[i], d[i + 1]);
        if patch == 0xFF && bank == 0xFF {
            break;
        }
        let off = u32::from_le_bytes([d[i + 2], d[i + 3], d[i + 4], d[i + 5]]) as usize;
        if off + 14 <= d.len() {
            let mut t = [0u8; 11];
            t.copy_from_slice(&d[off + 3..off + 14]);
            m.insert((bank, patch), Timbre { transpose: d[off + 2] as i8, t });
        }
        i += 6;
    }
    m
}

/// The start table: the value of register r (written for r = 0x01..0xF5).
fn init_value(r: u8) -> u8 {
    match r {
        0x01 => 0x20,
        0x04 => 0x60,
        0x20..=0x35 => 0x01,
        0x40..=0x55 => 0x3F,
        0x60..=0x75 => 0xFF,
        0x80..=0x95 => 0x0F,
        0xBD => 0xC0,
        _ => 0,
    }
}

const VEL: [u8; 16] = [0x52, 0x55, 0x58, 0x5B, 0x5E, 0x61, 0x64, 0x67, 0x6A, 0x6D, 0x70, 0x73, 0x76, 0x79, 0x7C, 0x7F];
/// Operator offset of the modulator of voices 0..8 (carrier = +3).
const OP_OFF: [u8; 9] = [0x00, 0x01, 0x02, 0x08, 0x09, 0x0A, 0x10, 0x11, 0x12];

#[rustfmt::skip]
const FNUM: [u16; 192] = [
    0x02B2, 0x02B4, 0x02B7, 0x02B9, 0x02BC, 0x02BE, 0x02C1, 0x02C3, 0x02C6, 0x02C9, 0x02CB, 0x02CE, 0x02D0, 0x02D3, 0x02D6, 0x02D8,
    0x02DB, 0x02DD, 0x02E0, 0x02E3, 0x02E5, 0x02E8, 0x02EB, 0x02ED, 0x02F0, 0x02F3, 0x02F6, 0x02F8, 0x02FB, 0x02FE, 0x0301, 0x0303,
    0x0306, 0x0309, 0x030C, 0x030F, 0x0311, 0x0314, 0x0317, 0x031A, 0x031D, 0x0320, 0x0323, 0x0326, 0x0329, 0x032B, 0x032E, 0x0331,
    0x0334, 0x0337, 0x033A, 0x033D, 0x0340, 0x0343, 0x0346, 0x0349, 0x034C, 0x034F, 0x0352, 0x0356, 0x0359, 0x035C, 0x035F, 0x0362,
    0x0365, 0x0368, 0x036B, 0x036F, 0x0372, 0x0375, 0x0378, 0x037B, 0x037F, 0x0382, 0x0385, 0x0388, 0x038C, 0x038F, 0x0392, 0x0395,
    0x0399, 0x039C, 0x039F, 0x03A3, 0x03A6, 0x03A9, 0x03AD, 0x03B0, 0x03B4, 0x03B7, 0x03BB, 0x03BE, 0x03C1, 0x03C5, 0x03C8, 0x03CC,
    0x03CF, 0x03D3, 0x03D7, 0x03DA, 0x03DE, 0x03E1, 0x03E5, 0x03E8, 0x03EC, 0x03F0, 0x03F3, 0x03F7, 0x03FB, 0x03FE, 0xFE01, 0xFE03,
    0xFE05, 0xFE07, 0xFE08, 0xFE0A, 0xFE0C, 0xFE0E, 0xFE10, 0xFE12, 0xFE14, 0xFE16, 0xFE18, 0xFE1A, 0xFE1C, 0xFE1E, 0xFE20, 0xFE21,
    0xFE23, 0xFE25, 0xFE27, 0xFE29, 0xFE2B, 0xFE2D, 0xFE2F, 0xFE31, 0xFE34, 0xFE36, 0xFE38, 0xFE3A, 0xFE3C, 0xFE3E, 0xFE40, 0xFE42,
    0xFE44, 0xFE46, 0xFE48, 0xFE4A, 0xFE4C, 0xFE4F, 0xFE51, 0xFE53, 0xFE55, 0xFE57, 0xFE59, 0xFE5C, 0xFE5E, 0xFE60, 0xFE62, 0xFE64,
    0xFE67, 0xFE69, 0xFE6B, 0xFE6D, 0xFE6F, 0xFE72, 0xFE74, 0xFE76, 0xFE79, 0xFE7B, 0xFE7D, 0xFE7F, 0xFE82, 0xFE84, 0xFE86, 0xFE89,
    0xFE8B, 0xFE8D, 0xFE90, 0xFE92, 0xFE95, 0xFE97, 0xFE99, 0xFE9C, 0xFE9E, 0xFEA1, 0xFEA3, 0xFEA5, 0xFEA8, 0xFEAA, 0xFEAD, 0xFEAF,
];

/// The AIL install reset: the messages sent to each channel at the driver install.
pub const AIL_RESET: [(u8, u8, u8); 15] = [
    (0xB0, 114, 0), (0xC0, 0, 0), (0xE0, 0, 0x40), (0xB0, 112, 0), (0xB0, 1, 0), (0xB0, 7, 127), (0xB0, 10, 64),
    (0xB0, 11, 127), (0xB0, 64, 0), (0xB0, 91, 40), (0xB0, 93, 0), (0xB0, 100, 0), (0xB0, 101, 0), (0xB0, 38, 0),
    (0xB0, 6, 2),
];

#[derive(Clone, Copy, Default)]
struct Chan {
    vol: u8,
    expr: u8,
    pan: u8,
    range: u8,
    lsb: u8,
    msb: u8,
    cc1: u8,
    cc112: u8,
    sus: u8,
    bank: u8,
    patch: u8,
    timbre: Option<Timbre>,
}

#[derive(Clone, Copy, Default)]
struct Slot {
    on: bool,
    ch: u8,
    key: u8,
    /// The sounding key (channel 10: the timbre's transpose byte) and the transpose added to it.
    skey: u8,
    tr: i8,
    vg: u8,
    sus: bool,
    voice: Option<u8>,
    /// The timbre at the note start.
    timbre: Option<Timbre>,
}

fn gain(a: u8, b: u8) -> u8 {
    let g = (a as u32 * b as u32 * 2) >> 8;
    (if g != 0 { g + 1 } else { 0 }) as u8
}

fn pan_gain(i: u8) -> u8 {
    if i < 63 { 2 * i } else { 127 }
}

fn level(b: u8, g: u8) -> u8 {
    let l = 63 - (b & 0x3F) as u32;
    (b & 0xC0) | (63 - (l * g as u32 / 127)) as u8
}

/// The driver: channel state, 16 note slots, 9 voices. Each register write goes to `regs` (the right value wins,
/// music.md "The two chips") and, when `log` is on, to `writes` as (chip 0 left / 1 right, register, value).
pub struct Driver {
    bank: HashMap<(u8, u8), Timbre>,
    installed: HashMap<(u8, u8), Timbre>,
    chans: [Chan; 16],
    slots: [Slot; 16],
    voice_used: [bool; 9],
    last_b0: [u8; 9],
    rr: i32,
    pub log: bool,
    pub writes: Vec<(u8, u8, u8)>,
    /// Register writes for the chip (register, value), in order.
    pub out: Vec<(u8, u8)>,
    /// Note-ons that sounded (got a slot).
    pub notes: u32,
}

impl Driver {
    /// The driver after its start writes; `log` keeps every write in `writes`.
    pub fn new(bank: HashMap<(u8, u8), Timbre>, log: bool) -> Driver {
        let mut d = Driver {
            bank,
            installed: HashMap::new(),
            chans: [Chan { patch: 0xFF, ..Chan::default() }; 16],
            slots: [Slot::default(); 16],
            voice_used: [false; 9],
            last_b0: [0; 9],
            rr: -1,
            log,
            writes: vec![],
            out: vec![],
            notes: 0,
        };
        for r in 0x01..=0xF5u8 {
            d.w(r, init_value(r), init_value(r));
        }
        d
    }

    /// The install reset of every channel (music.md "Start").
    pub fn ail_reset(&mut self) {
        for ch in 0..16u8 {
            for (s, a, b) in AIL_RESET {
                self.msg(s | ch, a, b);
            }
        }
    }

    fn w(&mut self, reg: u8, l: u8, r: u8) {
        if self.log {
            self.writes.push((0, reg, l));
            self.writes.push((1, reg, r));
        }
        self.out.push((reg, r));
    }

    /// Install the timbres (bank, patch) in order; false when one is not in the bank (the install stops there).
    pub fn install(&mut self, list: &[(u8, u8)]) -> bool {
        for &k in list {
            if self.installed.contains_key(&k) {
                continue;
            }
            let Some(t) = self.bank.get(&k).copied() else { return false };
            self.installed.insert(k, t);
            for c in self.chans.iter_mut() {
                if (c.bank, c.patch) == k {
                    c.timbre = Some(t);
                }
            }
        }
        true
    }

    /// The timbre a slot plays (the one at its note start).
    fn slot_timbre(&self, s: &Slot) -> Option<Timbre> {
        s.timbre
    }

    fn pitch(&self, s: &Slot) -> (u8, u8) {
        let c = &self.chans[s.ch as usize];
        let bend = ((((c.msb as i32) << 7) | c.lsb as i32) - 0x2000) >> 5;
        let bend = (bend * c.range as i32) as i16;
        let n = s.skey as i32 + s.tr as i32;
        let mut k = n - 12;
        while k < 0 {
            k += 12;
        }
        while k > 95 {
            k -= 12;
        }
        let mut a = ((bend.wrapping_add((k << 8) as i16)).wrapping_add(8) >> 4) as i32;
        while a < 0 {
            a += 0xC0;
        }
        while a > 0x5FF {
            a -= 0xC0;
        }
        let (s16, f) = (a >> 4, a & 15);
        let mut fnum = FNUM[((s16 % 12) * 16 + f) as usize] as i16 as i32;
        let mut block = s16 / 12 - 1;
        if fnum & 0x8000 != 0 {
            block += 1;
        }
        if block < 0 {
            block = 0;
            fnum >>= 1;
        }
        ((fnum & 0xFF) as u8, (((fnum >> 8) & 3) as u8) | (block as u8) << 2 | 0x20)
    }

    fn write_20(&mut self, v: u8, t: &Timbre, ch: u8) {
        let vib = if self.chans[ch as usize].cc1 >= 64 { 0x40 } else { 0 };
        let m = OP_OFF[v as usize];
        self.w(0x20 + m, t.b(3) | vib, t.b(3) | vib);
        self.w(0x23 + m, t.b(9) | vib, t.b(9) | vib);
    }

    fn write_levels(&mut self, v: u8, t: &Timbre, s: &Slot) {
        let c = self.chans[s.ch as usize];
        let g = gain(gain(c.vol, c.expr), s.vg);
        let (gl, gr) = (gain(pan_gain(c.pan), g), gain(pan_gain(127 - c.pan.min(127)), g));
        let m = OP_OFF[v as usize];
        let mb = t.b(4);
        if t.b(8) & 1 != 0 {
            self.w(0x40 + m, level(mb, gl), level(mb, gr));
        } else {
            self.w(0x40 + m, mb, mb);
        }
        let cb = t.b(10);
        self.w(0x43 + m, level(cb, gl), level(cb, gr));
    }

    fn write_pitch(&mut self, v: u8, s: &Slot) {
        let (a0, b0) = self.pitch(s);
        self.w(0xA0 + v, a0, a0);
        self.w(0xB0 + v, b0, b0);
        self.last_b0[v as usize] = b0;
    }

    /// All registers of slot `i`'s voice (a note start).
    fn voice_all(&mut self, i: usize) {
        let s = self.slots[i];
        let (Some(v), Some(t)) = (s.voice, self.slot_timbre(&s)) else { return };
        let m = OP_OFF[v as usize];
        self.write_20(v, &t, s.ch);
        self.write_levels(v, &t, &s);
        self.w(0x60 + m, t.b(5), t.b(5));
        self.w(0x63 + m, t.b(11), t.b(11));
        self.w(0x80 + m, t.b(6), t.b(6));
        self.w(0x83 + m, t.b(12), t.b(12));
        self.w(0xE3 + m, t.b(13), t.b(13));
        self.w(0xE0 + m, t.b(7), t.b(7));
        self.w(0xC0 + v, t.b(8) & 0x0F, t.b(8) & 0x0F);
        self.write_pitch(v, &s);
    }

    /// Key-off of slot i's voice; the voice and the slot are freed.
    fn end_slot(&mut self, i: usize) {
        if let Some(v) = self.slots[i].voice {
            let b0 = self.last_b0[v as usize] & !0x20;
            self.w(0xB0 + v, b0, b0);
            self.last_b0[v as usize] = b0;
            self.voice_used[v as usize] = false;
        }
        self.slots[i] = Slot::default();
    }

    fn steal(&mut self) {
        let mut p = [0i32; 16];
        let mut active = 0;
        for i in 0..16 {
            let s = self.slots[i];
            if !s.on {
                continue;
            }
            active += 1;
            let held = self.slots.iter().filter(|x| x.on && x.voice.is_some() && x.ch == s.ch).count() as i32;
            let base = if self.chans[s.ch as usize].cc112 >= 64 { 0xFFFF } else { 0x7FFF };
            p[i] = (base - held).max(0);
        }
        for _ in 0..active {
            let mut u: Option<usize> = None;
            let mut v: Option<usize> = None;
            for i in 0..16 {
                let s = self.slots[i];
                if !s.on {
                    continue;
                }
                if s.voice.is_none() {
                    if u.is_none_or(|j| p[i] >= p[j]) {
                        u = Some(i);
                    }
                } else if v.is_none_or(|j| p[i] <= p[j]) {
                    v = Some(i);
                }
            }
            let (Some(u), Some(vs)) = (u, v) else { break };
            if p[u] < p[vs] || p[u] == 0 {
                break;
            }
            let voice = self.slots[vs].voice;
            self.end_slot(vs);
            if let Some(x) = voice {
                self.voice_used[x as usize] = true;
            }
            self.slots[u].voice = voice;
            self.voice_all(u);
        }
    }

    fn note_on(&mut self, ch: u8, key: u8, vel: u8) {
        let (t, skey, tr) = if ch == 9 {
            match self.installed.get(&(127, key)) {
                Some(t) => (*t, t.transpose as u8, 0),
                None => return,
            }
        } else {
            match self.chans[ch as usize].timbre {
                Some(t) => (t, key, t.transpose),
                None => return,
            }
        };
        let Some(i) = self.slots.iter().position(|s| !s.on) else { return };
        self.notes += 1;
        self.slots[i] = Slot { on: true, ch, key, skey, tr, vg: VEL[(vel >> 3) as usize], sus: false, voice: None, timbre: Some(t) };
        for _ in 0..9 {
            self.rr = (self.rr + 1) % 9;
            if !self.voice_used[self.rr as usize] {
                self.voice_used[self.rr as usize] = true;
                self.slots[i].voice = Some(self.rr as u8);
                self.voice_all(i);
                return;
            }
        }
        self.steal();
    }

    fn note_off(&mut self, ch: u8, key: u8) {
        for i in 0..16 {
            let s = self.slots[i];
            if !s.on || s.ch != ch || s.key != key {
                continue;
            }
            if self.chans[ch as usize].sus >= 64 {
                self.slots[i].sus = true;
            } else {
                self.end_slot(i);
            }
        }
    }

    /// Rewrite registers of every voiced slot of channel `ch`: `what` bit 0 = 20, bit 1 = levels, bit 2 = pitch.
    fn rewrite(&mut self, ch: u8, what: u8) {
        for i in 0..16 {
            let s = self.slots[i];
            if !s.on || s.ch != ch {
                continue;
            }
            let (Some(v), Some(t)) = (s.voice, self.slot_timbre(&s)) else { continue };
            if what & 1 != 0 {
                self.write_20(v, &t, ch);
            }
            if what & 2 != 0 {
                self.write_levels(v, &t, &s);
            }
            if what & 4 != 0 {
                self.write_pitch(v, &s);
            }
        }
    }

    fn release_sustained(&mut self, ch: u8) {
        for i in 0..16 {
            let s = self.slots[i];
            if s.on && s.ch == ch && s.sus {
                self.note_off(ch, s.skey);
            }
        }
    }

    /// One MIDI channel message.
    pub fn msg(&mut self, st: u8, d1: u8, d2: u8) {
        let ch = st & 15;
        let c = ch as usize;
        match st & 0xF0 {
            0x80 => self.note_off(ch, d1),
            0x90 => {
                if d2 == 0 {
                    self.note_off(ch, d1)
                } else {
                    self.note_on(ch, d1, d2)
                }
            }
            0xC0 => {
                let k = (self.chans[c].bank, d1);
                self.chans[c].patch = d1;
                self.chans[c].timbre = self.installed.get(&k).copied();
            }
            0xE0 => {
                self.chans[c].lsb = d1;
                self.chans[c].msb = d2;
                self.rewrite(ch, 4);
            }
            0xB0 => match d1 {
                1 => {
                    self.chans[c].cc1 = d2;
                    self.rewrite(ch, 1);
                }
                6 => self.chans[c].range = d2,
                7 => {
                    self.chans[c].vol = d2;
                    self.rewrite(ch, 2);
                }
                10 => {
                    self.chans[c].pan = d2;
                    self.rewrite(ch, 2);
                }
                11 => {
                    self.chans[c].expr = d2;
                    self.rewrite(ch, 2);
                }
                64 => {
                    self.chans[c].sus = d2;
                    if d2 < 64 {
                        self.release_sustained(ch);
                    }
                }
                112 => self.chans[c].cc112 = d2,
                114 => self.chans[c].bank = d2,
                121 => {
                    self.chans[c].sus = 0;
                    self.release_sustained(ch);
                    let cs = &mut self.chans[c];
                    cs.cc1 = 0;
                    cs.expr = 127;
                    cs.lsb = 0;
                    cs.msb = 0x40;
                    self.rewrite(ch, 7);
                }
                123 => {
                    for i in 0..16 {
                        let s = self.slots[i];
                        if s.on && s.ch == ch {
                            self.note_off(ch, s.skey);
                        }
                    }
                }
                _ => {}
            },
            _ => {}
        }
    }
}

// ---- The chip imitation (YM3812 behaviour, not the game's) ----

#[derive(Clone, Copy, PartialEq)]
enum Stage {
    Attack,
    Decay,
    Sustain,
    Release,
    Off,
}

/// One FM operator, set from its 20/40/60/80/E0 registers.
#[derive(Clone, Copy)]
struct Op {
    am: bool,
    vib: bool,
    hold: bool,
    mult: f32,
    tl: f32,
    ksl_db: f32,
    ar: f32,
    dr: f32,
    sl: f32,
    rr: f32,
    ws: u8,
    phase: f32,
    att: f32,
    stage: Stage,
    out: [f32; 2],
}

const MULT: [f32; 16] = [0.5, 1.0, 2.0, 3.0, 4.0, 5.0, 6.0, 7.0, 8.0, 9.0, 10.0, 10.0, 12.0, 12.0, 15.0, 15.0];
const KSL_DB: [f32; 4] = [0.0, 3.0, 1.5, 6.0];
/// The key scale steps of the upper 4 bits of the frequency number (DBOPL KslCreateTable; the chip's own table).
const KSL_STEPS: [u8; 16] = [64, 32, 24, 19, 16, 12, 11, 10, 8, 6, 5, 4, 3, 2, 1, 0];

/// The attack per output sample as a part of (att + 0.1875 dB), or dB per output sample of a decay/release, of OPL rate
/// `r` (0..15). DBOPL TemplateVolume: the attack is vol += (~vol * change) >> 3, so the level falls on an exponential curve to
/// (att + 1 step of 0.1875 dB); 4.8 time constants per table time = measured on DBOPL (validate/diff/oplchip_diff.py).
fn rate_step(r: u8, rof: u8, attack: bool) -> f32 {
    if r == 0 {
        return 0.0;
    }
    let eff = (4 * r as u32 + rof as u32).min(63) as f32;
    if attack {
        if eff >= 60.0 {
            return f32::INFINITY;
        }
        // 2826 ms from silence to full at rate 1, halved per rate step.
        let ms = 2826.24 / 2f32.powf((eff - 4.0) / 4.0);
        4.8 / (ms / 1000.0 * RATE as f32)
    } else {
        // 39280 ms for 96 dB at rate 1, halved per rate step.
        let ms = 39280.64 / 2f32.powf((eff - 4.0) / 4.0);
        96.0 / (ms / 1000.0 * RATE as f32)
    }
}

impl Op {
    fn off() -> Op {
        Op {
            am: false,
            vib: false,
            hold: false,
            mult: 1.0,
            tl: 63.0,
            ksl_db: 0.0,
            ar: 0.0,
            dr: 0.0,
            sl: 0.0,
            rr: 0.0,
            ws: 0,
            phase: 0.0,
            att: 96.0,
            stage: Stage::Off,
            out: [0.0; 2],
        }
    }

    /// Registers 20, 40, 60, 80, E0 of the operator; `fnum` and `block` of the voice.
    fn set(&mut self, r: [u8; 5], fnum: u32, block: u8, ws_on: bool) {
        let rof = if r[0] & 0x10 != 0 { block * 2 } else { block / 2 };
        self.am = r[0] & 0x80 != 0;
        self.vib = r[0] & 0x40 != 0;
        self.hold = r[0] & 0x20 != 0;
        self.mult = MULT[(r[0] & 15) as usize];
        self.tl = (r[1] & 63) as f32;
        // DBOPL UpdateAttenuation: 8 steps per block minus the table step, a full octave = KSL_DB dB.
        let steps = (block as i32 * 8 - KSL_STEPS[(fnum >> 6) as usize & 15] as i32).max(0);
        self.ksl_db = KSL_DB[(r[1] >> 6) as usize] * steps as f32 / 8.0;
        self.ar = rate_step(r[2] >> 4, rof, true);
        self.dr = rate_step(r[2] & 15, rof, false);
        self.sl = if r[3] >> 4 == 15 { 93.0 } else { (r[3] >> 4) as f32 * 3.0 };
        self.rr = rate_step(r[3] & 15, rof, false);
        self.ws = if ws_on { r[4] & 3 } else { 0 };
    }

    fn envelope(&mut self) {
        match self.stage {
            Stage::Attack => {
                if self.ar.is_infinite() {
                    self.att = 0.0;
                } else {
                    self.att -= (self.att + 0.1875) * self.ar;
                }
                if self.att <= 0.0 {
                    self.att = 0.0;
                    self.stage = Stage::Decay;
                }
            }
            Stage::Decay => {
                self.att += self.dr;
                if self.att >= self.sl {
                    self.att = self.sl;
                    self.stage = Stage::Sustain;
                }
            }
            Stage::Sustain => {
                if !self.hold {
                    self.att += self.rr;
                }
            }
            Stage::Release => self.att += self.rr,
            Stage::Off => {}
        }
        if self.att >= 96.0 && self.stage != Stage::Attack {
            self.att = 96.0;
            self.stage = Stage::Off;
        }
    }

    /// One sample: phase advance `step` (cycles), modulation `m` (cycles), extra attenuation `extra` (dB).
    fn sample(&mut self, step: f32, m: f32, extra: f32) -> f32 {
        self.envelope();
        self.phase = (self.phase + step * self.mult).fract();
        let ph = (self.phase + m).rem_euclid(1.0);
        let s = (ph * std::f32::consts::TAU).sin();
        let w = match self.ws {
            0 => s,
            1 => s.max(0.0),
            2 => s.abs(),
            _ => {
                if ph.fract() % 0.5 < 0.25 {
                    s.abs()
                } else {
                    0.0
                }
            }
        };
        let att = self.att + self.tl * 0.75 + self.ksl_db + extra;
        let v = if att >= 96.0 { 0.0 } else { w * 10f32.powf(-att / 20.0) };
        self.out = [self.out[1], v];
        v
    }
}

/// The chip: one register set, 9 two-operator voices, mono.
pub struct Opl {
    regs: [u8; 256],
    ops: [[Op; 2]; 9],
    t: f32,
}

impl Default for Opl {
    fn default() -> Opl {
        Opl { regs: [0; 256], ops: [[Op::off(); 2]; 9], t: 0.0 }
    }
}

impl Opl {
    fn freq(&self, v: usize) -> (f32, u8) {
        let fnum = self.regs[0xA0 + v] as u32 | ((self.regs[0xB0 + v] as u32 & 3) << 8);
        let block = (self.regs[0xB0 + v] >> 2) & 7;
        (fnum as f32 * 49716.0 / (1u32 << (20 - block)) as f32, block)
    }

    fn update(&mut self, v: usize) {
        let (_, block) = self.freq(v);
        let fnum = self.regs[0xA0 + v] as u32 | ((self.regs[0xB0 + v] as u32 & 3) << 8);
        let ws_on = self.regs[1] & 0x20 != 0;
        for o in 0..2 {
            let off = OP_OFF[v] as usize + 3 * o;
            let r = [self.regs[0x20 + off], self.regs[0x40 + off], self.regs[0x60 + off], self.regs[0x80 + off], self.regs[0xE0 + off]];
            self.ops[v][o].set(r, fnum, block, ws_on);
        }
    }

    pub fn write(&mut self, reg: u8, val: u8) {
        let old = self.regs[reg as usize];
        self.regs[reg as usize] = val;
        let r = reg as usize;
        match r {
            0x20..=0x95 | 0xE0..=0xF5 => {
                let off = (r & 0x1F) as u8;
                for v in 0..9 {
                    if off == OP_OFF[v] || off == OP_OFF[v] + 3 {
                        self.update(v);
                    }
                }
            }
            0xA0..=0xA8 => self.update(r - 0xA0),
            0xB0..=0xB8 => {
                let v = r - 0xB0;
                self.update(v);
                if val & 0x20 != 0 && old & 0x20 == 0 {
                    for o in self.ops[v].iter_mut() {
                        o.phase = 0.0;
                        o.att = 96.0;
                        o.stage = Stage::Attack;
                    }
                } else if val & 0x20 == 0 && old & 0x20 != 0 {
                    for o in self.ops[v].iter_mut() {
                        if o.stage != Stage::Off {
                            o.stage = Stage::Release;
                        }
                    }
                }
            }
            _ => {}
        }
    }

    pub fn silent(&self) -> bool {
        self.ops.iter().all(|v| v.iter().all(|o| o.stage == Stage::Off))
    }

    /// One mono output sample (about -1..1 per voice).
    pub fn sample(&mut self) -> f32 {
        self.t += 1.0 / RATE as f32;
        let bd = self.regs[0xBD];
        let cents = if bd & 0x40 != 0 { 14.0 } else { 7.0 };
        let vib = 2f32.powf(cents / 1200.0 * (self.t * 6.07 * std::f32::consts::TAU).sin());
        let depth = if bd & 0x80 != 0 { 4.8 } else { 1.0 };
        let trem = depth * 0.5 * (1.0 + (self.t * 3.7 * std::f32::consts::TAU).sin());
        let mut sum = 0.0f32;
        for v in 0..9 {
            let [m, c] = &self.ops[v];
            if m.stage == Stage::Off && c.stage == Stage::Off {
                continue;
            }
            let (f, _) = self.freq(v);
            let c0 = self.regs[0xC0 + v];
            let additive = c0 & 1 != 0;
            let fbn = (c0 >> 1) & 7;
            let fb = if fbn == 0 { 0.0 } else { 2f32.powi(fbn as i32 - 7) * 2.0 };
            let [m, c] = &mut self.ops[v];
            let fbm = if fb > 0.0 { (m.out[0] + m.out[1]) * 0.5 * fb } else { 0.0 };
            let ms = if m.vib { f * vib } else { f } / RATE as f32;
            let mo = m.sample(ms, fbm, if m.am { trem } else { 0.0 });
            let cs = if c.vib { f * vib } else { f } / RATE as f32;
            let co = c.sample(cs, if additive { 0.0 } else { mo * 4.0 }, if c.am { trem } else { 0.0 });
            sum += if additive { co + mo } else { co };
        }
        sum
    }
}

#[cfg(test)]
mod chip_tests {
    use super::*;

    /// The chip that the original plays on here is DOSBox's DBOPL (music.md): Operator::GetSample adds the modulation to
    /// the wave index, and ForwardWave moves that index forward. So a modulation of a quarter wave gives the top of the
    /// sine, and a minus sign gives the bottom. On the game's DAY theme the wrong sign moves the mix to a correlation of
    /// 0.49 with the right one.
    #[test]
    fn modulation_adds_to_the_phase() {
        let mut op = Op::off();
        (op.tl, op.att, op.stage, op.hold) = (0.0, 0.0, Stage::Sustain, true);
        assert!((op.sample(0.0, 0.25, 0.0) - 1.0).abs() < 1e-3);
        assert!((op.sample(0.0, 0.75, 0.0) + 1.0).abs() < 1e-3);
        assert!((op.sample(0.25, 0.0, 0.0) - 1.0).abs() < 1e-3, "the phase moves forward");
    }

    /// Ledger rows ja1front:9880/9882: the chip is silent while every operator is off (the music render skips it), and
    /// not silent from a key-on (B0 bit 0x20) until the release ends.
    #[test]
    fn opl_silent_until_key_on() {
        let mut opl = Opl::default();
        assert!(opl.silent());
        opl.write(0xB0, 0x20);
        assert!(!opl.silent());
    }
}
