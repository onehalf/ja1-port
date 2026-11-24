//! The day replay (docs/spec/strategic.md "The day replay"; C interface in docs/spec/port_api.md): `ja1_day_replay`.
//! A child module of `ai`; the day's events come from host ops 353 replay_count / 354 replay_record, the pictures
//! and sounds go to host ops 355 replay_anim / 356 replay_draw, random through `Ai`. Set owned is that of `battle`,
//! Workable trees that of `ledger`, both run on `sector_replay` (the pointer stands in for `sector_state`).
use super::battle::set_owned;
use super::damage::{get8, put8};
use super::ledger::workable;
use super::{op, Ai, Ja1Host, Ja1World};

const REPLAY_LEN: usize = 60 * 9;

struct Replay {
    ai: Ai,
    show: bool,
}

impl Replay {
    fn r(&self, s: u8, o: i64) -> u8 {
        get8(unsafe { (*self.ai.w).sector_replay }, REPLAY_LEN, s as i64 * 9 + o)
    }
    fn put(&self, s: u8, o: i64, v: u8) {
        put8(unsafe { (*self.ai.w).sector_replay }, REPLAY_LEN, s as i64 * 9 + o, v)
    }
    fn dec(&self, s: u8, o: i64) {
        let v = self.r(s, o);
        if v != 0 {
            self.put(s, o, v - 1);
        }
    }
    fn owned(&self, s: u8) -> bool {
        self.r(s, 0) & 1 != 0
    }
    fn carry(&self) -> u8 {
        unsafe { (*self.ai.w).replay_carry }
    }
    fn set_carry(&self, v: u8) {
        unsafe { (*self.ai.w).replay_carry = v };
    }
    /// Host op 355 replay_anim, only while show = 1; an answer with bit 0 set ends the show.
    fn anim(&mut self, args: &[i32]) {
        if self.show && self.ai.call(op::REPLAY_ANIM, args) & 1 != 0 {
            self.show = false;
        }
    }
    fn draw(&self, k: i32, a: i32) {
        if self.show {
            self.ai.call(op::REPLAY_DRAW, &[k, a]);
        }
    }
    fn workable_all(&self) {
        for s in 0..60u8 {
            workable(self.ai.w, s);
        }
    }
    /// The battle show in sector f.
    fn battle(&mut self, f: u8) {
        let (mut z, mut m, mut x, mut y) = ([0i32; 3], [99i32; 3], [0i32; 3], [0i32; 3]);
        for j in 0..100 {
            if !self.show {
                return;
            }
            let r = 99 - j;
            for k in 0..3 {
                let l = if z[k] == 0 { 3 } else { 8 };
                if m[k] < l {
                    m[k] += 1;
                } else {
                    x[k] = self.ai.random(25) + 3;
                    y[k] = self.ai.random(18) + 3;
                    z[k] = (self.ai.random(10) > 4) as i32;
                    let l = if z[k] == 0 { 3 } else { 8 };
                    if r <= l {
                        z[k] = 2;
                        m[k] = 99;
                    } else {
                        m[k] = 0;
                    }
                }
            }
            let p = |k: usize| z[k] | m[k] << 8 | x[k] << 16 | y[k] << 24;
            self.anim(&[2, f as i32, j, p(0), p(1), p(2)]);
        }
    }

    fn event(&mut self, f: u8, t: u8, c: u8) {
        let s0 = self.show;
        let (fi, ti) = (f as i32, t as i8 as i32);
        if f >= 60 || (matches!(c, b'J' | b'K' | b'L') && t >= 60) {
            return; // SPEC GAP: the original reads and writes past its table
        }
        match c {
            b'J' => {
                if s0 {
                    self.anim(&[1, fi, ti, 1]);
                }
                if !self.owned(f) {
                    self.anim(&[0, fi, 2, 0]);
                }
                if !self.owned(t) {
                    self.anim(&[0, ti, 2, 1]);
                }
                self.put(t, 2, self.r(f, 2));
                self.put(f, 2, 0);
            }
            b'K' | b'L' => {
                if c == b'L' && s0 {
                    self.anim(&[0, fi, 2, 1]);
                    self.anim(&[1, fi, ti, 2]);
                }
                let k = self.carry();
                self.put(t, 1, self.r(t, 1).wrapping_add(k));
                self.put(f, 1, self.r(f, 1).wrapping_sub(k));
                self.set_carry(0);
            }
            b'M' => {
                if s0 {
                    self.battle(f);
                }
            }
            b'N' => {
                set_owned(self.ai.w, f, false);
                self.put(f, 4, 0);
                self.put(f, 3, 0);
                self.put(f, 2, 0);
                self.workable_all();
                self.draw(1, 1);
                self.draw(2, fi);
                self.anim(&[0, fi, 2, 0]);
            }
            b'O' | b'P' => {
                if s0 {
                    self.anim(&[3, fi, 2, 0]);
                }
                set_owned(self.ai.w, f, true);
                self.put(f, 1, 0);
                self.workable_all();
                self.draw(1, 1);
                self.draw(2, fi);
            }
            b'Q' => {
                if s0 {
                    self.anim(&[4, fi, 0, 0]);
                }
            }
            b'R' | b'S' | b'Z' | b'U' | b'V' => {
                match c {
                    b'R' => self.dec(f, 4),
                    b'S' => self.dec(f, 3),
                    b'Z' => self.dec(f, 2),
                    _ => {
                        self.dec(f, 6);
                        if c == b'V' {
                            self.dec(f, 8);
                        }
                        workable(self.ai.w, f);
                    }
                }
                self.draw(3, fi);
            }
            b'T' => self.dec(f, 1),
            b'W' => {
                self.put(f, 1, self.r(f, 1).wrapping_add(self.carry()));
                self.set_carry(0);
            }
            b'X' => {
                if s0 {
                    self.anim(&[0, fi, 1, 1]);
                }
            }
            b'Y' => {
                if s0 {
                    self.anim(&[0, fi, 1, 0]);
                }
                self.put(f, 2, 0);
                if !self.owned(f) {
                    self.anim(&[0, fi, 2, 0]);
                }
            }
            b'[' => {
                let v = self.r(f, 4);
                self.put(f, 4, if (v as i32) - ti < 0 { 0 } else { v.wrapping_sub(t) });
                self.draw(3, fi);
            }
            b'\\' => {
                let v = self.r(f, 5).wrapping_add(t);
                self.put(f, 5, v);
                let b4 = self.r(f, 4);
                if b4 as u32 + v as u32 > 8 {
                    self.put(f, 5, 8u8.wrapping_sub(b4));
                }
                self.draw(3, fi);
            }
            b']' => {
                let v = self.r(f, 4).wrapping_add(t);
                self.put(f, 4, v.min(8));
                let b5 = self.r(f, 5);
                self.put(f, 5, if (b5 as i32) - ti < 0 { 0 } else { b5.wrapping_sub(t) });
                self.draw(3, fi);
            }
            0..=0x3F => {
                if s0 {
                    self.anim(&[5, c as i32, fi, ti]);
                }
                let v = self.r(f, 2);
                if v != 0 {
                    self.put(f, 2, v - 1);
                    if v == 1 {
                        self.anim(&[3, fi, 1, 0]);
                    }
                }
                self.draw(1, 1);
            }
            101..=0x7F => self.set_carry(c - 100),
            _ => {}
        }
    }
}

fn day_replay(w: *mut Ja1World, h: *const Ja1Host) {
    let ai = Ai { w, h, s: 0 };
    let n = ai.call(op::REPLAY_COUNT, &[]) as i16;
    if n == 0 {
        return;
    }
    let real = unsafe { (*w).sector_state };
    unsafe { (*w).sector_state = (*w).sector_replay };
    let show = unsafe { (*w).opt_replays } != 0;
    let mut rp = Replay { ai, show };
    rp.ai.call(op::REPLAY_DRAW, &[0, 0]);
    for i in 0..n as i32 {
        let e = rp.ai.call(op::REPLAY_RECORD, &[i]);
        if e >> 24 & 1 != 0 {
            rp.show = false;
        }
        rp.event(e as u8, (e >> 8) as u8, (e >> 16) as u8);
    }
    if !rp.show {
        rp.workable_all();
        rp.ai.call(op::REPLAY_DRAW, &[0, 0]);
    }
    unsafe { (*w).sector_state = real };
}

#[no_mangle]
pub extern "C" fn ja1_day_replay(w: *mut Ja1World, h: *const Ja1Host) {
    if w.is_null() || h.is_null() || unsafe { (*w).sector_replay.is_null() } {
        return;
    }
    let real = unsafe { (*w).sector_state };
    if std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| day_replay(w, h))).is_err() {
        unsafe { (*w).sector_state = real };
    }
}
