//! The slot list of the office's Save Game and Restore Game (strategic.md "Save Game and Restore Game"): ten slots
//! GAME0.SAV .. GAME9.SAV, the list picture, the lit and plain rows, the name input with its key reader.

use std::collections::VecDeque;
use std::path::Path;

use crate::office::{put, Pic};
use crate::panel::Painter;

/// Mode 1 = Save Game, 0 = Restore Game.
pub const SAVE: i32 = 1;
pub const RESTORE: i32 = 0;
pub const NO_SAVES: &str = "No saved games exist!";
/// The message box while a chosen slot is written or read (strategic.md "After the list").
pub const SAVING: &str = "Saving...";
pub const RESTORING: &str = "Restoring game...";

/// The text of the box that shows while the chosen slot of `sl` is written or read; None when nothing is chosen.
pub fn busy_text(sl: &SlotList, out: Outcome) -> Option<&'static str> {
    match out {
        Outcome::Slot(_) => Some(if sl.mode == SAVE { SAVING } else { RESTORING }),
        _ => None,
    }
}
const B44: &[u8] = b"                                            ";
const B32: &[u8] = b"                                ";
/// The allowed keys of mode 1 (mode 0: only 27).
const ALLOWED: &[u8] = b"abcdefghijklmnopqrstuvwxyzABCDEFGHIJKLMNOPQRSTUVWXYZ 0123456789:;?,.!-&'/\x08\x0d\x1b";

/// The slot headers: per slot n whether GAMEn.SAV opened, its day (bytes 0..1) and its name (bytes 2..27).
#[derive(Clone, Default)]
pub struct Headers {
    pub used: [bool; 10],
    pub days: [u16; 10],
    pub names: [[u8; 26]; 10],
}

impl Headers {
    /// Read the headers of GAME0.SAV .. GAME9.SAV in `dir`.
    pub fn read(dir: &Path) -> Headers {
        let mut h = Headers::default();
        for n in 0..10 {
            h.days[n] = 0xFFFF;
            if let Ok(f) = std::fs::read(dir.join(slot_file(n))) {
                h.used[n] = true;
                let mut b = [0u8; 28];
                let m = f.len().min(28);
                b[..m].copy_from_slice(&f[..m]);
                h.days[n] = u16::from_le_bytes([b[0], b[1]]);
                h.names[n].copy_from_slice(&b[2..28]);
            }
        }
        h
    }
}

pub fn slot_file(n: usize) -> String {
    format!("GAME{}.SAV", n)
}

/// How the slot list ended.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Outcome {
    /// Escape or the right button: nothing chosen.
    Cancel,
    /// Restore Game with no slot in use: the message box "No saved games exist!".
    NoSaves,
    /// Slot n chosen (Save Game: after the name input ended with Enter).
    Slot(usize),
}

#[derive(Clone, Copy, PartialEq)]
enum St {
    /// Waiting for the left button that opened the list.
    Open,
    List,
    /// After a choice or a cancel with the left button held: (chosen row, cancel).
    PickWait(i32, bool),
    Name,
    /// The name input's right button wait.
    NameRightWait,
    Done(Outcome),
}

/// The slot list (one per opening).
pub struct SlotList {
    pub mode: i32,
    pub hdr: Headers,
    /// L: the numbers of the slots in use in rising order.
    list: Vec<usize>,
    st: St,
    /// The lit row H.
    lit: i32,
    keys: VecDeque<u8>,
    /// The mouse of the last poll.
    mx: i32,
    my: i32,
    /// The save name S (stays the game's save name, also after a cancel).
    pub name: [u8; 26],
    /// The `day` shown in the name input.
    pub day: i32,
    /// Name input: its row and cursor p.
    row: i32,
    cur: i32,
    /// Name input: the raw key read waits for a key byte (with the cancel of that pass).
    raw_wait: Option<bool>,
}

fn len(s: &[u8; 26]) -> i32 {
    s.iter().position(|&b| b == 0).unwrap_or(26) as i32
}

fn day_text(d: i32) -> Vec<u8> {
    format!("Day {:2}:", d).into_bytes()
}

fn row_y(r: i32) -> i32 {
    26 + 14 * r
}

impl SlotList {
    pub fn new(mode: i32, hdr: Headers, name: [u8; 26], day: i32) -> SlotList {
        let list = (0..10).filter(|&n| hdr.used[n]).collect();
        SlotList { mode, hdr, list, st: St::Open, lit: 0, keys: VecDeque::new(), mx: 0, my: 0, name, day, row: 0,
                   cur: 0, raw_wait: None }
    }

    pub fn outcome(&self) -> Option<Outcome> {
        match self.st {
            St::Done(o) => Some(o),
            _ => None,
        }
    }

    /// The slot of row r (1..10), None when the row does not exist.
    fn slot(&self, r: i32) -> Option<usize> {
        if !(1..=10).contains(&r) {
            return None;
        }
        if self.mode == SAVE {
            Some(r as usize - 1)
        } else {
            self.list.get(r as usize - 1).copied()
        }
    }

    /// The slot of row r is in use for the drawing: day ≠ 0xFFFF in mode 1, always in mode 0.
    fn in_use(&self, n: usize) -> bool {
        self.mode != SAVE || self.hdr.days[n] != 0xFFFF
    }

    fn name_text(&self, n: usize) -> &[u8] {
        let s = &self.hdr.names[n];
        &s[..len(s) as usize]
    }

    /// The first existing row whose hit rectangle holds the mouse; 0 = none.
    fn hit(&self) -> i32 {
        (1..=10)
            .find(|&r| {
                let y = row_y(r);
                self.slot(r).is_some() && 43 <= self.mx && self.mx <= 276 && y - 3 <= self.my && self.my <= y + 9
            })
            .unwrap_or(0)
    }

    /// The list picture.
    pub fn draw_list(&self, p: &mut Painter, slab: &Pic) {
        put(p.buf, slab, 43, 19);
        if self.mode == SAVE {
            p.text(b"Select Game Slot For Save", 93, 19, false, 0xFF, 8, 2);
        } else {
            p.text(b"Select Game to Restore", 99, 19, false, 0xFF, 8, 2);
        }
        for r in 1..=10 {
            let Some(n) = self.slot(r) else { continue };
            let y = row_y(r);
            put(p.buf, slab, 43, y - 3);
            if self.in_use(n) {
                p.text(&day_text(self.hdr.days[n] as i16 as i32), 47, y, false, 0x28, 1, 1);
                p.text(self.name_text(n), 103, y, false, 0x28, 1, 1);
            } else {
                p.text(b"Empty", 47, y, false, 0xCA, 0, 1);
            }
        }
    }

    /// The lit row r (`lit`) or the plain row r.
    fn draw_row(&self, p: &mut Painter, r: i32, lit: bool) {
        let Some(n) = self.slot(r) else { return };
        let y = row_y(r);
        p.text_mode(B44, 47, y, false, 0xFF, 8, 1, 0xF3);
        let (fg, sh) = if lit { (0xFE, 8) } else { (0x28, 1) };
        if self.in_use(n) {
            p.text(&day_text(self.hdr.days[n] as i16 as i32), 47, y, false, fg, sh, 1);
            p.text(self.name_text(n), 103, y, false, fg, sh, 1);
        } else if lit {
            p.text(b"Empty", 47, y, false, 0xFE, 8, 1);
        } else {
            p.text(b"Empty", 47, y, false, 0xCA, 0, 1);
        }
    }

    /// The text cursor at q.
    fn draw_cursor(&self, p: &mut Painter, q: i32) {
        let l = len(&self.name);
        let ch = if q < l { self.name[q as usize] } else { b' ' };
        let mut x = 103;
        for i in 0..q.min(l) {
            x += if matches!(self.name[i as usize], b' ' | b'-' | b'.' | b':') { 5 } else { 6 };
        }
        p.text_mode(&[ch], x, row_y(self.row), false, 0xFE, 8, 1, 0xCA);
    }

    /// Queue key bytes (a normal key: its ASCII byte; an extended key: 0 and its scan code).
    pub fn key(&mut self, b: u8) {
        self.keys.push_back(b);
    }

    /// The key reader K.
    fn read_key(&mut self) -> u8 {
        let c = self.keys.pop_front().unwrap_or(0);
        if c == 0 {
            let s = self.keys.pop_front().unwrap_or(0);
            return match s {
                0x4F => b']',
                0x47 => b'[',
                0x4B => b'<',
                0x4D => b'>',
                0x53 => 0x7F,
                _ => 0,
            };
        }
        let ok = if self.mode == SAVE { ALLOWED.contains(&c) } else { c == 27 };
        if ok { c } else { 0 }
    }

    /// One poll of the mouse (x, y, buttons: bit 0 left, bit 1 right held); queue key bytes with `key` first.
    /// Returns the outcome once the list ended.
    pub fn poll(&mut self, p: &mut Painter, slab: &Pic, x: i32, y: i32, buttons: i32) -> Option<Outcome> {
        if let Some(cancel) = self.raw_wait {
            // The name input waits in its raw key read: no poll happens until a key byte comes.
            let Some(k) = self.keys.pop_front() else { return None };
            self.raw_wait = None;
            self.raw_op(k);
            self.name_pass_end(p, cancel, false);
        }
        if matches!(self.st, St::Done(_)) {
            return self.outcome();
        }
        self.mx = x;
        self.my = y;
        match self.st {
            St::Open => {
                if buttons & 1 == 0 {
                    if self.mode == RESTORE && self.list.is_empty() {
                        self.st = St::Done(Outcome::NoSaves);
                    } else {
                        self.draw_list(p, slab);
                        self.lit = 0;
                        self.st = St::List;
                    }
                }
            }
            St::List => {
                let mut cancel = false;
                if !self.keys.is_empty() && self.read_key() == 27 {
                    cancel = true;
                }
                let h = self.hit();
                if h != self.lit {
                    self.draw_row(p, self.lit, false);
                    self.draw_row(p, h, true);
                    self.lit = h;
                }
                let mut chosen = 0;
                if buttons & 1 != 0 {
                    chosen = self.hit();
                } else if buttons & 2 != 0 {
                    cancel = true;
                }
                if chosen != 0 || cancel {
                    if buttons & 1 != 0 {
                        self.st = St::PickWait(chosen, cancel);
                    } else {
                        self.picked(p, chosen, cancel);
                    }
                }
            }
            St::PickWait(c, cancel) => {
                if buttons & 1 == 0 {
                    self.picked(p, c, cancel);
                }
            }
            St::Name => {
                if buttons & 2 != 0 {
                    self.st = St::NameRightWait;
                } else {
                    self.name_keys(p, false);
                }
            }
            St::NameRightWait => {
                if buttons & 2 == 0 {
                    self.st = St::Name;
                    self.name_keys(p, true);
                }
            }
            St::Done(_) => {}
        }
        self.outcome()
    }

    fn picked(&mut self, p: &mut Painter, c: i32, cancel: bool) {
        if cancel {
            self.st = St::Done(Outcome::Cancel);
        } else if self.mode == RESTORE {
            self.st = St::Done(Outcome::Slot(self.list[c as usize - 1]));
        } else {
            self.start_name(p, c);
        }
    }

    fn start_name(&mut self, p: &mut Painter, c: i32) {
        self.row = c;
        let n = c as usize - 1;
        self.name = [0; 26];
        let l = len(&self.hdr.names[n]) as usize;
        self.name[..l].copy_from_slice(&self.hdr.names[n][..l]);
        self.cur = len(&self.name) - 1;
        if self.cur > 23 {
            self.cur = 24;
        }
        let y = row_y(c);
        p.text_mode(B44, 47, y, false, 0xFF, 8, 1, 0xF3);
        p.text_mode(&day_text(self.day), 47, y, false, 0x28, 1, 1, 0xF3);
        let s = self.name;
        p.text_mode(&s[..len(&s) as usize], 103, y, false, 0xFE, 8, 1, 0xF3);
        self.draw_cursor(p, self.cur + 1);
        self.st = St::Name;
    }

    /// Step 2 of a name input pass (after the poll); `cancel` = the right button cancelled it in this pass.
    fn name_keys(&mut self, p: &mut Painter, mut cancel: bool) {
        if self.keys.is_empty() {
            if cancel {
                self.name_end(p, false);
            }
            return;
        }
        let r = self.read_key();
        let mut done = false;
        match r {
            0 => match self.keys.pop_front() {
                Some(k) => self.raw_op(k),
                None => {
                    self.raw_wait = Some(cancel);
                    return;
                }
            },
            8 => {
                if self.cur > -1 {
                    for i in self.cur as usize..25 {
                        self.name[i] = self.name[i + 1];
                    }
                    self.cur -= 1;
                }
            }
            13 => done = true,
            27 => cancel = true,
            _ => {
                if len(&self.name) < 24 {
                    let mut i = 25;
                    if self.cur + 1 < 25 {
                        loop {
                            self.name[i as usize] = self.name[i as usize - 1];
                            i -= 1;
                            if self.cur + 1 >= i {
                                break;
                            }
                        }
                    }
                    self.name[(self.cur + 1) as usize] = r;
                    self.cur += 1;
                }
            }
        }
        self.name_pass_end(p, cancel, done);
    }

    /// The raw key k after a key reader result 0.
    fn raw_op(&mut self, k: u8) {
        let l = len(&self.name);
        match k {
            0x47 => self.cur = -1,
            0x4B => {
                if self.cur > -1 {
                    self.cur -= 1;
                }
            }
            0x4D => {
                if self.cur < 24 && self.cur + 1 < l {
                    self.cur += 1;
                }
            }
            0x4F => self.cur = (l - 1).min(24),
            0x53 => {
                for i in (self.cur + 1).max(0) as usize..25 {
                    self.name[i] = self.name[i + 1];
                }
            }
            _ => {}
        }
    }

    /// The redraw after a key (when not cancelled), then the end of the input after Enter or a cancel.
    fn name_pass_end(&mut self, p: &mut Painter, cancel: bool, done: bool) {
        if !cancel {
            let y = row_y(self.row);
            p.text_mode(B32, 103, y, false, 0xFF, 8, 1, 0xF3);
            let s = self.name;
            p.text_mode(&s[..len(&s) as usize], 103, y, false, 0xFE, 8, 1, 0xF3);
            self.draw_cursor(p, self.cur + 1);
        }
        if done || cancel {
            self.name_end(p, done && !cancel);
        }
    }

    /// The name input ended (Enter = `done`, else cancelled): the row under the mouse is lit again.
    fn name_end(&mut self, p: &mut Painter, done: bool) {
        let h = self.hit();
        if h != self.lit {
            self.draw_row(p, self.lit, false);
            self.draw_row(p, h, true);
        }
        if done {
            self.st = St::Done(Outcome::Slot(self.row as usize - 1));
        } else {
            self.lit = 0;
            self.st = St::List;
        }
    }
}

/// RestoreGameDialog? 0x4E700, the steps after the slot list: no slot in use -> the message box (text, no load); a
/// slot chosen -> LoadGame between HidePointer and ShowMouse (true); a cancel -> nothing.
pub fn restore_steps(out: Outcome) -> (Option<&'static str>, bool) {
    match out {
        Outcome::NoSaves => (Some(NO_SAVES), false),
        Outcome::Slot(_) => (None, true),
        Outcome::Cancel => (None, false),
    }
}

/// After the list (strategic.md "After the list"): Save Game keeps the typed name as the game's save name (also
/// after a cancel) and writes `GAMEn.SAV` in `dir`; original SaveGame uses OpenMiscFile 0x38370, while the port
/// writes directly to the save directory. Restore Game loads `GAMEn.SAV` as a day save, keeping the
/// save name and save bytes 21541 and 21542 of the world before. Returns the script's stdout line.
pub fn finish(w: &mut crate::world::World, sl: &SlotList, out: Outcome, dir: Option<&Path>, iso: &str, exe: &str) -> String {
    if sl.mode == SAVE {
        if w.base.len() >= 28 {
            w.base[2..28].copy_from_slice(&sl.name);
        }
    }
    let Outcome::Slot(n) = out else {
        return if restore_steps(out).0.is_some() { "slots none".into() } else { "slots cancel".into() };
    };
    let Some(dir) = dir else {
        // Script mode without JA1_SAVE_DIR: no file is written or read (world_load.md section 10).
        return format!("slots {} {} (no save directory)", if sl.mode == SAVE { "saved" } else { "restored" }, n);
    };
    let path = dir.join(slot_file(n));
    if sl.mode == SAVE {
        if let Err(e) = std::fs::write(&path, w.save_bytes()) {
            eprintln!("save: {}: {}", path.display(), e);
        }
        return format!("slots saved {}", n);
    }
    match crate::world::World::load(iso, exe, &path.to_string_lossy()) {
        Ok(mut nw) => {
            if nw.base.len() >= 21543 && w.base.len() >= 21543 {
                nw.base[2..28].copy_from_slice(&w.base[2..28]);
                crate::ctlpanel::put_sound_switches(&mut nw, crate::ctlpanel::sound_switches(w));
            }
            // strategic.md "The day replay": no load writes replay_carry (the original's byte is not in a save).
            nw.replay_carry = w.replay_carry;
            *w = nw;
            format!("slots restored {} day {}", n, w.day)
        }
        Err(e) => {
            eprintln!("restore: {}: {}", path.display(), e);
            format!("slots error {}", n)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Restore Game keeps the save name (bytes 2..28) and the sound switches of the world before; a world before
    /// without these bytes (a short base) keeps nothing and does not fail.
    #[test]
    fn restore_keeps_the_name_only_from_a_full_world() {
        let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
        let (iso, exe) = (root.join("rom/JA1.gog"), root.join("build/game113b/JA.EXE"));
        let day = root.join("build/DAY_day3.SAV");
        if !iso.exists() || !exe.exists() || !day.exists() {
            return;
        }
        let (iso, exe) = (iso.to_str().unwrap(), exe.to_str().unwrap());
        let dir = std::env::temp_dir().join(format!("ja1_slots_test_{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        std::fs::copy(&day, dir.join(slot_file(3))).unwrap();
        let sl = SlotList::new(RESTORE, Headers::default(), [0; 26], 1);
        let mut w = crate::world::World::new_game(iso, exe, 0, 0).unwrap();
        w.base[2..28].copy_from_slice(b"KEPT NAME\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0");
        let line = finish(&mut w, &sl, Outcome::Slot(3), Some(&dir), iso, exe);
        assert!(line.starts_with("slots restored 3"), "{line}");
        assert_eq!(&w.base[2..11], b"KEPT NAME");
        let mut w = crate::world::World::new_game(iso, exe, 0, 0).unwrap();
        w.base[2..28].copy_from_slice(b"KEPT NAME\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0");
        w.base.truncate(100);
        let line = finish(&mut w, &sl, Outcome::Slot(3), Some(&dir), iso, exe);
        assert!(line.starts_with("slots restored 3"), "{line}");
        let saved = crate::world::World::load(iso, exe, &day.to_string_lossy()).unwrap();
        assert_eq!(&w.base[2..28], &saved.base[2..28]);
        assert_ne!(&saved.base[2..11], b"KEPT NAME");
        std::fs::remove_dir_all(&dir).ok();
    }

    /// strategic.md "The name input" (SaveSlotList? raw key 0x4D, Right): p = p + 1 when p < 24 and p + 1 < the
    /// length of S; Left stops at -1.
    #[test]
    fn the_right_arrow_stops_at_the_last_letter() {
        let mut name = [0u8; 26];
        name[..3].copy_from_slice(b"ABC");
        let mut l = SlotList::new(1, Headers::default(), name, 1);
        l.cur = -1;
        let mut seen = Vec::new();
        for _ in 0..4 {
            l.raw_op(0x4D);
            seen.push(l.cur);
        }
        assert_eq!(seen, [0, 1, 2, 2]);
        // a full name: the cursor stops at place 24
        l.name = [b'A'; 26];
        l.cur = 23;
        l.raw_op(0x4D);
        l.raw_op(0x4D);
        assert_eq!(l.cur, 24);
        for _ in 0..30 {
            l.raw_op(0x4B);
        }
        assert_eq!(l.cur, -1);
    }
}
