//! Noise (docs/spec/ai.md "Noise"; C interface in docs/spec/port_api.md): Make noise (library op 127), Hearing a
//! noise (library op 211) and Noise interrupt (library op 212). A child module of `ai`: every op goes through
//! `Ai::call` (the host_ops rule), also the library-to-library calls 211, 212, 182, 6, 7, 113 and 114. World data
//! is read again after every host call.
use super::damage::{get8, put16, put8, KNOWLEDGE_LEN, LAST_SEEN_E_LEN, LAST_SEEN_M_LEN};
use super::search::{px, py, s16, world_dist};
use super::{op, run, Ai, Ja1Host, Ja1World};

const ITQ_LEN: usize = 50;
const NO_SOLDIER: u8 = 0x40;

/// Make noise (library op 127): source src, tile, class cls, loudness loud, kind.
pub(super) fn make_noise(w: *mut Ja1World, h: *const Ja1Host, src: u8, tile: i32, cls: u8, loud: i32, kind: u8) {
    // Any view gives the world access and the dispatch point; slot 0 is only the caller of the calls.
    let Some(a) = Ai::new(w, h, 0) else { return };
    let tile = s16(tile);
    // "src record": soldiers[src] when src < 0x30.
    let sr = if src < 0x30 { Ai::new(w, h, src as i32) } else { None };
    // 1.
    if !a.w_combat() || loud == 0 {
        return;
    }
    // SPEC GAP: src 0x30..0x3F has no record; the port treats it as alive and with class 0.
    if let Some(r) = &sr {
        if r.rd8(0x183) == 0 {
            return;
        }
    }
    // 2.
    let listeners = if src < 0x28 || src == 0x42 {
        0x28..0x30
    } else if src < 0x30 || src == 0x41 {
        0..0x10
    } else {
        0..0x30
    };
    // 3.
    let mut c = 0u8;
    if let Some(r) = &sr {
        c = cls;
        if !(1..=9).contains(&c) {
            c = r.rd8(0x1B3);
        }
    }
    if sr.is_none() || !(1..=9).contains(&c) {
        c = a.t0_class(a.tb(tile, 0));
    }
    // 4.
    for k in listeners {
        let Some(l) = Ai::new(w, h, k) else { return };
        // a.
        if l.rd8(0x182) == 0 || l.rd8(0x138) == 0 || l.rd8(0x188) != 0 || l.s8(0x183) < 15 || l.s8(0xBA) == 2 {
            continue;
        }
        // b.
        if src < 0x40 {
            let flag = if src < 0x28 { 0x13A } else { 0x112 } + src as usize;
            if l.knowledge(src as usize) == 1 || l.s8(flag) == 1 {
                continue;
            }
        }
        // c.
        let mut v = loud.wrapping_add(l.s8(0x1D0)) as i16;
        if (l.rd8(0x1B3) == 3) != (c == 3) {
            v = v.wrapping_sub(5);
        }
        let d = s16(world_dist(s16(l.rd16(0x116) as i32), s16(l.rd16(0x118) as i32), px(tile), py(tile)));
        let v = (v as i32).wrapping_sub(d / 15 - 1).wrapping_sub(l.s8(0x1D5)) as i16;
        if v <= 0 {
            continue;
        }
        // d.
        let at = match &sr {
            Some(r) if kind == 4 => s16(r.rd16(0x114) as i32),
            _ => tile,
        };
        a.call(op::HEARS, &[k, at, v as u8 as i32, kind as i32]);
    }
}

impl Ai {
    /// Hearing a noise (library op 211): this soldier is the listener.
    pub(super) fn hears(&self, tile: i32, strength: u8, kind: u8) {
        let slot = self.slot();
        let tile = s16(tile);
        // 1.
        let occ = if matches!(kind, 6 | 8 | 9) { NO_SOLDIER } else { self.occupant(tile as u16) };
        let mut tell = true;
        let mut old = 0u8;
        let o = occ as usize;
        if occ < 0x30 {
            if (slot < 0x28) == (occ < 0x28) {
                return;
            }
            if slot < 0x28 {
                let kn = get8(unsafe { (*self.w).knowledge }, KNOWLEDGE_LEN, o as i64) as i8;
                if matches!(kn, 1 | 2 | -1) {
                    tell = false;
                }
                old = self.rd8(0x112 + o);
            } else {
                old = self.rd8(0x13A + o);
            }
        }
        // 2.
        let st = strength as i8 as i32;
        let level = if st < 3 {
            0
        } else if st < 6 {
            1
        } else if st < 10 {
            2
        } else {
            3
        };
        let me = s16(self.rd16(0x114) as i32);
        // 3.
        if slot < 8 && tell && !matches!(kind, 4 | 5 | 6 | 8 | 9) {
            let d = self.call(op::DIRECTION, &[px(me), py(me), px(tile), py(tile)]) as u8;
            self.call(op::HEARS_MSG, &[slot, level, kind as i32, d as i32]);
        }
        // 4.
        let mut seen = false;
        let mut turned = false;
        if self.call(op::TILE_DIST, &[me, tile]) as i32 <= 14 {
            let (x, y) = if occ < 0x30 {
                (s16(s16(self.rd16_of(o, 0x116) as i32) + 2), s16(s16(self.rd16_of(o, 0x118) as i32) + 4))
            } else {
                (px(tile), py(tile))
            };
            let sx = s16(self.rd16(0x116) as i32) + 2;
            let sy = s16(self.rd16(0x118) as i32) + 4;
            // SPEC: "direction (sx, sy, x, y)" is taken as library op 6 (through dispatch, as in step 3).
            turned = self.s8(0x1A1) != self.call(op::DIRECTION, &[sx, sy, x, y]) as i32;
            seen = self.call(op::SIGHT_LINE, &[slot, sx, sy, tile, x, y, 1, 0x0E, occ as i32]) != 0;
        }
        if occ < 0x30 {
            // 5.
            if seen {
                self.mark_seen(o);
                if slot < 0x28 {
                    self.call(op::SHARE_SIGHTINGS, &[slot, 0]);
                }
            } else {
                self.wr8(0x1C1, 1);
                if matches!(kind, 4 | 5) && level != 0 && self.s8(0xBB) < 2 {
                    self.wr8(0xBB, 2);
                    self.orders_bump();
                }
                let ot = self.rd16_of(o, 0x114);
                if slot >= 0x28 {
                    if self.s8(0x13A + o) != 2 {
                        self.wr8(0x13A + o, 0xFF);
                    }
                    let idx = (slot as i64 - 0x28) * 0x50 + 2 * o as i64;
                    put16(unsafe { (*self.w).last_seen_e }, LAST_SEEN_E_LEN, idx, ot);
                } else {
                    if self.s8(0x112 + o) != 2 {
                        self.wr8(0x112 + o, 0xFF);
                    }
                    let idx = slot as i64 * 16 + 2 * (o as i64 - 0x28);
                    put16(unsafe { (*self.w).last_seen_m }, LAST_SEEN_M_LEN, idx, ot);
                    if tell && slot < 8 {
                        put8(unsafe { (*self.w).knowledge }, KNOWLEDGE_LEN, o as i64, 0xFF);
                    }
                }
            }
        } else {
            // 6.
            if kind == 6 && level != 0 && self.s8(0xBB) < 2 {
                self.wr8(0xBB, 2);
                self.orders_bump();
            }
            if !seen || kind == 8 && turned {
                // SPEC: "S +0x1C1 = 1" is read as part of the "when st ≥ S +0x1C0" branch (the summary: a noise at
                // least as strong as the remembered one becomes the noise to check and the soldier re-thinks).
                if st >= self.rd8(0x1C0) as i32 {
                    self.wr16(0x122, tile as u16);
                    self.wr8(0x1C0, if st < 12 { strength } else { 12 });
                    self.wr8(0x1C1, 1);
                }
            } else {
                self.wr8(0x1C1, 1);
                if self.s8(0xBB) < 2 {
                    self.wr8(0xBB, 2);
                    self.orders_bump();
                }
            }
        }
        // 7.
        if kind == 8 {
            self.call(op::NOISE_INT, &[slot]);
        } else if occ < 0x30 {
            self.call(op::INTERRUPT_CHECK, &[slot, o as i32, old as i32]);
        }
    }

    /// Noise interrupt (library op 212): this soldier heard a kind-8 noise.
    pub(super) fn noise_interrupt(&self) {
        let slot = self.slot();
        // 1.
        if self.s8(0xBA) == 2 || self.rd8(0x18A) == 0 || self.cur_anim() < 4 || self.s8(0x1A5) < 1 {
            return;
        }
        // 2.
        if slot < 0x28 && unsafe { (*self.w).screen_drawn } != 0 {
            return;
        }
        // 3.
        self.wr8(0x1B0, 0);
        let sel = unsafe { (*self.w).selected };
        crate::trace::grant(slot as u8, sel);
        if sel >= 8 {
            self.itq_push(sel);
        } else {
            unsafe { (*self.w).kept_merc = sel };
        }
        // 4.
        if slot < 8 {
            unsafe { (*self.w).interrupter = slot as u8 };
            self.call(op::MERC_INTERRUPT, &[]);
            return;
        }
        // 5.
        self.itq_push(slot as u8);
        self.call(op::NEXT_TURN, &[1]);
    }
    /// itq[itq_len] = v, itq_len + 1 (8-bit).
    /// itq_len ≥ 50: a stop point (port_api.md "Division by zero"; the original writes past the queue).
    fn itq_push(&self, v: u8) {
        unsafe {
            let n = (*self.w).itq_len;
            if n as usize >= 50 {
                self.call(op::DIV_ZERO, &[]);
                return;
            }
            put8((*self.w).itq, ITQ_LEN, n as i64, v);
            (*self.w).itq_len = n.wrapping_add(1);
        }
    }
}

// ---- entry points ----
// SPEC GAP: a null world or host (or a null soldier buffer) does nothing.
#[no_mangle]
pub extern "C" fn ja1_make_noise(w: *mut Ja1World, h: *const Ja1Host, src: i32, tile: i32, cls: i32, loud: i32, kind: i32) {
    let _ = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        make_noise(w, h, src as u8, tile, cls as u8, loud, kind as u8)
    }));
}
// SPEC GAP: a slot outside 0..47 does nothing.
#[no_mangle]
pub extern "C" fn ja1_hears(w: *mut Ja1World, h: *const Ja1Host, slot: i32, tile: i32, strength: i32, kind: i32) {
    run(w, h, slot, (), |a| a.hears(tile, strength as u8, kind as u8))
}
#[no_mangle]
pub extern "C" fn ja1_noise_interrupt(w: *mut Ja1World, h: *const Ja1Host, slot: i32) {
    run(w, h, slot, (), |a| a.noise_interrupt())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::cell::RefCell;
    use std::os::raw::c_void;

    thread_local! { static LOG: RefCell<Vec<(i32, Vec<i32>)>> = RefCell::new(vec![]); }
    unsafe extern "C" fn fcall(_c: *mut c_void, op: i32, a: *const i32, n: i32, _o: *mut u8, _s: i32) -> u32 {
        let v = if n > 0 { std::slice::from_raw_parts(a, n as usize).to_vec() } else { vec![] };
        LOG.with(|l| l.borrow_mut().push((op, v)));
        0
    }
    const REC: usize = 0x200;

    #[test]
    fn noise_reaches_enemy_listener() {
        let mut soldiers = vec![0u8; 48 * REC];
        let mut tiles = vec![0u8; 3136 * 8];
        for t in 0..3136 {
            tiles[t * 8 + 5] = 0x40;
        }
        let mut know = vec![0u8; 64];
        let class = vec![0u8; 256];
        // Source merc 2 alive; listener enemy 0x28 in play, 20 hp, 3 tiles east of tile 1000.
        soldiers[2 * REC + 0x183] = 50;
        let k = 0x28 * REC;
        soldiers[k + 0x182] = 1;
        soldiers[k + 0x138] = 1;
        soldiers[k + 0x183] = 20;
        let t = 1003i32;
        let (x, y) = (px(t) as u16, py(t) as u16);
        soldiers[k + 0x116..k + 0x118].copy_from_slice(&x.to_le_bytes());
        soldiers[k + 0x118..k + 0x11A].copy_from_slice(&y.to_le_bytes());
        let mut w: Ja1World = unsafe { std::mem::zeroed() };
        w.soldiers = soldiers.as_mut_ptr();
        w.tiles = tiles.as_mut_ptr();
        w.knowledge = know.as_mut_ptr();
        w.t0_class = class.as_ptr();
        w.combat = 1;
        let h = Ja1Host { ctx: std::ptr::null_mut(), random: None, call: Some(fcall), host_ops: 1 << 63 };
        LOG.with(|l| l.borrow_mut().clear());
        ja1_make_noise(&mut w, &h, 2, 1000, 1, 10, 1);
        // dx = 48 px -> d = 48 * 15 / 16 = 45 -> v = 10 - (45 / 15 - 1) = 8.
        let log = LOG.with(|l| l.borrow().clone());
        assert_eq!(log, vec![(op::HEARS, vec![0x28, 1000, 8, 1])]);
        // A known source is skipped.
        know[2] = 1;
        LOG.with(|l| l.borrow_mut().clear());
        ja1_make_noise(&mut w, &h, 2, 1000, 1, 10, 1);
        assert!(LOG.with(|l| l.borrow().is_empty()));
    }

    #[test]
    fn noise_interrupt_queues_enemy() {
        let mut soldiers = vec![0u8; 48 * REC];
        let mut itq = vec![0u8; 50];
        let k = 0x29 * REC;
        soldiers[k + 0x18A] = 1;
        soldiers[k + 0x1E8] = 4;
        soldiers[k + 0x1A5] = 3;
        soldiers[k + 0x1B0] = 1;
        let mut w: Ja1World = unsafe { std::mem::zeroed() };
        w.soldiers = soldiers.as_mut_ptr();
        w.itq = itq.as_mut_ptr();
        w.selected = 3;
        let h = Ja1Host { ctx: std::ptr::null_mut(), random: None, call: Some(fcall), host_ops: 1 << 63 };
        LOG.with(|l| l.borrow_mut().clear());
        ja1_noise_interrupt(&mut w, &h, 0x29);
        assert_eq!(LOG.with(|l| l.borrow().clone()), vec![(op::NEXT_TURN, vec![1])]);
        assert_eq!((w.kept_merc, w.itq_len, itq[0], soldiers[k + 0x1B0]), (3, 1, 0x29, 0));
    }
}
