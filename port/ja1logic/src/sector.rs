//! Sector start (docs/spec/sector_start.md). Batch 1: slot reset, item reset, equipment roll (only the host's
//! random(n)). Batch 2: placing soldiers (XRA enemy, tapper, guard, guard tile, place merc, merc entry), with host
//! ops 51 replace and 58 reserve_ap. Batch 3: putting an item on the ground (no host call). Batch 4: taking items
//! off the ground (host op 85 cursor_back). Batch 5: soldier start (no host call). Batch 6: map patches, supply
//! drops, plants (host ops 86 open_patch, 87 read, 88 close, 89 replay_event). Batch 7: fixed items, the sector tmp
//! file, ground blast (host ops 90 explode, 91 effect, 92 open_tmp). Batch 8: the whole sector start (host ops 33
//! radio_result, 93 sector_resets, 95 sector_tail, 96 seek). Batch 9: ops 51 replace and 58
//! reserve_ap become library ops (Replace soldier, Reserve AP; host op 17 action_ap). Batch 10: op 33 radio_result
//! becomes a library op (Radio result; host op 44 quest_check). C interface in docs/spec/port_api.md "Sector
//! start entry points". `ja1_enter_sector` (docs/spec/world_load.md section 4) wraps batch 8 with the merc
//! selection, op 89 replay_event and the view position.
//! World memory belongs to the caller and the random callback may change any of it, so every field is read
//! through a raw pointer at the point of use.
use crate::ai::{op, shared, Ja1Host, Ja1World};
use std::panic::{catch_unwind, AssertUnwindSafe};

const REC: usize = 0x200;
const SLOTS: usize = 48;
const NONE: u16 = 0xC41;
const ITEM_TYPES: usize = 136;
const WEAPONS: usize = 256;
const ATTACH: usize = 82;

const GUNS: [u8; 8] = [1, 2, 3, 4, 8, 9, 10, 11];
const THROWN: [u8; 7] = [0x1C, 0x1C, 0x1C, 0x16, 0x16, 0x18, 0x1A];
const COUNTS: [u8; 6] = [1, 1, 1, 2, 2, 3];
const TILES: i32 = 3136;
const MERCS: usize = 8;
const GROUND: usize = 200;
const NOTES: usize = 15;
const BLAST: usize = 30;
const LISTED: usize = 32;
/// Recursion depth limit of Take ground patch (more than the 3136 map tiles, so a patch that ends is never cut).
const PATCH_DEPTH: u32 = 4096;
const TEXT_BUF: usize = 32;
const SECTORS: usize = 80;
const FIXED: usize = 11;
/// Rows of the batch-7 `sectors` table and the bytes per row.
const SECTOR_ROWS: usize = 60;
const SECTOR_ROW: usize = 0xB1;
/// Bytes per sector in DAT\XRA.DAT (16 soldier templates, 3136 tile bytes, 50 ground items).
const XRA_RECORD: i32 = 0x2E98;
const XRA_ITEMS: usize = 50;
const GUARD_DIRS: [u8; 8] = [2, 6, 4, 8, 1, 5, 3, 7];

struct Ctx {
    w: *mut Ja1World,
    h: *const Ja1Host,
}

impl Ctx {
    fn new(w: *mut Ja1World, h: *const Ja1Host) -> Option<Ctx> {
        // SPEC GAP: a null world or host does nothing.
        if w.is_null() || h.is_null() {
            return None;
        }
        Some(Ctx { w, h })
    }

    fn random(&self, n: i32) -> i32 {
        unsafe {
            let h = &*self.h;
            match h.random {
                Some(r) => r(h.ctx, n),
                None => 0,
            }
        }
    }

    // ---- soldier records ----
    fn rec(&self, slot: usize) -> *mut u8 {
        // Callers check slot < SLOTS and soldiers != null.
        unsafe { (*self.w).soldiers.add(slot * REC) }
    }
    fn soldiers_ok(&self) -> bool {
        unsafe { !(*self.w).soldiers.is_null() }
    }

    // ---- read-only tables (null or out of range: 0) ----
    fn table(p: *const u8, len: usize, i: usize) -> u8 {
        // SPEC GAP: an index outside the table (e.g. an item type >= 136) or a null table reads 0.
        if p.is_null() || i >= len {
            0
        } else {
            unsafe { *p.add(i) }
        }
    }
    fn item_type(&self, t: u8, off: usize) -> u8 {
        Self::table(unsafe { (*self.w).item_types }, ITEM_TYPES * 12, 12 * t as usize + off)
    }
    fn weapon_ammo(&self, t: u8) -> u8 {
        Self::table(unsafe { (*self.w).weapons }, WEAPONS * 6, 6 * t as usize + 1)
    }
    fn supply(&self, i: usize) -> i32 {
        Self::table(unsafe { (*self.w).supplies } as *const u8, 4, i) as i32
    }
    fn attach(&self, i: usize) -> u8 {
        Self::table(unsafe { (*self.w).attach }, ATTACH * 4, i)
    }

    /// "Stack count" (part of the equipment roll).
    fn stack_count(&self, t: u8) -> u8 {
        if self.item_type(t, 6) != 5 {
            return 1;
        }
        let r = self.random(100);
        if r < 1 {
            5
        } else if r < 3 {
            4
        } else if r < 10 {
            3
        } else if r < 25 {
            2
        } else {
            1
        }
    }

    /// "Item reset" on the 12-byte entry at `item`, DL = dl, BL = bl.
    fn item_reset(&self, item: *mut u8, dl: u8, bl: u8) {
        unsafe {
            let at = |o: usize| item.add(o);
            // 1.
            *at(3) = 0;
            *at(4) = 0;
            *at(5) = 0;
            // 2.
            let t = *at(0);
            if t == 0 {
                for o in [2, 1, 8, 9, 10, 11] {
                    *at(o) = 0;
                }
                return;
            }
            // 3.
            match dl {
                1 => *at(2) = (self.random(31) + 70) as u8,
                2 => *at(2) = 100,
                _ => {}
            }
            // 4.
            if dl != 0 {
                for o in 8..12 {
                    *at(o) = 0;
                }
            }
            let cls = self.item_type(t, 6);
            let top = self.item_type(t, 5);
            // 5.
            if cls == 1 {
                if top == 99 || bl == 2 {
                    *at(1) = top;
                } else if bl == 0 {
                    if (*at(1) as i8) > (top as i8) {
                        *at(1) = top;
                    }
                } else if bl == 1 {
                    *at(1) = (self.random(top as i8 as i32 - 1) + 1) as u8;
                }
                return;
            }
            // 6.
            if (*at(1) as i8 as i32) > cls as i32 {
                *at(1) = cls;
            }
            if *at(1) == 0 {
                *at(1) = 1;
            }
            // 7.
            // SPEC GAP: the loop bound is s8 +1 - 1 and is not limited to the 4 unit bytes +8..+11; a count above
            // 5 (only possible with a stack class above 5) writes past the 12-byte entry, as written.
            let mut i: i32 = 0;
            while i < *at(1) as i8 as i32 - 1 {
                let p = at(8 + i as usize);
                match dl {
                    1 => *p = (self.random(31) + 70) as u8,
                    2 => *p = 100,
                    _ => {
                        if *p == 0 {
                            *p = *at(2);
                        }
                    }
                }
                i += 1;
            }
        }
    }

    // ---- Slot reset ----
    fn slot_reset(&self, record: usize, k: i32) {
        let s = self.rec(record);
        unsafe {
            let b = |o: usize, v: u8| *s.add(o) = v;
            let rw = |o: usize| u16::from_le_bytes([*s.add(o), *s.add(o + 1)]);
            let ww = |o: usize, v: u16| {
                let [lo, hi] = v.to_le_bytes();
                *s.add(o) = lo;
                *s.add(o + 1) = hi;
            };
            // 1.
            for o in [
                0x1AB, 0xB9, 0x1BD, 0x1F3, 0x17F, 0x1A4, 0x1C7, 0x1AD, 0x1B9, 0x189, 0x1D2, 0x181, 0x1A6, 0x1A7,
            ] {
                b(o, 0);
            }
            // 2.
            b(0x9C, 0xFF);
            b(0x188, 1);
            b(0x1C8, 0x78);
            // 3.
            for o in [0x12A, 0xDE, 0xE0, 0x134] {
                ww(o, NONE);
            }
            ww(0x1E8, 4);
            ww(0x10A, 0);
            // 4.
            b(0x17A, k as u8);
            // 5.
            ww(0x124, rw(0x12A));
            ww(0x114, rw(0x12A));
            // 6.
            b(0x180, (k > 3) as u8);
            // 7.
            ww(0xE4, rw(0xDE));
            ww(0xE6, rw(0xE0));
            // 8.
            if k >= 8 {
                // SPEC GAP: k >= 48 has no record k; that write is skipped (+0x1B1 on s is still cleared).
                if (k as usize) < SLOTS {
                    let r = self.rec(k as usize);
                    *r.add(0x182) = 0;
                    *r.add(0x17E) = 1;
                }
                b(0x1B1, 0);
            }
        }
    }

    // ---- Equipment roll ----
    fn equip(&self, slot: usize, flag: u8) {
        let s = self.rec(slot);
        let rd = |o: usize| unsafe { *s.add(o) };
        let wr = |o: usize, v: u8| unsafe { *s.add(o) = v };
        let item = |k: usize| unsafe { s.add(12 * k) };
        let swap = |old: u8, new: u8| {
            for k in 7..12 {
                if rd(12 * k) == old {
                    wr(12 * k, new);
                }
            }
        };
        let roll = |x: i32| self.random(100) < x;
        let a = || self.supply(0);
        let b_ = || self.supply(1);
        let c_ = || self.supply(3);

        // 1.
        if rd(0xBA) == 4 {
            wr(0x18, 0);
            wr(0x3C, 0);
            return;
        }
        // 2.
        if flag == 0 {
            for k in 0..12 {
                let m = (rd(12 * k + 2) == 0) as u8;
                self.item_reset(item(k), m, m);
            }
        } else {
            for o in 0..0x90 {
                wr(o, 0);
            }
        }
        // 3.
        let lv = (rd(0x192) as i32 - 4) * 5;
        let day = unsafe { (*self.w).day };
        let d = if day < 5 { day.wrapping_mul(5).wrapping_sub(25) } else { 0 };
        let diff = unsafe { (*self.w).difficulty } as usize;
        // SPEC GAP: a difficulty above 2 indexes past the 6-byte table; it reads 0.
        let skill = Self::table(unsafe { (*self.w).equip_skill }, 6, 2 * diff) as i32;
        let c = skill.wrapping_add(d).wrapping_add(lv) as u8 as i8 as i32;
        // 4.
        wr(0x1B1, roll(a() + c + 15) as u8);
        // 5.
        if flag != 0 || a() != 0 {
            // a.
            let old = rd(0);
            let mut u: i32 = 7;
            loop {
                if roll((7 - u) * 5 + c + b_()) {
                    break;
                }
                u -= 1;
                if u == 0 {
                    break;
                }
            }
            let new = GUNS[u as usize];
            // b.
            if new > old {
                if old != 0 {
                    swap(old, new);
                    if (1..=14).contains(&old) {
                        swap(self.weapon_ammo(old), self.weapon_ammo(new));
                    }
                }
                wr(0, new);
                self.item_reset(item(0), 1, 1);
            }
            // c.
            if rd(0xC) == 0 {
                if roll(c_() + c + 15) {
                    // SPEC GAP: a host random outside 0..6 / 0..5 picks entry 0.
                    let t = THROWN.get(self.random(7) as usize).copied().unwrap_or(THROWN[0]);
                    wr(0xC, t);
                    let n = COUNTS.get(self.random(6) as usize).copied().unwrap_or(COUNTS[0]);
                    wr(0xD, n);
                    self.item_reset(item(1), 1, 0);
                } else {
                    let b = if rd(0x1D3) == 5 { 25 } else { 0 };
                    if roll(a() + b + 25) {
                        let t = if roll(a() + b + 10) { 0x10 } else { 0xF };
                        wr(0xC, t);
                        self.item_reset(item(1), 1, 0);
                    }
                }
            }
        }
        // 6.
        for (k, top, field) in [(2usize, 0x26i32, 0x18usize), (5, 0x30, 0x3C)] {
            let base = top - 3;
            let mut picked: Option<u8> = None;
            let mut t = top;
            while t >= base {
                if (t - base) < rd(0x1B4) as i32 && roll(a() + c + 50) {
                    picked = Some(t as u8);
                    break;
                }
                t -= 1;
            }
            match picked {
                Some(t) => {
                    let f = rd(field);
                    if f != 0 {
                        swap(f, t);
                    }
                    wr(field, t);
                    self.item_reset(item(k), 1, 0);
                }
                None => wr(field, 0),
            }
        }
        // 7.
        if rd(0x1B4) == 4 && roll(a() + c) {
            wr(0x48, 0x31);
        } else {
            wr(0x48, (0x32 + self.random(4)) as u8);
        }
        self.item_reset(item(6), 1, 0);
        // 8.
        for (k, field, high, low) in [(4usize, 0x30usize, 0x2Bu8, 0x2Au8), (3, 0x24, 0x28, 0x27)] {
            if rd(field) != 0 {
                continue;
            }
            let set = if roll(a() + c) {
                wr(field, high);
                true
            } else if roll(c + 25 + a()) {
                wr(field, low);
                if k == 3 {
                    wr(0x25, 1);
                }
                true
            } else {
                false
            };
            if set {
                self.item_reset(item(k), 1, 0);
            }
        }
        // 9.
        if rd(3) == 0 {
            let (mut have_b, mut have_l) = (false, false);
            for e in 0..ATTACH {
                if self.attach(4 * e + 1) == rd(0) {
                    match self.attach(4 * e) {
                        0x42 => have_b = true,
                        0x5B => have_l = true,
                        _ => {}
                    }
                }
            }
            if have_b && roll(c - 5 + a()) {
                wr(3, 0x42);
                wr(4, 1);
                wr(5, (self.random(31) + 70) as u8);
            }
            if have_l && rd(3) != 0x42 && roll(c - 10 + a()) {
                wr(3, 0x5B);
                wr(4, 1);
                wr(5, (self.random(31) + 70) as u8);
            }
        }
        // 10.
        if flag == 0 {
            for k in 7..10 {
                let t = rd(12 * k);
                if (0x36..=0x3E).contains(&t) {
                    wr(12 * k + 1, self.stack_count(t));
                    self.item_reset(item(k), 1, 0);
                }
            }
            if rd(0xC) == 0x1C && rd(0xE) == 100 {
                wr(0xE, (self.random(50) + 50) as u8);
            }
        } else {
            let mut dst = 7usize;
            for k in 0..6 {
                if rd(12 * k) != 0 && roll(8) {
                    for o in [0, 1, 2, 8, 9, 10, 11] {
                        wr(12 * dst + o, rd(12 * k + o));
                    }
                    self.item_reset(item(dst), 0, 0);
                    dst += 1;
                    if dst > 9 {
                        break;
                    }
                }
            }
        }
        // 11.
        let t = rd(0);
        if (1..=14).contains(&t) && roll(if flag != 0 { 45 } else { 25 }) {
            wr(0x84, self.weapon_ammo(t));
            wr(0x85, self.stack_count(rd(0x84)));
            self.item_reset(item(11), 1, 0);
        }
    }
}

/// Low 16 bits read as signed.
fn s16(v: i32) -> i32 {
    v as i16 as i32
}

// ================= Batch 2: placing soldiers =================
impl Ctx {
    // ---- record fields of slot `r` (callers check r < SLOTS) ----
    fn g8(&self, r: usize, o: usize) -> u8 {
        unsafe { *self.rec(r).add(o) }
    }
    fn p8(&self, r: usize, o: usize, v: u8) {
        unsafe { *self.rec(r).add(o) = v }
    }
    fn g16(&self, r: usize, o: usize) -> u16 {
        u16::from_le_bytes([self.g8(r, o), self.g8(r, o + 1)])
    }
    fn p16(&self, r: usize, o: usize, v: u16) {
        let [lo, hi] = v.to_le_bytes();
        self.p8(r, o, lo);
        self.p8(r, o + 1, hi);
    }

    // ---- shared tile helpers (ai.md) ----
    fn step_dir(&self, t: i32, d: i32) -> i32 {
        shared::step(t, shared::dir_step(self.w, self.h, d))
    }
    fn standable(&self, t: i32) -> i32 {
        shared::tile_standable(self.w, self.h, t)
    }
    fn occupant(&self, t: i32) -> u8 {
        shared::occupant(self.w, self.h, t)
    }
    fn occupy(&self, t: i32, slot: u8) {
        shared::occupy(self.w, self.h, t, slot);
    }
    /// Tile byte write; SPEC GAP: a tile outside 0..3135 (or a null tile buffer) is not written.
    fn tile_put(&self, t: i32, i: usize, v: u8) {
        unsafe {
            let p = (*self.w).tiles;
            if !p.is_null() && (0..TILES).contains(&t) {
                *p.add(t as usize * 8 + i) = v;
            }
        }
    }
    fn tile_get(&self, t: i32, i: usize) -> u8 {
        unsafe {
            let p = (*self.w).tiles;
            if !p.is_null() && (0..TILES).contains(&t) {
                *p.add(t as usize * 8 + i)
            } else {
                0
            }
        }
    }

    /// One host call. No host call function: result 0.
    fn host(&self, opc: i32, args: &[i32]) -> u32 {
        unsafe {
            let h = &*self.h;
            match h.call {
                Some(c) => c(h.ctx, opc, args.as_ptr(), args.len() as i32, std::ptr::null_mut(), 0),
                None => 0,
            }
        }
    }

    /// The record of the slot number in byte +0x17A of record s, if it is 0..47.
    fn self_rec(&self, s: usize) -> Option<usize> {
        let r = self.g8(s, 0x17A) as usize;
        // SPEC GAP: a +0x17A value of 48 or more names no record; the steps that work on that record are skipped.
        (r < SLOTS).then_some(r)
    }
    fn item_of(&self, r: usize, k: usize) -> *mut u8 {
        unsafe { self.rec(r).add(12 * k) }
    }

    // ---- XRA enemy ----
    fn xra_enemy(&self, s: usize) {
        // 1.
        self.p8(s, 0x1E0, self.random(4) as u8);
        self.p8(s, 0x182, 1);
        self.p8(s, 0x138, 1);
        // 2.
        let off = match self.random(3) {
            1 => 0x116,
            2 => 0x118,
            // SPEC GAP: a host random outside 0..2 picks +0x114 (as for 0).
            _ => 0x114,
        };
        let t = self.g16(s, off) as i16 as i32;
        // 3.
        if self.g8(s, 0x3C) == 0x31 {
            self.p8(s, 0x3C, 0x30);
        }
        // 4.
        for e in [0x54usize, 0x60, 0x6C] {
            if self.g8(s, e) == 0 {
                self.p8(s, e + 1, 0);
                self.p8(s, e + 2, 0);
            } else if self.g8(s, e + 3) == 0 {
                self.p8(s, e + 4, 0);
                self.p8(s, e + 5, 0);
            }
        }
        // 5.
        for o in [0x124, 0x114, 0x12E] {
            self.p16(s, o, t as u16);
        }
        // 6.
        self.p8(s, 0x185, 0);
        self.p8(s, 0x1BD, 0x6D);
        self.p16(s, 0x10C, 0);
        self.p8(s, 0x18A, 100);
        self.p8(s, 0x184, self.g8(s, 0x183));
        self.p8(s, 0x17B, self.g8(s, 0x17B).wrapping_add(4));
        self.p8(s, 0x17C, (self.random(8) + 2) as u8);
        // 7.
        self.occupy(t, self.g8(s, 0x17A));
        // 8.
        self.replace_op(s);
    }

    // ---- Tapper ----
    /// InitTapper 0x2BB90.
    fn init_tapper(&self, s: usize, tile: i32, facing: u8) {
        // 1.
        for o in [0x182, 0x138, 0x9C] {
            self.p8(s, o, 1);
        }
        self.p8(s, 0x17E, 0);
        self.p8(s, 0x1BD, 0x6C);
        self.p8(s, 0x17B, 0x19);
        // 2.
        for o in [0x124, 0x114, 0x12E] {
            self.p16(s, o, tile as u16);
        }
        // 3.
        for o in [0x1A2, 0x1A1, 0x1A4] {
            self.p8(s, o, facing);
        }
        // 4.
        self.p8(s, 0x17C, (self.random(8) + 2) as u8);
        let r = self.random(4);
        // 5.
        self.p8(s, 0x1D4, 0);
        self.p8(s, 0x1D3, 0);
        self.p8(s, 0x19C, 7);
        self.p8(s, 0x1E0, r as u8);
        self.p8(s, 0x192, 1);
        // 6.
        for (o, base, n) in [(0x18B, 30, 26), (0x18C, 40, 26), (0x183, 40, 21), (0x18D, 20, 26)] {
            let v = self.random(n) + base;
            self.p8(s, o, v.wrapping_add(self.random(n)) as u8);
        }
        // 7.
        self.p8(s, 0x185, 0);
        self.p16(s, 0x10C, 0);
        self.p8(s, 0x18A, 100);
        self.p8(s, 0x1F3, 0);
        self.p8(s, 0x184, self.g8(s, 0x183));
        self.p16(s, 0x1E8, 0x11);
        // 8.
        // SPEC GAP: Occupy gets the `tile` argument as the full int32 (the step does not say s16).
        self.occupy(tile, self.g8(s, 0x17A));
    }

    // ---- Guard ----
    /// InitGuard 0x2B7DC.
    fn init_guard(&self, s: usize, tile: i32, facing: u8, orders: u8, anchor: i32) {
        // 1.
        for o in [0x182, 0x9C, 0x138] {
            self.p8(s, o, 1);
        }
        self.p8(s, 0x17E, 0);
        self.p8(s, 0x1BD, 0x6B);
        self.p8(s, 0x17B, 0x17);
        // 2.
        self.p16(s, 0x124, tile as u16);
        self.p16(s, 0x114, tile as u16);
        self.p8(s, 0x1A2, facing);
        self.p8(s, 0x1A1, facing);
        // 3.
        self.p8(s, 0x17C, (self.random(8) + 2) as u8);
        self.p8(s, 0x1E0, self.random(4) as u8);
        self.p16(s, 0x12E, anchor as u16);
        self.p8(s, 0x1D4, orders);
        self.p8(s, 0x1D3, self.random(6) as u8);
        self.p8(s, 0x19C, 7);
        // 4.
        let day = unsafe { (*self.w).day };
        let n = s16((day as u16 as i32) + 5);
        self.p8(s, 0x192, (self.random(n) / 10 + 1) as u8);
        if self.g8(s, 0x192) as i8 > 9 {
            self.p8(s, 0x192, 9);
        }
        // 5.
        for (o, base) in [(0x18B, 40), (0x18C, 40), (0x183, 40), (0x18D, 20), (0x191, 30)] {
            let v = self.random(26) + base;
            self.p8(s, o, v.wrapping_add(self.random(26)) as u8);
        }
        let v = self.random(16);
        self.p8(s, 0x18E, v.wrapping_add(self.random(16)) as u8);
        // 6.
        let a = (self.g8(s, 0x192) as i32 - 1).wrapping_mul(5) as u8;
        if a != 0 {
            for o in [0x18B, 0x18C, 0x183, 0x18D, 0x191, 0x18E] {
                self.p8(s, o, self.g8(s, o).wrapping_add(a));
                if self.g8(s, o) as i8 > 100 {
                    self.p8(s, o, 100);
                }
            }
        }
        // 7.
        let c = unsafe { (*self.w).guard_skill }.wrapping_add(a) as i32;
        self.p8(s, 0x1B1, (self.random(100) < c) as u8);
        // 8. (R is fixed here; a later change of +0x17A does not move it.)
        let r_rec = self.self_rec(s);
        // 9.
        let extra = (self.random(2) != 0) as i32;
        self.p8(s, 0, (1 + extra + c / 5).min(0xE) as u8);
        if let Some(r) = r_rec {
            self.item_reset(self.item_of(r, 0), 1, 2);
        }
        // 10.
        if self.random(100) < c + 50 {
            let more = (self.random(100) < c + 25) as u8;
            self.p8(s, 0xC, 0xF + more);
            if let Some(r) = r_rec {
                self.item_reset(self.item_of(r, 1), 1, 2);
            }
        }
        // 11.
        if c != 0 {
            for (field, base, top, k) in [(0x18usize, 0x22u8, 0x26u8, 2usize), (0x3C, 0x2C, 0x30, 5)] {
                let q = (self.random(c) / 10) as u8;
                if q != 0 {
                    let v = q.wrapping_add(base);
                    self.p8(s, field, if v > top { top } else { v });
                    if let Some(r) = r_rec {
                        self.item_reset(self.item_of(r, k), 1, 0);
                    }
                }
            }
        }
        // 12.
        self.p8(s, 0x185, 0);
        self.p16(s, 0x10C, 0);
        self.p8(s, 0x184, self.g8(s, 0x183));
        self.p8(s, 0x18A, 100);
        // 13.
        // SPEC GAP: Occupy gets the `tile` argument as the full int32 (the step does not say s16).
        self.occupy(tile, self.g8(s, 0x17A));
        self.replace_op(s);
    }

    // ---- Guard tile ----
    /// GuardTile 0x2C70C.
    fn guard_tile(&self, tile: i32, dir: u8, dist: i32) -> i32 {
        let d0 = dir;
        let mut d = d0;
        let mut dist = dist;
        let mut misses = 0;
        // 1.
        if dist > 27 {
            return tile;
        }
        // SPEC GAP: with d0 outside 1..8, d never comes back to d0, so dist never grows and the loop as written
        // can run forever (when no tile has a class above 7); the library gives up after 1,000,000 passes and
        // returns tile.
        for _ in 0..1_000_000 {
            // a.
            let mut t = tile;
            for _ in 0..dist.max(0) {
                t = self.step_dir(t, d as i32);
            }
            // b.
            let c = shared::ground_class(self.w, self.h, t);
            // c.
            if self.standable(t) > 0 && self.occupant(t) == 0x40 && c != 3 && c < 8 {
                return t;
            }
            // d.
            d = d.wrapping_add(1);
            if d > 8 {
                d = 1;
            }
            if d == d0 {
                dist += 1;
            }
            // e.
            if c > 7 {
                misses += 1;
                if misses >= 150 {
                    return tile;
                }
            }
            // f.
            if dist > 27 {
                return tile;
            }
        }
        tile
    }

    // ---- Place merc ----
    /// PlaceMercOnTile 0x2B380.
    fn place_merc(&self, s: usize, tile: i32, facing: u8) {
        // 1.
        self.p16(s, 0x124, tile as u16);
        self.p16(s, 0x114, tile as u16);
        let t = self.g16(s, 0x114) as i16 as i32;
        // 2.
        let me = self.g8(s, 0x17A);
        self.p8(s, 0x1E0, me);
        self.tile_put(t, 5, me);
        self.tile_put(t, 4, self.tile_get(t, 4) | 0x80);
        // 3.
        self.p8(s, 0x1CF, 0);
        self.p8(s, 0x9C, 1);
        self.p8(s, 0x1A2, facing);
        self.p8(s, 0x1A1, facing);
        // 4.
        self.replace_op(s);
        // 5.
        self.p8(s, 0x1BD, 0x6A);
        self.p8(s, 0x138, 1);
        // 6.
        if self.g8(s, 0x179) > 0x3B {
            self.p8(s, 0x1D4, 2);
            self.p8(s, 0x1D3, 2);
        } else if (self.g8(s, 0x18E) as i8) > (self.g8(s, 0x191) as i8) {
            self.p8(s, 0x1D4, 3);
            self.p8(s, 0x1D3, 0);
        } else {
            self.p8(s, 0x1D4, 4);
            let v = match self.g8(s, 0x19C) {
                3 => 5,
                4 | 8 => 0,
                2 | 6 => self.random(5) as u8,
                _ => self.random(6) as u8,
            };
            self.p8(s, 0x1D3, v);
        }
    }

    // ---- Merc entry ----
    /// PlaceMercsAtEntry 0x2B470.
    fn merc_entry(&self, entry_tile: i32, facing: u8) {
        let mut tile = entry_tile;
        let mut facing = facing;
        let sel = unsafe { (*self.w).selected } as usize;
        let mut placed = [false; MERCS];
        // placed[] has entries 0..7 only; a read of a higher index (a +0x17A byte of 8 or more) counts as "yes"
        // (so no loop waits on it) and a write to one is dropped: step 1 below copies what selected 8..47 overwrites.
        let is_placed = |p: &[bool; MERCS], i: usize| p.get(i).copied().unwrap_or(true);
        let set_placed = |p: &mut [bool; MERCS], i: usize| {
            if let Some(e) = p.get_mut(i) {
                *e = true;
            }
        };
        let mut last: i32 = -1;
        let mut ring2 = false;
        let mut lead: Option<i32> = None;
        // 1.
        if self.standable(tile) > 0 {
            // SPEC GAP: a selected slot of 48 or more names no record; Place merc is skipped (lead is still set).
            // The original then writes past the soldiers and over the caller's stack. Only 0x40 can occur, and
            // only with a left click in the overview of a sector with no merc (#2 row 48).
            if sel < SLOTS {
                self.place_merc(sel, tile, facing);
            }
            set_placed(&mut placed, sel);
            // Original bug (issue #2 row 48): placed[] is 8 bytes on the stack at 0x2B4C8, the other locals follow.
            // Byte 1 goes over: 8..11 the entry tile, 12..15 the last merc used, 24 and 25 the ring 2 word, 32 the
            // facing. 16..23 and 28..31 are written again before a read, 26, 27 and 33..35 are not read, and 36..47 are
            // the caller's EBP, EDI and ESI, which SetupSectorFromXra sets again before a read (at 0x2F3FC..0x2F46A).
            // LoadTacticalSector? gives 8 with no merc on duty; no road is known for 9 or more (see #2 row 48).
            let byte_over = |v: i32, at: usize| (v & !(0xFF << (at * 8))) | (1 << (at * 8));
            match sel {
                8..=11 => tile = byte_over(tile, sel - 8),
                12..=15 => last = byte_over(last, sel - 12),
                24 | 25 => ring2 = true,
                32 => facing = 1,
                _ => {}
            }
            lead = Some(s16(tile));
        }
        // 2. SPEC GAP: at 48 or more the original reads past the soldier table; the port skips the call.
        if sel < SLOTS {
            self.reserve_op(sel);
        }
        // 3.
        for k in 0..MERCS {
            // a.
            if self.g8(k, 0x182) == 0 || self.g8(k, 0x138) == 0 || self.g8(k, 0x183) == 0 {
                self.p8(k, 0x138, 0);
                continue;
            }
            // b.
            let mut base = tile;
            while !is_placed(&placed, self.g8(k, 0x17A) as usize) {
                let mut found = false;
                for d in 1..=8 {
                    let mut c = self.step_dir(base, d);
                    if ring2 {
                        c = self.step_dir(c, d);
                    }
                    if self.standable(c) > 0 && self.occupant(c) == 0x40 {
                        self.place_merc(k, c, facing);
                        let me = self.g8(k, 0x17A) as usize;
                        set_placed(&mut placed, me);
                        if me == sel {
                            lead = Some(s16(c));
                        }
                        found = true;
                        break;
                    }
                }
                if found {
                    continue;
                }
                // SPEC GAP: `last` below -1 (selected 12..14) reads stack bytes under the frame in the original. They are
                // not defined here, so the search starts at merc 0.
                let mut j = last.saturating_add(1).max(0);
                while j < MERCS as i32 && !placed[j as usize] {
                    j += 1;
                }
                if j < MERCS as i32 {
                    base = self.g16(j as usize, 0x114) as i16 as i32;
                    last = j;
                } else if ring2 {
                    break;
                } else {
                    ring2 = true;
                }
            }
        }
        // 4.
        for i in (0..MERCS).rev() {
            if placed[i] && i != sel {
                let ti = self.g16(i, 0x114) as i16 as i32;
                // SPEC GAP: lead unset here is undefined in the original; the library uses s16 of the entry tile.
                let ld = lead.unwrap_or(s16(tile));
                if sel < SLOTS {
                    self.place_merc(sel, ti, facing);
                }
                self.place_merc(i, ld, facing);
                break;
            }
        }
    }
}

// ================= Batches 3 and 4: ground items =================
impl Ctx {
    /// Byte 6 of tile t (its ground item entry); a tile outside 0..3135 reads 0xFF (sector_start.md batch 4).
    fn tile_g(&self, t: i32) -> u8 {
        if (0..TILES).contains(&t) {
            self.tile_get(t, 6)
        } else {
            0xFF
        }
    }
    /// Byte i of ground entry g; entries g of 200 or more read past the 200 entries (`shared::ground_byte`).
    fn gi(&self, g: usize, i: usize) -> u8 {
        shared::ground_byte(self.w, 12 * g + i)
    }
    /// Write byte `off` of the ground entries, past the 200 entries as `gi` reads.
    fn gi_put(&self, off: usize, v: u8) {
        shared::ground_byte_put(self.w, off, v)
    }

    // ---- helpers (batch 3) ----
    fn water(&self, t: i32) -> bool {
        let t1 = self.tile_get(t, 1);
        self.tile_get(t, 0) >= 0x6C && t1 != 0x1C && t1 != 0x1F
    }
    fn surface(&self, t: i32) -> bool {
        let t1 = self.tile_get(t, 1);
        let c = Self::table(unsafe { (*self.w).t1_class }, 256, t1 as usize);
        if c <= 5 || c >= 9 {
            return true;
        }
        matches!(t1, 0x3B | 0x41 | 0x5A | 0x60)
    }
    fn item_tile_ok(&self, t: i32, flag: u8) -> bool {
        let g = self.tile_g(t);
        if g != 0xFF && ((self.gi(g as usize, 2) as i8) > 0 || (self.gi(g as usize, 6) as i8) != -1) {
            return false;
        }
        if self.water(t) {
            return false;
        }
        flag != 0 || self.surface(t)
    }
    fn find_item_tile(&self, t: i32, flag: u8) -> i32 {
        if self.item_tile_ok(t, flag) {
            return t;
        }
        for r in 1..=11 {
            for d in 1..=8 {
                let mut u = t;
                for _ in 0..r {
                    u = self.step_dir(u, d);
                }
                if self.item_tile_ok(u, flag) {
                    return u;
                }
            }
        }
        NONE as i32
    }
    fn note_placed(&self, ty: u8) -> bool {
        let p = unsafe { (*self.w).notes };
        if p.is_null() {
            // SPEC GAP: a null notes buffer has no match.
            return false;
        }
        for k in 0..NOTES {
            unsafe {
                if *p.add(2 * k) == ty {
                    if *p.add(2 * k + 1) == 1 {
                        return true;
                    }
                    *p.add(2 * k + 1) = 1;
                    return false;
                }
            }
        }
        false
    }

    // ---- Put item ----
    fn put_item(&self, tile: i32, item: *const u8, b6: u8, exact: u8, note: u8, near: u8) {
        let it = |i: usize| unsafe { *item.add(i) };
        // SPEC GAP: a null ground_items buffer does nothing.
        if unsafe { (*self.w).ground_items }.is_null() {
            return;
        }
        // 1.
        let ty = it(0);
        if ty == 0 {
            return;
        }
        // 2.
        if note != 0 && self.note_placed(ty) {
            return;
        }
        // 3.
        if self.water(tile) && self.item_type(ty, 13) != 0 {
            return;
        }
        // 4.
        let k = (0..GROUND).find(|&k| self.gi(k, 0) == 0).unwrap_or(GROUND); // FreeGroundItem? 0x2390C
        // 5.
        let dest = if exact != 0 { tile } else { self.find_item_tile(tile, near) };
        // 6.
        if k < GROUND && dest != NONE as i32 {
            // a.
            self.tile_put(dest, 6, k as u8);
            // b.
            for i in 0..12 {
                self.gi_put(12 * k + i, it(i));
            }
            self.gi_put(12 * k + 6, b6);
            // c.
            if self.item_type(self.gi(k, 0), 6) > 1 && (self.gi(k, 1) as i8) > 1 {
                let mut j: i32 = 0;
                while j < self.gi(k, 1) as i8 as i32 - 1 {
                    let off = 12 * k + 8 + j as usize;
                    // PutItem 0x3FDDB has no bound: for entry 199 a big stack fills on into g_textBuf 0xEBAC0 and
                    // g_profiles 0xEBB40, as gi / gi_put read and write them (SPEC GAP: past the profiles, nothing).
                    if self.gi(0, off) == 0 {
                        self.gi_put(off, self.gi(k, 2));
                    }
                    j += 1;
                }
            }
        }
        // 7.
        if dest != tile {
            let g = self.tile_g(tile);
            if g < 0xFF && (self.gi(g as usize, 6) as i8) != -1 {
                self.tile_put(tile, 6, k as u8);
                if dest != NONE as i32 {
                    self.tile_put(dest, 6, g);
                }
            }
        }
    }

    // ---- helpers (batch 4) ----
    fn listed(&self, ty: u8) -> bool {
        let p = unsafe { (*self.w).listed };
        ty != 0 && (0..LISTED).any(|i| Self::table(p, LISTED, i) == ty)
    }
    fn blast_remove(&self, t: i32) {
        let p = unsafe { (*self.w).blast_list };
        if p.is_null() {
            return;
        }
        for k in 0..BLAST {
            unsafe {
                let v = u16::from_le_bytes([*p.add(2 * k), *p.add(2 * k + 1)]) as i16 as i32;
                if v == t {
                    let [lo, hi] = NONE.to_le_bytes();
                    *p.add(2 * k) = lo;
                    *p.add(2 * k + 1) = hi;
                    return;
                }
            }
        }
    }
    /// combat.md "Rebuilding all move codes" into `move_codes`, from tiles, t0_class and t1_class.
    /// The door pass marks the door sides as MarkDoorSide 0x2BFEC.
    fn rebuild_move_codes(&self) {
        unsafe {
            let w = &*self.w;
            // SPEC GAP: a null tiles, t0_class, t1_class or move_codes buffer skips the rebuild.
            if w.tiles.is_null() || w.t0_class.is_null() || w.t1_class.is_null() || w.move_codes.is_null() {
                return;
            }
            let tiles = std::slice::from_raw_parts(w.tiles as *const u8, TILES as usize * 8);
            let gc = &*(w.t0_class as *const [u8; 256]);
            let oc = &*(w.t1_class as *const [u8; 256]);
            let mut out = vec![0u16; TILES as usize];
            crate::rebuild(tiles, gc, oc, &mut out);
            for (i, c) in out.iter().enumerate() {
                let [lo, hi] = c.to_le_bytes();
                *w.move_codes.add(2 * i) = lo;
                *w.move_codes.add(2 * i + 1) = hi;
            }
        }
    }

    // ---- Take ground item ----
    fn take_ground_item(&self, tile: i32, keep: u8) {
        // 1.
        if !(0..=0xC40).contains(&tile) || self.tile_g(tile) == 0xFF {
            return;
        }
        // 2.
        let g = self.tile_g(tile) as usize;
        if self.tile_get(tile, 1) == 0x86 {
            self.tile_put(tile, 1, 0);
            self.rebuild_move_codes();
        }
        // 3.
        self.blast_remove(tile);
        // 4.
        if keep != 0 {
            self.host(op::CURSOR_BACK, &[]);
            let held = unsafe { (*self.w).held };
            // SPEC GAP: a null held buffer is not written.
            if !held.is_null() {
                for i in 0..12 {
                    unsafe { *held.add(i) = self.gi(g, i) };
                }
                let ty = unsafe { *held };
                if self.item_type(ty, 0xA) == 0 && !matches!(ty, 0x6F | 0x57 | 0x58) {
                    unsafe { *held.add(2) = 100 };
                }
            }
        } else if self.listed(self.gi(g, 0)) {
            return;
        }
        // 5.
        self.tile_put(tile, 6, 0xFF);
        if g < GROUND {
            for i in 0..7 {
                self.gi_put(12 * g + i, 0);
            }
        }
    }

    // ---- Take ground patch ----
    fn take_ground_patch(&self, tile: i32, ty: u8, depth: u32) {
        // SPEC GAP: a listed type recurses forever in the original (not tested); the library stops at depth 4096
        // (deeper than any patch of the 3136-tile map that ends).
        if depth >= PATCH_DEPTH {
            return;
        }
        // 1.
        self.take_ground_item(tile, 0);
        // 2.
        for d in [-56, 1, 56, -1] {
            let t = s16(tile.wrapping_add(d));
            let g = self.tile_g(t);
            if g != 0xFF && self.gi(g as usize, 0) == ty {
                self.take_ground_patch(t, ty, depth + 1);
            }
        }
    }
}

// ================= Batch 5: soldier start =================
impl Ctx {
    fn p32(&self, r: usize, o: usize, v: u32) {
        for (i, b) in v.to_le_bytes().iter().enumerate() {
            self.p8(r, o + i, *b);
        }
    }
    fn s8f(&self, r: usize, o: usize) -> i32 {
        self.g8(r, o) as i8 as i32
    }
    /// Write byte i of a world buffer of `len` bytes; a null buffer or an index outside it is not written.
    fn buf_put(p: *mut u8, len: usize, i: i64, v: u8) {
        // SPEC GAP: a last-seen / remember / enemy_watch write outside its table (or to a null table) is dropped.
        if !p.is_null() && i >= 0 && (i as u64) < len as u64 {
            unsafe { *p.add(i as usize) = v }
        }
    }

    /// Shot end (silent).
    fn shot_end_silent(&self, s: usize) {
        for o in [0xDC, 0xDE, 0xE0, 0xE4, 0xE6, 0x1E6] {
            self.p16(s, o, NONE);
        }
        for o in [0x1BF, 0x1C7, 0x1FF] {
            self.p8(s, o, 0);
        }
        self.p8(s, 0x1BC, 1);
        unsafe {
            if (*self.w).busy > 0 {
                (*self.w).busy -= 1;
            }
        }
    }
    /// Clear target.
    fn clear_target(&self, s: usize) {
        self.p8(s, 0x1D8, 0x40);
        self.p8(s, 0x1C2, 0x40);
        self.p8(s, 0x1BE, 0);
        self.p16(s, 0xD0, NONE);
        self.p16(s, 0xDC, NONE);
    }
    /// Armour bonus.
    fn armour_bonus(&self, s: usize) {
        self.p8(s, 0x1D0, (self.s8f(s, 0x192) > 3) as u8);
        if self.g8(s, 0x30) == 0x2B {
            let v = self.s8f(s, 0x1D0) + 5 - (100 - self.s8f(s, 0x32)) / 20;
            self.p8(s, 0x1D0, v as u8);
        }
    }
    /// Wakes (1 / 0).
    fn wakes(&self, s: usize) -> u8 {
        if unsafe { (*self.w).cur_sector } == 0 {
            return 1;
        }
        let base = match self.g8(s, 0x1D4) {
            0 => 5,
            1 => 10,
            2 => 50,
            3 | 4 => 25,
            _ => 0,
        };
        let x = s16(base + (self.s8f(s, 0x192) - 1) * 5);
        (self.random(100) < x) as u8
    }

    fn soldier_start(&self, s: usize) {
        let w = self.w;
        let slot = self.g8(s, 0x17A) as usize;
        // 1.
        for o in [0x1EA, 0x1EC, 0x1EE, 0x1F0] {
            self.p16(s, o, 1000);
        }
        // 2.
        let cur = unsafe { (*w).cur_sector };
        let (ty, name) = if slot >= 0x28 {
            if cur == 0 && slot == 0x28 {
                (4, Some("SANTINO".to_string()))
            } else {
                (3, Some(format!("ENEMY {}", slot)))
            }
        } else if slot >= 0x10 {
            (2, Some(format!("TAPPER{:2}", slot)))
        } else if slot >= 8 {
            (1, Some(format!("GUARD {}", slot)))
        } else {
            (0, None)
        };
        self.p8(s, 0xBA, ty);
        if let Some(n) = name {
            for (i, b) in n.bytes().chain(std::iter::once(0)).enumerate() {
                self.p8(s, 0x16E + i, b);
            }
        }
        // 3.
        for o in [
            0x1CC, 0x1FA, 0x1D7, 0x130, 0x1DB, 0x1FC, 0x1E1, 0x9D, 0x16D, 0x1D1, 0x1D6, 0x1C1, 0x1C0, 0x197, 0x196,
            0x193, 0x1D2,
        ] {
            self.p8(s, o, 0);
        }
        for o in [0x10E, 0x108, 0x132, 0x122, 0x120] {
            self.p16(s, o, NONE);
        }
        // 4.
        self.p8(s, 0x1BC, 1);
        let v = self.g8(s, 0x1E1);
        self.p8(s, 0x1E2, v);
        self.p8(s, 0x1E3, v);
        self.p32(s, 0xC0, 0xFFFF_FFFF);
        self.p32(s, 0xC4, 0xFFFF_FFFF);
        self.p8(s, 0x1F7, 0x40);
        self.p16(s, 0xEC, NONE);
        for o in [0x1FF, 0x1BA, 0x1FE] {
            self.p8(s, o, 0);
        }
        self.p8(s, 0x198, 0xFF);
        self.p16(s, 0x106, NONE);
        // 5.
        if self.g8(s, 0x183) != 0 {
            self.p8(s, 0x188, 0);
        }
        self.p8(s, 0x1A8, 0);
        self.p8(s, 0x1AC, 2);
        self.p8(s, 0x1A3, 0);
        self.p32(s, 0xA4, ((10 - self.s8f(s, 0x192)) * 10) as u32);
        self.p8(s, 0x177, 0);
        // 6.
        self.shot_end_silent(s);
        self.clear_target(s);
        // 7.
        self.p8(s, 0x1D5, 0);
        self.p16(s, 0xDC, NONE);
        self.p16(s, 0x1E6, NONE);
        self.p8(s, 0x1F6, 0);
        self.p16(s, 0x1F8, NONE);
        self.p8(s, 0x195, 0);
        self.p8(s, 0x1DA, 0x40);
        self.p16(s, 0xD0, NONE);
        self.p8(s, 0x1F2, 0);
        self.p8(s, 0x1FD, 0);
        // 8.
        if self.g8(s, 0x182) != 0 {
            // a.
            self.p8(s, 0x131, 0);
            self.p16(s, 0xF0, 0);
            self.p8(s, 0x1C4, 0);
            self.p8(s, 0x194, 0);
            self.p8(s, 0x1A5, shared::max_ap(w, self.h, s) as u8);
            self.armour_bonus(s);
            // b.
            self.p16(s, 0x11A, 0xA);
            self.p16(s, 0x11C, 0xD);
            let t = self.g16(s, 0x114) as i16 as i32;
            let x = (t % 56) * 16 + 5;
            let y = (t / 56) * 14 + 5;
            self.p16(s, 0x116, x as u16);
            self.p16(s, 0x126, x as u16);
            self.p16(s, 0x118, y as u16);
            self.p16(s, 0x128, y as u16);
            // c. SPEC GAP: tile bytes of a tile outside 0..3135 read as 0.
            let (t1c, t0c, t2c) = unsafe { ((*w).t1_class, (*w).t0_class, (*w).t2_class) };
            self.p8(s, 0x1B4, self.tile_get(t, 1));
            self.p8(s, 0x1B5, Self::table(t1c, 256, self.g8(s, 0x1B4) as usize));
            self.p8(s, 0x1B2, self.tile_get(t, 0));
            self.p8(s, 0x1B3, Self::table(t0c, 256, self.g8(s, 0x1B2) as usize));
            self.p8(s, 0x1B6, self.tile_get(t, 2));
            self.p8(s, 0x1B7, Self::table(t2c, 256, self.g8(s, 0x1B6) as usize));
            // d.
            let weak = (self.s8f(s, 0x183) < 15) as u8;
            self.p8(s, 0x186, weak);
            self.p8(s, 0x187, weak);
            self.p8(s, 0x1B9, 0);
            self.p16(s, 0xD6, NONE);
            for o in [0x1CA, 0x1A9, 0x1AE] {
                self.p8(s, o, 0);
            }
            self.p16(s, 0xD4, 0);
            self.p32(s, 0xB0, 100);
            self.p32(s, 0xBC, 0x32);
            self.p32(s, 0xA8, self.random(300) as u32);
            self.p32(s, 0xAC, 100);
            self.p32(s, 0x1DC, 0);
            self.p32(s, 0xB4, 0xFFFF_FFFF);
            // e.
            if slot < 0x28 {
                self.p8(s, 0xB9, 0);
                if slot < 8 && self.g8(s, 0x17E) == 8 {
                    self.p8(s, 0x17E, 0);
                }
            } else {
                let v = self.wakes(s);
                self.p8(s, 0xB9, v);
                let watch = matches!(self.g8(s, 0x1D4), 3 | 4) as u8;
                Self::buf_put(unsafe { (*w).enemy_watch }, SLOTS, slot as i64, watch);
            }
            // f.
            self.p32(s, 0xCC, 100);
            self.p8(s, 0x19F, 2);
            self.p8(s, 0x1B8, 1);
            self.p16(s, 0x136, 0);
            for o in [0x1A6, 0x1AD, 0x1F3] {
                self.p8(s, o, 0);
            }
            // g.
            let p = 0x50 * self.g16(s, 0x1E8) as usize;
            self.p32(s, 0x90, p as u32);
            self.p16(s, 0xC8, 0);
            self.p8(s, 0x1C9, 0);
            // SPEC GAP: a script position past the 68 x 0x50-byte table (or a null table) reads 0.
            self.p8(s, 0x1F4, Self::table(unsafe { (*w).anim_scripts }, 68 * 0x50, p));
            self.p8(s, 0x1AF, 0);
            shared::frame_delay(w, self.h, s);
            // h.
            self.p8(s, 0x1CD, 0);
            for o in 0x162..=0x169 {
                self.p8(s, o, 0);
            }
            self.p32(s, 0xA0, 500);
            // SetViewPose? 0x2B6A0 (called by UseInventoryItem? 0x1D9B8 when the inventory closes).
            self.p8(s, 0x19E, view_pose(self.g8(s, 0x192), self.g8(s, 0x24)));
            // i.
            if slot >= 8 {
                self.p8(s, 0x179, 0x40);
                self.p8(s, 0xEF, 0);
                self.p8(s, 0x1B0, 1);
            } else {
                self.p8(s, 0x1B0, 0);
            }
        } else {
            self.p8(s, 0x194, 1);
        }
        // 9.
        for o in 0x13A..=0x161 {
            self.p8(s, o, 0);
        }
        // 10.
        if self.s8f(s, 0xBA) != 2 {
            let sl = slot as i64;
            let [lo, hi] = NONE.to_le_bytes();
            let (p, len, base, n) = if slot < 0x28 {
                (unsafe { (*w).last_seen_m }, 16 * 8 * 2, sl * 16, 8)
            } else {
                (unsafe { (*w).last_seen_e }, 8 * 40 * 2, (sl - 0x28) * 80, 40)
            };
            for j in 0..n {
                Self::buf_put(p, len, base + 2 * j, lo);
                Self::buf_put(p, len, base + 2 * j + 1, hi);
            }
            // remember_m / remember_e are declared const in the world; this step writes them.
            let (p, len, base, n) = if slot < 0x10 {
                (unsafe { (*w).remember_m } as *mut u8, 16 * 8, sl * 8, 8)
            } else {
                (unsafe { (*w).remember_e } as *mut u8, 8 * 40, (sl - 0x28) * 40, 40)
            };
            for j in 0..n {
                Self::buf_put(p, len, base + j, 0);
            }
        }
        // 11.
        let v = if slot >= 0x28 { self.g8(s, 0x1A1) } else { 0 };
        self.p8(s, 0x1CF, v);
        // 12.
        for k in 0..12 {
            if self.g8(s, 12 * k) == 0 {
                for i in 0..12 {
                    self.p8(s, 12 * k + i, 0);
                }
            }
        }
    }
}

// ================= Batch 6: map patches, supply drops, plants =================
/// SetViewPose? 0x2B6A0: pose +0x19E = 7 when the level byte (+0x192) is 2 or less, else 8; +1 when item 3's
/// type (+0x24) is 0x27.
pub fn view_pose(level: u8, item3: u8) -> u8 {
    let mut p: u8 = if level <= 2 { 7 } else { 8 };
    if item3 == 0x27 {
        p = p.wrapping_add(1);
    }
    p
}

impl Ctx {
    /// One host call with an `out` buffer.
    fn host_out(&self, opc: i32, args: &[i32], out: &mut [u8]) -> u32 {
        unsafe {
            let h = &*self.h;
            match h.call {
                Some(c) => c(h.ctx, opc, args.as_ptr(), args.len() as i32, out.as_mut_ptr(), out.len() as i32),
                None => 0,
            }
        }
    }
    /// Op 87 read(n): the bytes the host got (0..n), as a slice of a local buffer.
    fn read(&self, n: usize) -> Vec<u8> {
        let mut out = vec![0u8; n];
        let got = self.host_out(op::READ, &[n as i32], &mut out) as i32;
        // SPEC GAP: a result below 0 counts as 0 bytes, above n as n.
        out.truncate(got.clamp(0, n as i32) as usize);
        out
    }
    /// Write the patch path and op 86 open_patch; false when the host answers -1.
    fn open_patch(&self, kind: i32, number: i32, count: i32) -> bool {
        let name = match kind {
            0 => "CRATE",
            1 => "ITEM",
            2 => "DEADPL",
            3 => "LIVEPL",
            // Kind 4 = DAT\XRA.DAT: the original passes a fixed text, so text_buf is not written.
            4 => return self.host(op::OPEN_PATCH, &[kind, number, count]) as i32 != -1,
            // SPEC GAP: a kind outside 0..3 writes the path with an empty file name part ("MISC\<nn>.BIN").
            _ => "",
        };
        let path = format!("MISC\\{}{:02}.BIN", name, number);
        let p = unsafe { (*self.w).text_buf };
        // SPEC GAP: a null text_buf is not written; text past 32 bytes is cut (never happens: at most 26 + 1).
        if !p.is_null() {
            for (i, b) in path.bytes().chain(std::iter::once(0)).take(TEXT_BUF).enumerate() {
                unsafe { *p.add(i) = b };
            }
        }
        self.host(op::OPEN_PATCH, &[kind, number, count]) as i32 != -1
    }
    fn move_code_put(&self, t: i32, v: u16) {
        let p = unsafe { (*self.w).move_codes };
        if !p.is_null() && (0..TILES).contains(&t) {
            let [lo, hi] = v.to_le_bytes();
            unsafe {
                *p.add(2 * t as usize) = lo;
                *p.add(2 * t as usize + 1) = hi;
            }
        }
    }
    fn move_code_get(&self, t: i32) -> u16 {
        let p = unsafe { (*self.w).move_codes } as *const u8;
        let i = 2 * t.max(0) as usize;
        if !(0..TILES).contains(&t) {
            return 0;
        }
        u16::from_le_bytes([Self::table(p, TILES as usize * 2, i), Self::table(p, TILES as usize * 2, i + 1)])
    }
    /// A 256-byte class table copied out of the world (null: all 0).
    fn class_table(p: *const u8) -> [u8; 256] {
        let mut a = [0u8; 256];
        for (i, e) in a.iter_mut().enumerate() {
            *e = Self::table(p, 256, i);
        }
        a
    }

    /// The move code of tile t rebuilt from the tile (combat.md "Movement", the per-tile rule: ground t0, object
    /// t1, roof byte 3, previous code = move_codes[t]).
    fn move_code_tile(&self, t: i32) {
        let (gc, oc) = unsafe { (Self::class_table((*self.w).t0_class), Self::class_table((*self.w).t1_class)) };
        let code = crate::move_code(
            self.tile_get(t, 0),
            self.tile_get(t, 1),
            self.tile_get(t, 3),
            self.move_code_get(t),
            &gc,
            &oc,
        );
        self.move_code_put(t, code);
    }

    // ---- Tile patch ----
    /// ApplyTileObjPatch 0x317EC.
    fn tile_patch(&self, kind: i32, number: i32, count: i32, bl: u8, cl: u8) -> i32 {
        // 1.
        if !self.open_patch(kind, number, count) {
            return -1;
        }
        // 2.
        let mut rec = [0u8; 4];
        let mut done = 0;
        // 3.
        for _ in 0..count.max(0) {
            // a.
            for (i, b) in self.read(2).into_iter().enumerate() {
                rec[i] = b;
            }
            for (i, b) in self.read(2).into_iter().enumerate() {
                rec[2 + i] = b;
            }
            // b.
            let obj = u16::from_le_bytes([rec[2], rec[3]]);
            if cl != 0 && (0x9E..=0xAA).contains(&(obj as i16)) {
                rec[2..4].copy_from_slice(&obj.wrapping_add(1).to_le_bytes());
            }
            // c.
            let t = i16::from_le_bytes([rec[0], rec[1]]) as i32;
            if t != 0 && t < 0xC40 {
                // SPEC GAP: a negative t (not in the game files) counts as done but writes nothing.
                self.tile_put(t, 1, rec[2]);
                self.move_code_tile(t);
                if bl != 0 {
                    self.tile_put(t, 3, 0x73);
                }
                done += 1;
            }
        }
        // 4.
        self.host(op::CLOSE, &[]);
        done
    }

    // ---- Item patch ----
    /// ApplyItemPatch 0x318C4.
    fn item_patch(&self, kind: i32, number: i32, count: i32, bl: u8, cl: u8) -> i32 {
        // 1.
        if !self.open_patch(kind, number, count) {
            return -1;
        }
        // 2.
        let mut rec = [0u8; 2];
        let mut done = 0;
        // 3.
        for _ in 0..count.max(0) {
            // a.
            for (i, b) in self.read(2).into_iter().enumerate() {
                rec[i] = b;
            }
            let got = self.read(12);
            let ib = unsafe { (*self.w).item_buf };
            // SPEC GAP: a null item_buf is not written and reads as all 0 (so every record is skipped).
            if !ib.is_null() {
                for (i, b) in got.into_iter().enumerate() {
                    unsafe { *ib.add(i) = b };
                }
            }
            let ib0 = if ib.is_null() { 0 } else { unsafe { *ib } };
            // b.
            let t = i16::from_le_bytes(rec) as i32;
            if t == 0 || t >= 0xC40 || ib0 == 0 {
                continue;
            }
            // c.
            if bl != 0 {
                self.take_ground_item(t, 0);
            } else {
                if cl != 0 && self.tile_g(t) != 0xFF {
                    let g = self.tile_g(t) as usize;
                    let mut copy = [0u8; 12];
                    for (i, e) in copy.iter_mut().enumerate() {
                        *e = self.gi(g, i);
                    }
                    self.put_item(t, copy.as_ptr(), copy[6], 0, 0, 0);
                    let g2 = self.tile_g(t) as usize;
                    self.gi_put(12 * g2, 0);
                }
                let ib6 = unsafe { *ib.add(6) };
                self.put_item(t, ib, ib6, 1, 1, 0);
            }
            // d.
            done += 1;
        }
        // 4.
        self.host(op::CLOSE, &[]);
        done
    }

    // ---- supply drops ----
    fn crate_byte(p: *mut u8, k: usize) -> u8 {
        Self::table(p as *const u8, 4, k)
    }
    fn crate_put(p: *mut u8, k: usize, v: u8) {
        // SPEC GAP: a null crate_state / crate_shown buffer is not written (and reads 0).
        Self::buf_put(p, 4, k as i64, v);
    }
    fn owned(&self, sec: i32) -> bool {
        // SPEC GAP: a sector outside 0..79 (or a null sector_state) is not owned.
        if !(0..SECTORS as i32).contains(&sec) {
            return false;
        }
        Self::table(unsafe { (*self.w).sector_state } as *const u8, SECTORS * 9, 9 * sec as usize) & 1 != 0
    }
    fn crate_lost(&self, k: usize) {
        let st = unsafe { (*self.w).crate_state };
        if Self::crate_byte(st, k) as i8 == 3 {
            return;
        }
        Self::crate_put(st, k, 3);
        let add: [u8; 4] = match k {
            0 => [10, 15, 10, 5],
            1 => [10, 25, 25, 0],
            2 => [10, 5, 5, 20],
            _ => [0; 4],
        };
        let sp = unsafe { (*self.w).supplies };
        for (i, a) in add.iter().enumerate() {
            if *a != 0 {
                let v = Self::table(sp as *const u8, 4, i).wrapping_add(*a);
                Self::buf_put(sp, 4, i as i64, v);
            }
        }
    }
    /// LoadCrateDrops 0x62360.
    fn crate_drops(&self) {
        for k in 0..4 {
            let cr = |i: usize| Self::table(unsafe { (*self.w).crates }, 16, 4 * k + i);
            let sec = cr(0) as i8 as i32;
            if sec != unsafe { (*self.w).cur_sector } as i32 {
                continue;
            }
            let dday = cr(1) as i8 as i32;
            let n = cr(2) as i8 as i32;
            let owned = self.owned(sec);
            let tiles = |b: u8, c: u8| {
                self.tile_patch(0, sec, n, b, c);
            };
            let items = |b: u8, c: u8| {
                self.item_patch(1, sec, n, b, c);
            };
            let st = || unsafe { (*self.w).crate_state };
            let sh = || unsafe { (*self.w).crate_shown };
            let day = unsafe { (*self.w).day };
            // 1.
            if dday == day {
                let s = Self::crate_byte(st(), k);
                if s == 0 {
                    if owned {
                        Self::crate_put(st(), k, 5);
                    } else {
                        tiles(1, 0);
                        items(0, 1);
                        Self::crate_put(st(), k, 1);
                        Self::crate_put(sh(), k, 0);
                    }
                    continue;
                }
                if s as i8 == 1 {
                    if owned {
                        Self::crate_put(st(), k, 2);
                    } else if cr(3) != 0 {
                        Self::crate_put(st(), k, 4);
                    } else {
                        tiles(0, 1);
                        items(1, 0);
                        self.crate_lost(k);
                        Self::crate_put(sh(), k, 1);
                    }
                    continue;
                }
            }
            // 2.
            if dday >= unsafe { (*self.w).day } {
                continue;
            }
            let s = Self::crate_byte(st(), k) as i8;
            let f = Self::crate_byte(sh(), k) as i8;
            match (s, f) {
                (4, -1) => {
                    tiles(1, 0);
                    items(0, 1);
                    Self::crate_put(sh(), k, 0);
                }
                (3, -1) => {
                    tiles(1, 1);
                    Self::crate_put(sh(), k, 1);
                }
                (3, 0) => {
                    tiles(0, 1);
                    items(1, 0);
                    Self::crate_put(sh(), k, 1);
                }
                _ => {}
            }
        }
    }

    // ---- plants ----
    /// The first plant record k = 1..3 whose u32 sector (+4) = cur_sector.
    fn plant_here(&self) -> Option<*mut u8> {
        let p = unsafe { (*self.w).plants };
        // SPEC GAP: a null plants buffer has no match.
        if p.is_null() {
            return None;
        }
        let cur = unsafe { (*self.w).cur_sector } as u32;
        (1..4).map(|k| unsafe { p.add(12 * k) }).find(|&r| {
            let sec = unsafe { u32::from_le_bytes([*r.add(4), *r.add(5), *r.add(6), *r.add(7)]) };
            sec == cur
        })
    }
    /// LoadPlantSector 0x62848.
    fn plant_sector(&self) {
        let Some(r) = self.plant_here() else { return };
        let cur = unsafe { (*self.w).cur_sector };
        let sec = unsafe { u32::from_le_bytes([*r.add(4), *r.add(5), *r.add(6), *r.add(7)]) } as i32;
        // 1.
        if unsafe { *r.add(8) } as i8 != 2 {
            return;
        }
        let nt = unsafe { *r.add(9) } as i8 as i32;
        let ni = unsafe { *r.add(10) } as i8 as i32;
        let stage = Self::table(unsafe { (*self.w).story }, 32, 5) as i8;
        if stage < 2 {
            // 2.
            self.tile_patch(2, sec, nt, 0, 0);
            self.item_patch(1, sec, ni, 1, 0);
            unsafe { *r.add(8) = 0 };
            self.host(op::REPLAY_EVENT, &[cur as i8 as i32, 0, 0x51]);
        } else {
            // 3.
            self.tile_patch(3, sec, nt, 0, 0);
            let owned = self.owned(cur as i32);
            self.item_patch(1, sec, ni, owned as u8, 0);
            if !owned {
                unsafe { *r.add(8) = 3 };
            }
        }
    }
    /// PlantDone 0x629AC.
    fn plant_done(&self) {
        let Some(r) = self.plant_here() else { return };
        if unsafe { *r.add(8) } as i8 == 3 {
            unsafe { *r.add(8) = 0 };
            let cur = unsafe { (*self.w).cur_sector };
            self.host(op::REPLAY_EVENT, &[cur as i8 as i32, 0, 0x51]);
        }
    }
}

// ================= Batch 7: fixed items, the sector tmp file, ground blast =================
impl Ctx {
    // ---- Fixed items ----
    fn fixed_items(&self) {
        for i in 0..FIXED {
            // SPEC GAP: a null fixed_items table reads as all 0 (item type 0: Put item does nothing).
            let f = |o: usize| Self::table(unsafe { (*self.w).fixed_items }, FIXED * 8, 8 * i + o);
            // 1. (room for 12 + 256 bytes: the count loop of Item reset can write past byte 11)
            let mut item = [0u8; 12 + 256];
            item[0] = f(2);
            item[1] = f(4);
            item[2] = f(6);
            // 2.
            self.item_reset(item.as_mut_ptr(), 0, 0);
            // 3.
            let t = i16::from_le_bytes([f(0), f(1)]) as i32;
            self.put_item(t, item.as_ptr(), 0, 0, 0, 0);
        }
    }

    // ---- Tmp file ----
    /// SectorTmpFile 0x2BCF0.
    fn tmp_file(&self) -> i32 {
        // 1.
        let cur = unsafe { (*self.w).cur_sector };
        if self.host(op::OPEN_TMP, &[cur as i32]) as i32 == -1 {
            return 0;
        }
        // 2.
        for t in 0..TILES {
            for o in [1usize, 2, 3, 4, 6] {
                // SPEC GAP: a short read (does not occur) leaves the tile byte as it is.
                if let Some(&b) = self.read(1).first() {
                    self.tile_put(t, o, b);
                }
            }
        }
        // 3.
        for (i, b) in self.read(GROUND * 12).into_iter().enumerate() {
            self.gi_put(i, b);
        }
        // 4.
        self.host(op::CLOSE, &[]);
        1
    }

    // ---- Ground blast ----
    fn ground_blast(&self, ty: u8, mut flag: u8, mut power: i32, thrower: u8, last: u8) {
        // 1.
        if power < 100 && ty == 0x11 {
            power = self.random(1000) + 500;
        }
        // 2.
        let mut more = false;
        for t in 0..TILES {
            // a.
            let g = self.tile_g(t);
            if g == 0xFF {
                continue;
            }
            let g = g as usize;
            if self.gi(g, 0) != ty {
                continue;
            }
            if ty == 0x11 {
                // b.
                self.take_ground_item(t, 0);
                let cur = unsafe { (*self.w).cur_sector } as usize;
                let sp = unsafe { (*self.w).sectors };
                let i = SECTOR_ROW * cur + 0xB0;
                // SPEC GAP: a cur_sector of 60 or more (or a null sectors table) reads 0 and is not written.
                let m = Self::table(sp as *const u8, SECTOR_ROWS * SECTOR_ROW, i).wrapping_sub(1);
                // The new count goes back to the row.
                Self::buf_put(sp, SECTOR_ROWS * SECTOR_ROW, i as i64, m);
                more = m != 0;
            } else {
                // c.
                let mut chain = false;
                if matches!(ty, 0x29 | 0x3F | 0x6E | 0x71) {
                    if ty == 0x6E {
                        self.gi_put(12 * g + 1, 1);
                    }
                    if ty == 0x6E || ty == 0x71 {
                        self.plant_done();
                    }
                    chain = ty != 0x6E;
                }
                let kind = self.gi(g, 1) as i8 as i32 + 1;
                shared::lib_call(self.w, self.h, op::EXPLODE, &[thrower as i32, t, kind, 1]);
                if ty == 0x3F {
                    unsafe { (*self.w).bomb_count = (*self.w).bomb_count.wrapping_add(1) };
                }
                if chain {
                    if unsafe { (*self.w).bomb_count } >= 3 && (flag as i8) < 1 {
                        flag = 1;
                    }
                    if ty == 0x29 {
                        flag = 0;
                    }
                    shared::lib_call(
                        self.w,
                        self.h,
                        op::EFFECT,
                        &[NONE as i32, ty as i32, flag as i32, power, thrower as i32, last as i8 as i32],
                    );
                }
            }
            // d.
            if !more {
                break;
            }
        }
    }
}

// ================= Batch 8: the whole sector start =================
impl Ctx {
    // ---- sectors row S and sector_state row T of cur_sector ----
    fn cur(&self) -> usize {
        unsafe { (*self.w).cur_sector as usize }
    }
    fn s_get(&self, o: usize) -> u8 {
        // SPEC GAP: a cur_sector of 60 or more (or a null sectors table) reads 0 and is not written.
        Self::table(unsafe { (*self.w).sectors } as *const u8, SECTOR_ROWS * SECTOR_ROW, SECTOR_ROW * self.cur() + o)
    }
    fn s_put(&self, o: usize, v: u8) {
        let i = SECTOR_ROW * self.cur() + o;
        Self::buf_put(unsafe { (*self.w).sectors }, SECTOR_ROWS * SECTOR_ROW, i as i64, v);
    }
    fn s_dec(&self, o: usize) {
        self.s_put(o, self.s_get(o).wrapping_sub(1));
    }
    fn t_get(&self, o: usize) -> u8 {
        Self::table(unsafe { (*self.w).sector_state } as *const u8, SECTORS * 9, 9 * self.cur() + o)
    }
    /// Fill `len` bytes of a world buffer with v (null: nothing).
    fn fill(p: *mut u8, len: usize, v: u8) {
        for i in 0..len {
            Self::buf_put(p, len, i as i64, v);
        }
    }

    // ---- Setup sector ----
    /// SetupSectorFromXra 0x2E918.
    fn setup_sector(&self, tile: i32, facing: u8) {
        let w = self.w;
        // 1.
        self.host(op::SECTOR_RESETS, &[]);
        unsafe {
            (*w).turn_based = 0;
            // 0x2E941: dword 0x92DB0 = 0 with 0x92DAC (the fast-turn timer). A sector entered while an enemies' turn
            // timer still ran (fight21, fight22, f1) kept it, and the timer later set turn-based mode by itself.
            (*w).fast_timer = 0;
            (*w).bomb_count = 0;
            (*w).show_ai = 0;
            (*w).santino_timer = 0;
            (*w).anim_signal = -1;
            (*w).mission_slot = 0x40;
            (*w).last_spotter = 0x40;
            (*w).active_slot = 0x40;
            // calls: u16 +0 and +4 = NO_TILE, +2 and +6 = 0 (+3 and +7 stay).
            let calls = (*w).calls;
            let [lo, hi] = NONE.to_le_bytes();
            for o in [0, 4] {
                Self::buf_put(calls, 8, o, lo);
                Self::buf_put(calls, 8, o + 1, hi);
                Self::buf_put(calls, 8, o + 2, 0);
            }
            Self::fill((*w).knowledge, SLOTS, 0);
            Self::fill((*w).remember_e, 320, 0);
            Self::fill((*w).remember_m, 128, 0);
            let bl = (*w).blast_list;
            for i in 0..BLAST {
                let [lo, hi] = NONE.to_le_bytes();
                Self::buf_put(bl, BLAST * 2, 2 * i as i64, lo);
                Self::buf_put(bl, BLAST * 2, 2 * i as i64 + 1, hi);
            }
            (*w).interrupt_go = 0;
            (*w).turn_moves = 0;
            (*w).quote_focus = 0;
            (*w).quote_delay = 0;
            Self::fill((*w).ap_warned, 8, 0);
            (*w).next_selected = 0x40;
            (*w).quote_slot = 0x40;
            (*w).spotted = 0x40;
            // Turns (turns.md fields).
            Self::fill((*w).itq, 50, 0x40);
            (*w).itq_len = 0;
            Self::fill((*w).bled, 48, 0);
            let fx = (*w).timed_fx;
            for e in 0..5 {
                for o in 0..4 {
                    Self::buf_put(fx, 65, (13 * e + o) as i64, 0xFF);
                }
            }
            (*w).new_round = 1;
            (*w).turn_timer = 0;
            (*w).turn_timer2 = 0;
            (*w).shown_slot = 0x40;
            (*w).view_tile = NONE as i32;
            (*w).focus_slot = 0x40;
            (*w).view_hold = 0;
            (*w).interrupt_turn = 0;
            // Update end fields (soldier_update.md).
            (*w).ambient_count = 0xF8;
            // New turn fields (turns.md, New turn); speech_wait is the one random(3) of the resets.
            (*w).turn_clock = 20000;
            (*w).show_timer = 0;
            (*w).handed_over = 0;
            (*w).interrupter = 0x40;
        }
        let sw = (self.random(3) + 1) as u8;
        unsafe {
            (*w).speech_wait = sw;
            // Shooting fields (shooting.md).
            (*w).in_flight = 0;
            (*w).replay_m = 0;
            (*w).marked_slot = 0x40;
            // Blast fields (combat.md Blast, exact steps).
            (*w).blaster = 0x40;
            let g = (*w).gas_clouds;
            if !g.is_null() {
                for k in 0..5 {
                    g.add(6 * k).write(NONE as u8);
                    g.add(6 * k + 1).write((NONE >> 8) as u8);
                }
            }
            (*w).alarm = self.t_get(0) & 1;
        }
        // 2.
        for k in 0..SLOTS {
            self.slot_reset(k, k as i32);
        }
        // 3.
        let tmp = self.tmp_file();
        self.rebuild_move_codes();
        // 4.
        if self.t_get(0) & 1 == 0 {
            // a.
            self.open_patch(4, 0, 0);
            self.host(op::SEEK, &[(self.cur() as i32).wrapping_mul(XRA_RECORD)]);
            for want in [2i8, 1] {
                for k in 0x28..0x30 {
                    let got = self.read(REC);
                    let dst = if self.s_get(0x6E + k) as i8 == want {
                        self.rec(k)
                    } else {
                        // SPEC GAP: a null scratch buffer drops the template.
                        unsafe { (*w).scratch }
                    };
                    if !dst.is_null() {
                        for (i, b) in got.into_iter().enumerate() {
                            unsafe { *dst.add(i) = b };
                        }
                    }
                    // SPEC GAP: the clearing of items 7 and 8 is read as done to record k whether or not the
                    // template went there.
                    if want == 2 {
                        for o in 0x54..0x6C {
                            self.p8(k, o, 0);
                        }
                    }
                    self.p8(k, 0x182, 0);
                    self.p8(k, 0x17A, k as u8);
                }
            }
            // b.
            if tmp == 0 {
                for t in (0..=0xC3F).rev() {
                    // SPEC GAP: a short read leaves the tile byte as it is.
                    if let Some(&b) = self.read(1).first() {
                        self.tile_put(t, 6, b);
                    }
                }
                for i in 0..XRA_ITEMS {
                    for (j, b) in self.read(12).into_iter().enumerate() {
                        self.gi_put(12 * i + j, b);
                    }
                    let e = |o: usize| self.gi(i, o);
                    let ep = |o: usize, v: u8| self.gi_put(12 * i + o, v);
                    if e(3) == 0 {
                        ep(5, 0);
                        ep(4, 0);
                    }
                    if e(0) != 0 && self.item_type(e(0), 6) > 1 && e(1) as i8 > 1 {
                        let mut j: i32 = 0;
                        // SPEC GAP: j can reach past byte 11 of the entry (s8 E[1] above 5); those writes go to the
                        // next entry (and past the last one, are dropped), as written.
                        while j < e(1) as i8 as i32 - 1 {
                            let o = 8 + j as usize;
                            let base = e(2) as i8 as i32;
                            let mut u = e(o);
                            if base < 0 {
                                u = (self.random(10) - 5 + e(2) as i32) as u8;
                                if u as i8 > -1 {
                                    u = 0xFF;
                                }
                                if u as i8 <= -100 {
                                    u = e(2);
                                }
                            } else {
                                if base < 100 {
                                    u = (self.random(10) - 5 + e(2) as i32) as u8;
                                }
                                if u as i8 > 100 {
                                    u = 100;
                                }
                                if u as i8 <= 0 {
                                    u = e(2);
                                }
                            }
                            ep(o, u);
                            j += 1;
                        }
                    }
                }
                for t in 0..=0xC3F {
                    let g = self.tile_g(t);
                    if g != 0xFF && self.note_placed(self.gi(g as usize, 0)) {
                        self.take_ground_item(t, 0);
                    }
                }
            }
            // c.
            self.host(op::CLOSE, &[]);
            // d.
            if unsafe { (*w).purifier } != 0 && self.cur() != 0x3B {
                self.s_put(0x9E, 0x79);
                unsafe { (*w).purifier = 0 };
            }
            // e.
            let mut given = false;
            for i in 0..8 {
                let k = 0x28 + i;
                if self.s_get(0x96 + i) as i8 > 0 {
                    self.xra_enemy(k);
                    self.equip(k, (self.s_get(0x96 + i) == 2) as u8);
                    let it = self.s_get(0x9E);
                    if it != 0 && !given {
                        self.p8(k, 0x6C, it);
                        let q = self.item_type(it, 5);
                        self.p8(k, 0x6D, if (q as i8) < 99 { q } else { 99 });
                        self.p8(k, 0x6E, 100);
                        for o in 0x6F..0x73 {
                            self.p8(k, o, 0);
                        }
                        given = true;
                    }
                }
            }
        }
        // 5.
        self.crate_drops();
        self.plant_sector();
        // 6.
        let m = self.radio_op() as u8 as i8;
        unsafe { (*w).mission_step = m };
        // 7.
        for k in 0..5 {
            // a.
            let t = i16::from_le_bytes([self.s_get(0x82 + 2 * k), self.s_get(0x83 + 2 * k)]) as i32;
            let b = self.s_get(0x9F + k);
            if t != NONE as i32 && b != 0 {
                let story = unsafe { (*w).story };
                if (b == 0xAC || b == 0xAD) && Self::table(story, 32, 11) == 2 {
                    self.tile_put(t, 1, 0);
                    self.s_put(0x9F + k, 0);
                    let [lo, hi] = NONE.to_le_bytes();
                    self.s_put(0x82 + 2 * k, lo);
                    self.s_put(0x83 + 2 * k, hi);
                    Self::buf_put(story, 32, 0x17, 0xFF);
                } else {
                    self.tile_put(t, 1, b);
                }
                self.move_code_tile(t);
            }
            // b.
            let t = i16::from_le_bytes([self.s_get(0x8C + 2 * k), self.s_get(0x8D + 2 * k)]) as i32;
            if t != NONE as i32 {
                let ib = unsafe { (*w).item_buf };
                // Item reset can write past byte 11: it works on a copy with room, then bytes 0..11 go back.
                // SPEC GAP: a null item_buf uses that copy only (bytes 1..11 start as 0).
                let mut item = [0u8; 12 + 256];
                if !ib.is_null() {
                    for (i, e) in item.iter_mut().take(12).enumerate() {
                        *e = unsafe { *ib.add(i) };
                    }
                }
                item[0] = self.s_get(0xA4 + k);
                self.item_reset(item.as_mut_ptr(), 1, 2);
                if item[0] == 0x86 {
                    item[3] = 0x69;
                    item[4] = 99;
                    item[5] = 0x64;
                }
                let src = if ib.is_null() {
                    item.as_ptr()
                } else {
                    for (i, e) in item.iter().take(12).enumerate() {
                        unsafe { *ib.add(i) = *e };
                    }
                    ib as *const u8
                };
                let b6 = if item[0] == 0x5E { 0xFF } else { 0 };
                self.put_item(t, src, b6, 1, 1, 0);
                self.s_put(0xA4 + k, 0);
                let [lo, hi] = NONE.to_le_bytes();
                self.s_put(0x8C + 2 * k, lo);
                self.s_put(0x8D + 2 * k, hi);
            }
        }
        // 8.
        while self.s_get(0xAE) != 0 && self.s_get(0xAD) != 0 {
            self.s_dec(0xAE);
            self.s_dec(0xAD);
        }
        // 9.
        if self.s_get(0xB0) != 0 {
            self.ground_blast(0x11, 0, 0, 0x40, 0);
        }
        // 10.
        for t in 0..=0xC3F {
            let b = self.tile_get(t, 1);
            if b != 0x35 && b != 0x36 {
                continue;
            }
            if self.s_get(0xAC) != 0 {
                self.tile_put(t, 1, 0x34);
                // SPEC GAP: for t + 0x38 past the map (no tree lies there) the byte is not written.
                self.tile_put(t + 0x38, 4, self.tile_get(t + 0x38, 4) & 0xB7);
                self.s_dec(0xAC);
                if b == 0x36 {
                    self.s_dec(0xAD);
                }
            } else if b == 0x35 {
                if self.s_get(0xAE) != 0 {
                    self.tile_put(t, 1, 0x36);
                    self.s_dec(0xAE);
                }
            } else if self.s_get(0xAD) != 0 {
                self.tile_put(t, 1, 0x35);
                self.s_dec(0xAD);
            }
        }
        // 11.
        if unsafe { (*w).team_on_duty } != 0 {
            self.merc_entry(tile, facing);
        }
        // 12.
        // (replaces the former host op 94 keep_selected, which is no longer called).
        unsafe { (*w).merc_to_show = (*w).selected };
        // 13.
        let t3 = self.t_get(3) as i32;
        let groups = (t3 + 2) / 3;
        let (mut done, mut done3) = (0i32, 0i32);
        let mut slot = 0x10usize;
        for t in (0..=0xC3F).rev() {
            // TileIsObj36? 0x236D0.
            if self.tile_get(t, 1) != 0x36 || done >= groups {
                continue;
            }
            let r = self.random(4) as u8 as i32;
            let mut d = 1;
            for j in 0..(t3 - done3).min(3) {
                if j == r {
                    d += 2;
                }
                let c = self.step_dir(t, d);
                // SPEC GAP: a slot of 48 or more (more than 32 tappers) is not set up.
                if slot < SLOTS {
                    self.init_tapper(slot, c, ((d + 3) % 8 + 1) as u8);
                }
                slot += 1;
                d += 2;
            }
            done3 += 3;
            done += 1;
        }
        // 14.
        let gd = self.t_get(4) as i32;
        let per = if gd == 0 || groups == 0 {
            0
        } else if gd < groups {
            1
        } else {
            gd / groups
        }
        .min(3);
        let mut slot = 8usize;
        let mut placed = 0i32;
        let place_guard = |slot: usize, c: i32, f: u8, tr: u8, anchor: i32| {
            // SPEC GAP: a slot of 48 or more is not set up.
            if slot < SLOTS {
                self.init_guard(slot, c, f, tr, anchor);
            }
        };
        // a.
        if per != 0 {
            for t in (0..=0xC3F).rev() {
                if self.tile_get(t, 1) != 0x36 {
                    continue;
                }
                for m in 0..per as usize {
                    let d = self.random(3) + 2;
                    let c = self.guard_tile(t, GUARD_DIRS[m], d);
                    let f = (self.random(8) + 1) as u8;
                    place_guard(slot, c, f, 1, t);
                    slot += 1;
                    placed += 1;
                }
                if placed >= groups * per {
                    break;
                }
            }
        }
        // b.
        // SPEC GAP: "m < 8 here"; GUARD_DIRS is not indexed past 8 (the loop stops there).
        for m in 0..(gd - placed).clamp(0, 8) as usize {
            let d = self.random(11) + 5;
            let c = self.guard_tile(0x63C, GUARD_DIRS[m], d);
            let f = (self.random(8) + 1) as u8;
            place_guard(slot, c, f, 3, NONE as i32);
            slot += 1;
        }
        // 15.
        for k in 0..SLOTS {
            self.soldier_start(k);
        }
        // 16.
        unsafe {
            (*w).busy = 0;
            (*w).fx_id = 0xFF;
            (*w).ripple_timer = 250;
            // 0x2F6C3: word 0x22A478 = 0 (enemies seen). The field lives in the port world, so op 95's write of the
            // same global on the host side does not reach it; a fight dump (fight59) kept the old count here.
            (*w).seen_enemies = 0;
        }
        if unsafe { (*w).team_on_duty } != 0 {
            let sel = unsafe { (*w).selected } as usize;
            // SPEC GAP: a selected slot of 48 or more is not written.
            if sel < SLOTS {
                self.p8(sel, 0x1BD, 0x6E);
            }
        }
        self.host(op::SECTOR_TAIL, &[]);
        // 17.
        let row3b = Self::table(unsafe { (*w).sectors } as *const u8, SECTOR_ROWS * SECTOR_ROW, SECTOR_ROW * 0x3B + 0xA9);
        if self.cur() == 0x3B && row3b == 0 {
            self.fixed_items();
        }
        // 18.
        if self.t_get(2) != 0 {
            self.s_put(0xA9, 1);
        }
    }

    // ---- Entering a sector (world_load.md section 4, `ja1_enter_sector`) ----
    fn enter_sector(&self, entry_tile: i32) {
        let w = self.w;
        // 0.
        unsafe {
            (*w).kept_merc = 0x40;
            // shooting.md batch 4.
            (*w).last_killer = 0x40;
            // soldier_update.md Update end.
            (*w).ambient_t1 = 200;
            (*w).ambient_t2 = 300;
            // soldier_update.md Loop head.
            (*w).turn_tick = 10;
            // turns.md The day clock, Start of the day.
            (*w).day_minutes = 0;
            (*w).day_bar = 0;
            (*w).weather_tire = 0;
            (*w).redraw_next = 0;
            (*w).night_warned = 0;
            (*w).day_hours = 0;
            (*w).clock_minute = 0;
            (*w).clock_hour = 7;
        }
        // 1.
        for k in 0..MERCS {
            let v = (self.g8(k, 0x17E) == 0) as u8;
            self.p8(k, 0x138, v);
        }
        // 2.
        let sel = (0..MERCS)
            .find(|&k| self.g8(k, 0x182) != 0 && matches!(self.g8(k, 0x17E), 0 | 8) && self.g8(k, 0x183) != 0)
            .unwrap_or(MERCS);
        unsafe { (*w).selected = sel as u8 };
        // 3.
        unsafe { (*w).idle_day = (sel >= MERCS) as u8 };
        if sel < MERCS && self.t_get(2) != 0 {
            // SPEC GAP: "replay_event(cur, cur, 0x58)" does not say how cur widens to int; batch 6 passes s8 cur,
            // here cur goes in zero-extended (the plain value; the two differ only for cur_sector >= 128).
            let cur = unsafe { (*w).cur_sector } as i32;
            self.host(op::REPLAY_EVENT, &[cur, cur, 0x58]);
        }
        // 3, then: Time compression (0) (turns.md).
        crate::ai::clock::time_compress(w, self.h, 0);
        // 4.
        self.setup_sector(entry_tile, 1);
        // 5. (state byte 2 read again now; the host may have changed cur_sector or sector_state)
        unsafe {
            if self.t_get(2) != 0 {
                let sel = (*w).selected as usize;
                // SPEC GAP: a selected slot of 48 or more names no record; the view is left as it is.
                if sel >= SLOTS {
                    return;
                }
                let t = self.g16(sel, 0x114) as i16 as i32;
                let px = (t % 56) * 16;
                let py = (t / 56) * 14;
                let on = (*w).opt_onscreen;
                let (vx, vy, vw, vh) = ((*w).view_x, (*w).view_y, (*w).view_w, (*w).view_h);
                let outside = px < vx || px >= vx.wrapping_add(vw) || py < vy || py >= vy.wrapping_add(vh);
                if on != 0 && outside {
                    let m = if on < 2 { 0x70 } else { 0xA0 };
                    (*w).view_x = px - m + 16;
                    let mut y = py.wrapping_sub(vh / 2).wrapping_add(14);
                    while y % 14 != 0 {
                        y = y.wrapping_sub(1);
                    }
                    (*w).view_y = y;
                    // JumpViewToTile? 0x3F490 ends in ClampView 0x24830 (loadsector_diff).
                    (*w).view_x = (*w).view_x.max(0).min(0x380 - vw);
                    (*w).view_y = (*w).view_y.max(0).min(0x310 - vh);
                }
            } else {
                (*w).view_x = 336;
                (*w).view_y = 308;
            }
        }
    }
}

// ================= Batch 9: replace and reserve AP =================
/// Blast kind of an item type y (sector_start.md batch 9).
/// Put item (batch 3) for the library's other procedures (shooting.md batch 4 Kill); `item` = 12 readable bytes.
pub(crate) fn put_item_at(w: *mut Ja1World, h: *const Ja1Host, tile: i32, item: *const u8, b6: u8, exact: u8, note: u8, near: u8) {
    if item.is_null() {
        return;
    }
    if let Some(c) = Ctx::new(w, h) {
        c.put_item(tile, item, b6, exact, note, near);
    }
}
/// Listed (batch 4) for the library's other procedures.
pub(crate) fn listed(w: *mut Ja1World, h: *const Ja1Host, ty: u8) -> bool {
    Ctx::new(w, h).map_or(false, |c| c.listed(ty))
}

pub(crate) fn blast_kind(y: u8) -> u8 {
    if y <= 0x15 {
        0
    } else if y <= 0x1B {
        y - 0x12
    } else if y <= 0x1D {
        2
    } else if y == 0x5D {
        1
    } else {
        3
    }
}

impl Ctx {
    /// Bit `opc` (0..62) of host_ops is set: the host claims the op.
    fn host_claims(&self, opc: i32) -> bool {
        unsafe { (*self.h).host_ops & (1u64 << opc) != 0 }
    }
    /// Op 51 replace on record s (library op, port_api.md): the host call when host_ops claims it, else
    /// Replace soldier.
    fn replace_op(&self, s: usize) {
        if self.host_claims(op::REPLACE) {
            self.host(op::REPLACE, &[s as i32]);
        } else {
            self.replace_soldier(s);
        }
    }
    /// Op 58 reserve_ap on slot s (library op): the host call when host_ops claims it, else Reserve AP.
    fn reserve_op(&self, s: usize) {
        // SPEC GAP: a slot of 48 or more (Merc entry's `selected`) names no record; as for every library op
        // (ai.rs dispatch), the op then goes to the host.
        if self.host_claims(op::RESERVE_AP) || s >= SLOTS {
            self.host(op::RESERVE_AP, &[s as i32]);
        } else {
            self.reserve_ap(s);
        }
    }

    /// "Replace soldier" (`ja1_replace_soldier`).
    fn replace_soldier(&self, s: usize) {
        let t = s16(self.g16(s, 0x114) as i32);
        // 1. SPEC GAP: the spec says t is a map tile here; a tile outside 0..3135 reads its bytes as 0.
        self.p16(s, 0xF4, t as u16);
        self.p8(s, 0x1B2, self.tile_get(t, 0));
        self.p8(s, 0x1B4, self.tile_get(t, 1));
        // 2.
        self.p8(s, 0x1F3, 0);
        // 3.
        let (b2, b4) = (self.g8(s, 0x1B2), self.g8(s, 0x1B4));
        let code: u16 = if b2 >= 0x6C && b4 != 0x1C && b4 != 0x1F {
            if b2 < 0x97 {
                0x33
            } else if b2 < 0x9A {
                0x36
            } else {
                0x39
            }
        } else {
            4
        };
        self.p16(s, 0x1E8, code);
        // 4.
        self.p32(s, 0x90, (self.g16(s, 0x1E8) as u32).wrapping_mul(0x50));
        // 5. Reads the old ground class +0x1B3.
        shared::frame_delay(self.w, self.h, s);
        // 6.
        self.p8(s, 0x138, 1);
        // 7.
        let (t0c, t1c, t2c) = unsafe { ((*self.w).t0_class, (*self.w).t1_class, (*self.w).t2_class) };
        self.p8(s, 0x1B2, self.tile_get(t, 0));
        self.p8(s, 0x1B3, Self::table(t0c, 256, self.g8(s, 0x1B2) as usize));
        self.p8(s, 0x1B4, self.tile_get(t, 1));
        self.p8(s, 0x1B5, Self::table(t1c, 256, self.g8(s, 0x1B4) as usize));
        self.p8(s, 0x1B6, self.tile_get(t, 2));
        self.p8(s, 0x1B7, Self::table(t2c, 256, self.g8(s, 0x1B6) as usize));
    }

    /// "Reserve AP" (`ja1_reserve_ap`): op 17 action_ap is a library op from batch 12 on.
    fn reserve_ap(&self, s: usize) {
        // 1.
        let k = blast_kind(self.g8(s, 0x00));
        if matches!(k, 3 | 5 | 7 | 9) {
            self.p8(s, 0xEF, 6);
            return;
        }
        // 2.
        let y = self.g8(s, 0x00);
        if k == 0 && (y == 0 || y > 0xE) {
            self.p8(s, 0xEF, 0);
            return;
        }
        // 3.
        let ap = if self.host_claims(op::ACTION_AP) {
            self.host(op::ACTION_AP, &[s as i32, NONE as i32, 1]) as u8
        } else {
            shared::action_ap(self.w, self.h, s, NONE as i32, 1) as u8
        };
        self.p8(s, 0xEF, ap);
        // 4.
        if (self.g8(s, 0x1A5) as i8) > (ap as i8) && self.g8(s, 0x16A) != 0 {
            let v = ap.wrapping_add(4);
            self.p8(s, 0xEF, v);
            if (v as i8) > (self.g8(s, 0x1A5) as i8) {
                self.p8(s, 0xEF, self.g8(s, 0x1A5));
            }
        }
    }
}

// ================= Batch 10: radio result =================
impl Ctx {
    /// Op 33 radio_result (library op): the host call when host_ops claims it, else Radio result.
    fn radio_op(&self) -> u32 {
        if self.host_claims(op::RADIO_RESULT) {
            self.host(op::RADIO_RESULT, &[])
        } else {
            self.radio_result() as u32
        }
    }
    fn set_mission_target(&self, t: u16) {
        unsafe { (*self.w).mission_target = t };
    }

    /// "Radio result" (`ja1_radio_result`): the new mission step 0..4.
    fn radio_result(&self) -> i32 {
        let cur = self.cur();
        // 1.
        self.set_mission_target(NONE);
        // SPEC GAP: a null tile buffer skips the loop (mission_target stays 0xC41).
        let tiles_ok = unsafe { !(*self.w).tiles.is_null() };
        for t in (0..0xC40).filter(|_| tiles_ok) {
            let g = self.tile_get(t, 6);
            if g < 0xFF && self.gi(g as usize, 0) == 0x72 {
                self.set_mission_target(t as u16);
                break;
            }
        }
        // 2.
        let story = |k: usize| Self::table(unsafe { (*self.w).story }, 32, k);
        // SPEC GAP: "cur = s8 story[0x17]" compares the unsigned cur_sector byte with the signed byte, so a
        // story[0x17] of 0x80..0xFF never matches.
        if story(0x0B) as i8 == 1 && cur as i32 == story(0x17) as i8 as i32 && story(0x16) == 0 && story(0x15) == 0 {
            for k in 0..5 {
                if self.s_get(0x9F + k) == 0xAE {
                    self.set_mission_target(u16::from_le_bytes([self.s_get(0x82 + 2 * k), self.s_get(0x83 + 2 * k)]));
                    return 1;
                }
            }
        }
        // 3.
        if cur == 0x0B {
            return if self.quest_op(0x1C9) as u8 != 0 { 3 } else { 0 };
        }
        // 4. (plant 0 is not read)
        if let Some(r) = self.plant_here() {
            return if unsafe { *r.add(8) } as i8 == 3 { 2 } else { 0 };
        }
        // 5. SPEC GAP: a cur_sector of 80 or more (or a null mission_done) reads 0.
        if Self::table(unsafe { (*self.w).mission_done }, SECTORS, cur) != 0 {
            return 0;
        }
        // 6.
        if unsafe { (*self.w).mission_target } as i16 != NONE as i16 {
            return 4;
        }
        // 7. SPEC GAP: a null soldiers buffer skips this step.
        if self.soldiers_ok() {
            for k in 0x28..0x30 {
                if self.g8(k, 0x182) != 0
                    && self.g8(k, 0x183) != 0
                    && self.g8(k, 0x188) == 0
                    && (self.g8(k, 0x0C) == 0x40 || self.g8(k, 0x00) == 0x40)
                {
                    return 4;
                }
            }
        }
        // 8.
        0
    }
}

// ================= Batch 11: quest check =================
impl Ctx {
    /// Op 44 quest_check (library op): the host call when host_ops claims it, else Quest check.
    fn quest_op(&self, quest: i32) -> u32 {
        if self.host_claims(op::QUEST_CHECK) {
            self.host(op::QUEST_CHECK, &[quest])
        } else {
            self.quest_check(quest) as u32
        }
    }

    /// Op 19 mission_cancel (library op): MissionCancel 0x72670 (read in the asm). The enemy that can reach the
    /// mission target and has the lowest path AP to the mission spot takes the mission order: mission_slot = it,
    /// its +0x1C1 = 1 and +0xB9 = 1. No change when the mission target is "no tile" or no enemy qualifies.
    fn mission_cancel(&self) {
        let (target, spot) = unsafe { ((*self.w).mission_target, (*self.w).mission_spot) };
        if target == NONE || !self.soldiers_ok() {
            return;
        }
        let (mut best, mut best_ap) = (0x40u8, 1000i16);
        for k in 0x28..0x30usize {
            // In the sector, in play, s8 hp >= 15, +0x188 = 0, s8 energy (+0x18A) >= 35, s8 +0x1D5 <= 0.
            if self.g8(k, 0x182) == 0
                || self.g8(k, 0x138) == 0
                || (self.g8(k, 0x183) as i8) < 15
                || self.g8(k, 0x188) != 0
                || (self.g8(k, 0x18A) as i8) < 0x23
                || self.g8(k, 0x1D5) as i8 > 0
            {
                continue;
            }
            let b9 = self.g8(k, 0xB9);
            self.p8(k, 0xB9, 1);
            if shared::lib_call(self.w, self.h, op::REACH_AP, &[k as i32, target as i16 as i32, 0]) == 0 {
                self.p8(k, 0xB9, b9);
                continue;
            }
            let here = self.g16(k, 0x114) == spot;
            let ap = if here {
                0
            } else {
                shared::lib_call(self.w, self.h, op::PATH_AP, &[k as i32, spot as i16 as i32]) as u16 as i16
            };
            self.p8(k, 0xB9, b9);
            if (ap != 0 || here) && ap < best_ap {
                best_ap = ap;
                best = k as u8;
            }
        }
        if best < 0x40 {
            unsafe { (*self.w).mission_slot = best };
            self.p8(best as usize, 0x1C1, 1);
            self.p8(best as usize, 0xB9, 1);
        }
    }

    /// "Quest check" (`ja1_quest_check`): 1 = the condition holds, else 0. No host call, no write.
    fn quest_check(&self, quest: i32) -> i32 {
        let story = |k: usize| Self::table(unsafe { (*self.w).story }, 32, k);
        // 1.
        if self.cur() != 0x0B
            || quest as i16 != 0x1C9
            || story(0x0E) as i8 != 1
            || unsafe { (*self.w).safe_opened } != 0
            || story(0x1C) != 0
            || story(0x1E) != 0
        {
            return 0;
        }
        // 2. SPEC GAP: a null soldiers buffer counts as no merc carrying either type.
        let carries = |y: u8| {
            self.soldiers_ok()
                && (0..MERCS).any(|k| self.g8(k, 0x182) != 0 && (0..12).any(|i| self.g8(k, 12 * i) == y))
        };
        if carries(0x6E) || carries(0x7C) {
            return 0;
        }
        // 3.
        1
    }
}

fn guard(f: impl FnOnce()) {
    let _ = catch_unwind(AssertUnwindSafe(f));
}

#[no_mangle]
pub extern "C" fn ja1_slot_reset(w: *mut Ja1World, h: *const Ja1Host, record: i32, slot: i32) {
    guard(|| {
        // SPEC GAP: a record outside 0..47 or a null soldiers buffer does nothing.
        if let Some(c) = Ctx::new(w, h) {
            if c.soldiers_ok() && (0..SLOTS as i32).contains(&record) {
                c.slot_reset(record as usize, slot);
            }
        }
    })
}

#[no_mangle]
pub extern "C" fn ja1_item_init(w: *mut Ja1World, h: *const Ja1Host, item: *mut u8, cond_mode: i32, count_mode: i32) {
    guard(|| {
        if item.is_null() {
            return;
        }
        if let Some(c) = Ctx::new(w, h) {
            c.item_reset(item, cond_mode as u8, count_mode as u8);
        }
    })
}

#[no_mangle]
pub extern "C" fn ja1_equip(w: *mut Ja1World, h: *const Ja1Host, slot: i32, flag: i32) {
    guard(|| {
        // SPEC GAP: a slot outside 0..47 or a null soldiers buffer does nothing.
        if let Some(c) = Ctx::new(w, h) {
            if c.soldiers_ok() && (0..SLOTS as i32).contains(&slot) {
                c.equip(slot as usize, flag as u8);
            }
        }
    })
}

/// Batch 2 entry check: world, host, soldier buffer and (when given) a slot in 0..47.
fn with_slot(w: *mut Ja1World, h: *const Ja1Host, slot: i32, f: impl FnOnce(&Ctx, usize)) {
    guard(|| {
        // SPEC GAP: a slot outside 0..47 or a null soldiers buffer does nothing.
        if let Some(c) = Ctx::new(w, h) {
            if c.soldiers_ok() && (0..SLOTS as i32).contains(&slot) {
                f(&c, slot as usize);
            }
        }
    })
}

#[no_mangle]
pub extern "C" fn ja1_xra_enemy(w: *mut Ja1World, h: *const Ja1Host, slot: i32) {
    with_slot(w, h, slot, |c, s| c.xra_enemy(s))
}

#[no_mangle]
pub extern "C" fn ja1_init_tapper(w: *mut Ja1World, h: *const Ja1Host, slot: i32, tile: i32, facing: i32) {
    with_slot(w, h, slot, |c, s| c.init_tapper(s, tile, facing as u8))
}

#[no_mangle]
pub extern "C" fn ja1_init_guard(
    w: *mut Ja1World,
    h: *const Ja1Host,
    slot: i32,
    tile: i32,
    facing: i32,
    orders: i32,
    anchor: i32,
) {
    with_slot(w, h, slot, |c, s| c.init_guard(s, tile, facing as u8, orders as u8, anchor))
}

#[no_mangle]
pub extern "C" fn ja1_guard_tile(w: *mut Ja1World, h: *const Ja1Host, tile: i32, dir: i32, dist: i32) -> i32 {
    let mut out = tile;
    guard(|| {
        // SPEC GAP: a null world or host returns tile unchanged.
        if let Some(c) = Ctx::new(w, h) {
            out = c.guard_tile(tile, dir as u8, dist);
        }
    });
    out
}

#[no_mangle]
pub extern "C" fn ja1_place_merc(w: *mut Ja1World, h: *const Ja1Host, slot: i32, tile: i32, facing: i32) {
    with_slot(w, h, slot, |c, s| c.place_merc(s, tile, facing as u8))
}

#[no_mangle]
pub extern "C" fn ja1_merc_entry(w: *mut Ja1World, h: *const Ja1Host, tile: i32, facing: i32) {
    guard(|| {
        // SPEC GAP: a null soldiers buffer does nothing.
        if let Some(c) = Ctx::new(w, h) {
            if c.soldiers_ok() {
                c.merc_entry(tile, facing as u8);
            }
        }
    })
}

#[no_mangle]
pub extern "C" fn ja1_put_item(
    w: *mut Ja1World,
    h: *const Ja1Host,
    tile: i32,
    item: *const u8,
    b6: i32,
    exact: i32,
    note: i32,
    near: i32,
) {
    guard(|| {
        // SPEC GAP: a null item pointer does nothing.
        if item.is_null() {
            return;
        }
        if let Some(c) = Ctx::new(w, h) {
            c.put_item(tile, item, b6 as u8, exact as u8, note as u8, near as u8);
        }
    })
}

#[no_mangle]
pub extern "C" fn ja1_take_ground_item(w: *mut Ja1World, h: *const Ja1Host, tile: i32, keep: i32) {
    guard(|| {
        if let Some(c) = Ctx::new(w, h) {
            c.take_ground_item(tile, keep as u8);
        }
    })
}

#[no_mangle]
pub extern "C" fn ja1_take_ground_patch(w: *mut Ja1World, h: *const Ja1Host, tile: i32, ty: i32) {
    guard(|| {
        if let Some(c) = Ctx::new(w, h) {
            c.take_ground_patch(tile, ty as u8, 0);
        }
    })
}

#[no_mangle]
pub extern "C" fn ja1_soldier_start(w: *mut Ja1World, h: *const Ja1Host, record: i32) {
    with_slot(w, h, record, |c, s| c.soldier_start(s))
}

#[no_mangle]
pub extern "C" fn ja1_tile_patch(w: *mut Ja1World, h: *const Ja1Host, kind: i32, number: i32, count: i32, bl: i32, cl: i32) -> i32 {
    let mut out = 0;
    guard(|| {
        // SPEC GAP: a null world or host returns 0 (no host call).
        if let Some(c) = Ctx::new(w, h) {
            out = c.tile_patch(kind, number, count, bl as u8, cl as u8);
        }
    });
    out
}

#[no_mangle]
pub extern "C" fn ja1_item_patch(w: *mut Ja1World, h: *const Ja1Host, kind: i32, number: i32, count: i32, bl: i32, cl: i32) -> i32 {
    let mut out = 0;
    guard(|| {
        if let Some(c) = Ctx::new(w, h) {
            out = c.item_patch(kind, number, count, bl as u8, cl as u8);
        }
    });
    out
}

#[no_mangle]
pub extern "C" fn ja1_crate_lost(w: *mut Ja1World, h: *const Ja1Host, drop: i32) {
    guard(|| {
        // SPEC GAP: a drop outside 0..3 does nothing.
        if let Some(c) = Ctx::new(w, h) {
            if (0..4).contains(&drop) {
                c.crate_lost(drop as usize);
            }
        }
    })
}

#[no_mangle]
pub extern "C" fn ja1_crate_drops(w: *mut Ja1World, h: *const Ja1Host) {
    guard(|| {
        if let Some(c) = Ctx::new(w, h) {
            c.crate_drops();
        }
    })
}

#[no_mangle]
pub extern "C" fn ja1_plant_sector(w: *mut Ja1World, h: *const Ja1Host) {
    guard(|| {
        if let Some(c) = Ctx::new(w, h) {
            c.plant_sector();
        }
    })
}

#[no_mangle]
pub extern "C" fn ja1_plant_done(w: *mut Ja1World, h: *const Ja1Host) {
    guard(|| {
        if let Some(c) = Ctx::new(w, h) {
            c.plant_done();
        }
    })
}

#[no_mangle]
pub extern "C" fn ja1_fixed_items(w: *mut Ja1World, h: *const Ja1Host) {
    guard(|| {
        if let Some(c) = Ctx::new(w, h) {
            c.fixed_items();
        }
    })
}

#[no_mangle]
pub extern "C" fn ja1_tmp_file(w: *mut Ja1World, h: *const Ja1Host) -> i32 {
    let mut out = 0;
    guard(|| {
        // SPEC GAP: a null world or host returns 0 (no host call).
        if let Some(c) = Ctx::new(w, h) {
            out = c.tmp_file();
        }
    });
    out
}

#[no_mangle]
pub extern "C" fn ja1_ground_blast(
    w: *mut Ja1World,
    h: *const Ja1Host,
    ty: i32,
    flag: i32,
    power: i32,
    thrower: i32,
    last: i32,
) {
    guard(|| {
        if let Some(c) = Ctx::new(w, h) {
            c.ground_blast(ty as u8, flag as u8, power, thrower as u8, last as u8);
        }
    })
}

#[no_mangle]
pub extern "C" fn ja1_setup_sector(w: *mut Ja1World, h: *const Ja1Host, tile: i32, facing: i32) {
    guard(|| {
        // SPEC GAP: a null soldiers buffer does nothing.
        if let Some(c) = Ctx::new(w, h) {
            if c.soldiers_ok() {
                c.setup_sector(tile, facing as u8);
            }
        }
    })
}

/// world_load.md section 4 "ja1_enter_sector (library)": the host calls of batch 8 plus op 89 replay_event.
#[no_mangle]
pub extern "C" fn ja1_enter_sector(w: *mut Ja1World, h: *const Ja1Host, entry_tile: i32) {
    guard(|| {
        // SPEC GAP: a null soldiers buffer does nothing.
        if let Some(c) = Ctx::new(w, h) {
            if c.soldiers_ok() {
                c.enter_sector(entry_tile);
            }
        }
    })
}

#[no_mangle]
pub extern "C" fn ja1_replace_soldier(w: *mut Ja1World, h: *const Ja1Host, slot: i32) {
    with_slot(w, h, slot, |c, s| c.replace_soldier(s))
}

#[no_mangle]
pub extern "C" fn ja1_reserve_ap(w: *mut Ja1World, h: *const Ja1Host, slot: i32) {
    with_slot(w, h, slot, |c, s| c.reserve_ap(s))
}

/// Op 19 mission_cancel (MissionCancel 0x72670), for the diff tests.
#[no_mangle]
pub extern "C" fn ja1_mission_cancel(w: *mut Ja1World, h: *const Ja1Host) {
    guard(|| {
        if let Some(c) = Ctx::new(w, h) {
            c.mission_cancel();
        }
    });
}

#[no_mangle]
pub extern "C" fn ja1_radio_result(w: *mut Ja1World, h: *const Ja1Host) -> i32 {
    let mut out = 0;
    guard(|| {
        // SPEC GAP: a null world or host returns 0 (no host call).
        if let Some(c) = Ctx::new(w, h) {
            out = c.radio_result();
        }
    });
    out
}

#[no_mangle]
pub extern "C" fn ja1_quest_check(w: *mut Ja1World, h: *const Ja1Host, quest: i32) -> i32 {
    let mut out = 0;
    guard(|| {
        // SPEC GAP: a null world or host returns 0.
        if let Some(c) = Ctx::new(w, h) {
            out = c.quest_check(quest);
        }
    });
    out
}

// ================= Walk-ins (strategic.md "Walk-ins") =================

/// The 7 gate tiles of edge 9 (sector 0).
const GATE_TILES: [i32; 7] = [0x240, 0x241, 0x242, 0x278, 0x2B0, 0x2B1, 0x2B2];

impl Ctx {
    /// Edge tile: a free tile on the map edge, or -1.
    fn edge_tile(&self, edge: i32) -> i32 {
        let d = (edge as u8 as i8 as i32).wrapping_sub(1) as u8;
        // SPEC GAP: an edge other than 1, 3, 5, 7, 9 gives -1 at once (no random number).
        if d > 8 || d % 2 != 0 {
            return -1;
        }
        let mut deep = 0;
        for _ in 0..1000 {
            let t = match d {
                0 => 0xC08 + self.random(56),
                2 => 56 * self.random(56),
                4 => self.random(56),
                6 => 56 * self.random(56) + 55,
                _ => GATE_TILES.get(self.random(7) as usize).copied().unwrap_or(-1),
            };
            if self.standable(t) > 0 {
                if self.tile_get(t, 0) < 0x6C || deep >= 10 {
                    if self.occupant(t) >= 0x40 {
                        return t;
                    }
                } else {
                    deep += 1;
                }
            }
        }
        -1
    }

    fn clear_record(&self, s: usize) {
        unsafe { std::ptr::write_bytes(self.rec(s), 0, REC) };
    }

    /// Guard enters (op 312): 1 = a hired guard came in at the edge.
    fn guard_enter(&self, edge: i32) -> u8 {
        let Some(k) = (8..16).find(|&k| self.g8(k, 0x182) == 0) else {
            return 0;
        };
        let t = self.edge_tile(edge);
        if t == -1 {
            return 0;
        }
        self.clear_record(k);
        self.slot_reset(k, k as i32);
        let f = (self.random(8) + 1) as u8;
        self.init_guard(k, t, f, 4, NONE as i32);
        self.soldier_start(k);
        self.p8(k, 0x1AB, 1);
        1
    }

    /// Enemy walks in (op 329): 1 = the enemy of slot s came in at the edge.
    fn enemy_walk_in(&self, s: usize, edge: i32) -> u8 {
        let e = edge as u8;
        let t = self.edge_tile(e as i32);
        if t == -1 {
            return 0;
        }
        // 1.
        self.clear_record(s);
        self.slot_reset(s, s as i32);
        self.p8(s, 0x1E0, self.random(4) as u8);
        // 2.
        let f = if e == 9 { (self.random(8) + 1) as u8 } else { e };
        self.p8(s, 0x1A1, f);
        self.p8(s, 0xBB, 2);
        for o in [0x124, 0x114, 0x12E] {
            self.p16(s, o, t as u16);
        }
        self.p8(s, 0x182, 1);
        self.p8(s, 0x17B, (self.random(6) + 0x11) as u8);
        self.p8(s, 0x17C, (self.random(8) + 2) as u8);
        let r = self.random(6) as u8;
        // 3.
        self.p8(s, 0x1D4, 4);
        self.p8(s, 0x17E, 0);
        self.p8(s, 0x1F3, 0);
        self.p16(s, 0x1E8, 4);
        self.p16(s, 0x134, NONE);
        for o in [0x186, 0x188, 0x189, 0x19C] {
            self.p8(s, o, 0);
        }
        self.p16(s, 0x12A, NONE);
        self.p8(s, 0x185, 0);
        self.p8(s, 0x1BD, 0x6D);
        self.p8(s, 0x9C, 0xFF);
        self.p8(s, 0x1D3, r);
        self.p8(s, 0x1A2, self.g8(s, 0x1A1));
        // 4.
        let day = unsafe { (*self.w).day };
        let n = if day > 50 { 500 } else { 1000i32.wrapping_sub(day.wrapping_sub(1).wrapping_mul(10)) };
        let r = self.random(n as u16 as i16 as i32);
        let mut i = 8i32;
        while i >= 0 && i * i * i > r {
            i -= 1;
        }
        self.p8(s, 0x192, (9 - i) as u8);
        if unsafe { (*self.w).cur_sector } == 0 {
            if (self.g8(s, 0x192) as i8) < 6 {
                self.p8(s, 0x192, 6);
            }
        } else {
            let q = day.wrapping_add(2) / 3;
            if (self.g8(s, 0x192) as i8 as i32) > (q as u8 as i32) {
                self.p8(s, 0x192, q as u8);
            }
        }
        // 5.
        for o in [0x183, 0x18B, 0x18C, 0x18D, 0x18E, 0x18F, 0x190, 0x191] {
            let mut v = self.g8(s, 0x192).wrapping_mul(5).wrapping_add(0x23);
            v = v.wrapping_add(self.random(11) as u8);
            v = v.wrapping_add(self.random(10) as u8);
            self.p8(s, o, v);
        }
        // 6.
        self.p16(s, 0x10C, 0);
        self.p8(s, 0x18A, 100);
        self.p8(s, 0x184, self.g8(s, 0x183));
        let u = ((self.g8(s, 0x192) as i8 as i32) / 2).wrapping_add(self.random(2)) as u8;
        self.p8(s, 0x1B4, u.min(4));
        // 7.
        self.equip(s, 1);
        self.occupy(t, self.g8(s, 0x17A));
        self.replace_op(s);
        self.soldier_start(s);
        1
    }
}

pub(crate) fn guard_enter(w: *mut Ja1World, h: *const Ja1Host, edge: i32) -> u8 {
    match Ctx::new(w, h) {
        Some(c) if c.soldiers_ok() => c.guard_enter(edge),
        _ => 0,
    }
}

pub(crate) fn enemy_walk_in(w: *mut Ja1World, h: *const Ja1Host, slot: i32, edge: i32) -> u8 {
    let s = slot as u8 as usize;
    match Ctx::new(w, h) {
        // SPEC GAP: a slot of 48 or more names no record: result 0.
        Some(c) if c.soldiers_ok() && s < SLOTS => c.enemy_walk_in(s, edge),
        _ => 0,
    }
}

#[no_mangle]
pub extern "C" fn ja1_edge_tile(w: *mut Ja1World, h: *const Ja1Host, edge: i32) -> i32 {
    let mut out = -1;
    guard(|| {
        if let Some(c) = Ctx::new(w, h) {
            out = c.edge_tile(edge);
        }
    });
    out
}

#[no_mangle]
pub extern "C" fn ja1_guard_enter(w: *mut Ja1World, h: *const Ja1Host, edge: i32) -> i32 {
    let mut out = 0;
    guard(|| out = guard_enter(w, h, edge) as i32);
    out
}

#[no_mangle]
pub extern "C" fn ja1_enemy_walk_in(w: *mut Ja1World, h: *const Ja1Host, slot: i32, edge: i32) -> i32 {
    let mut out = 0;
    guard(|| out = enemy_walk_in(w, h, slot, edge) as i32);
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::cell::RefCell;
    use std::os::raw::c_void;

    /// Port safety rule of `tile_put` (its SPEC GAP; ledger row 1938): a write to a tile outside 0..3135 is dropped.
    /// The original has no range test there: it writes into the globals beside g_tileMap 0x100A18, which the port
    /// does not hold.
    #[test]
    fn tile_put_off_map_is_dropped() {
        let mut buf = vec![0u8; (TILES as usize + 2) * 8];
        let mut w: Ja1World = unsafe { std::mem::zeroed() };
        w.tiles = buf.as_mut_ptr();
        let c = Ctx { w: &mut w, h: std::ptr::null() };
        c.tile_put(TILES - 1, 6, 7);
        c.tile_put(TILES, 6, 9);
        assert_eq!((buf[(TILES as usize - 1) * 8 + 6], buf[TILES as usize * 8 + 6]), (7, 0));
    }

    struct Rng {
        vals: Vec<i32>,
        pos: usize,
        log: Vec<i32>,
        calls: Vec<(i32, Vec<i32>)>,
        /// patch file bytes served by op 87 read, and the op 86 open_patch / 92 open_tmp result
        stream: Vec<u8>,
        spos: usize,
        open: i32,
    }
    fn rng(vals: Vec<i32>) -> RefCell<Rng> {
        RefCell::new(Rng { vals, pos: 0, log: vec![], calls: vec![], stream: vec![], spos: 0, open: 0 })
    }
    unsafe extern "C" fn call(ctx: *mut c_void, op: i32, args: *const i32, n: i32, o: *mut u8, os: i32) -> u32 {
        let r = &*(ctx as *const RefCell<Rng>);
        let mut r = r.borrow_mut();
        let args = std::slice::from_raw_parts(args, n as usize).to_vec();
        r.calls.push((op, args.clone()));
        match op {
            86 | 92 => r.open as u32,
            17 => 0x103, // action_ap: low byte 3
            44 => r.open as u32, // quest_check: the batch 10 test sets it through `open`

            87 => {
                let want = (args[0] as usize).min(os as usize);
                let got = want.min(r.stream.len() - r.spos);
                for i in 0..got {
                    *o.add(i) = r.stream[r.spos + i];
                }
                r.spos += got;
                got as u32
            }
            _ => 0,
        }
    }
    /// The batch 2..8 tests keep ops 17, 33, 44, 51 and 58 as host calls (host_ops claims them).
    const HOST_CLAIMED: u64 = 1 << op::ACTION_AP | 1 << op::RADIO_RESULT | 1 << op::QUEST_CHECK | 1 << op::REPLACE | 1 << op::RESERVE_AP;
    fn lib_host(r: &RefCell<Rng>) -> Ja1Host {
        Ja1Host { ctx: r as *const _ as *mut c_void, random: Some(rnd), call: Some(call), host_ops: 0 }
    }
    fn host(r: &RefCell<Rng>) -> Ja1Host {
        Ja1Host { ctx: r as *const _ as *mut c_void, random: Some(rnd), call: Some(call), host_ops: HOST_CLAIMED }
    }
    unsafe extern "C" fn rnd(ctx: *mut c_void, n: i32) -> i32 {
        let r = &*(ctx as *const RefCell<Rng>);
        let mut r = r.borrow_mut();
        r.log.push(n);
        let v = r.vals.get(r.pos).copied().unwrap_or(0);
        r.pos += 1;
        if n <= 0 { 0 } else { v.rem_euclid(n) }
    }

    fn world(soldiers: &mut [u8], item_types: &[u8]) -> Ja1World {
        let mut w: Ja1World = unsafe { std::mem::zeroed() };
        w.soldiers = soldiers.as_mut_ptr();
        w.item_types = item_types.as_ptr();
        w
    }

    #[test]
    fn slot_reset_fields() {
        let mut sold = vec![0xAAu8; SLOTS * REC];
        let it = vec![0u8; ITEM_TYPES * 12];
        let mut w = world(&mut sold, &it);
        let rng = rng(vec![]);
        let h = Ja1Host { ctx: &rng as *const _ as *mut c_void, random: Some(rnd), call: None, host_ops: 0 };
        ja1_slot_reset(&mut w, &h, 3, 10);
        let s = &sold[3 * REC..4 * REC];
        assert_eq!(s[0x17A], 10);
        assert_eq!(s[0x180], 1);
        assert_eq!(&s[0x124..0x126], &[0x41, 0x0C]);
        assert_eq!(&s[0xE6..0xE8], &[0x41, 0x0C]);
        assert_eq!(s[0x1B1], 0);
        assert_eq!(sold[10 * REC + 0x17E], 1);
        assert_eq!(sold[10 * REC + 0x182], 0);
        assert!(rng.borrow().log.is_empty());
    }

    #[test]
    fn item_reset_stack() {
        let mut sold = vec![0u8; SLOTS * REC];
        let mut it = vec![0u8; ITEM_TYPES * 12];
        it[12 * 7 + 6] = 5; // class 5, not a counted class
        let mut w = world(&mut sold, &it);
        let rng = rng(vec![5, 6, 7, 8]);
        let h = Ja1Host { ctx: &rng as *const _ as *mut c_void, random: Some(rnd), call: None, host_ops: 0 };
        let mut e = [7u8, 9, 0, 1, 1, 1, 0, 0, 0, 0, 0, 0];
        ja1_item_init(&mut w, &h, e.as_mut_ptr(), 1, 0);
        // count capped to 5; condition + 4 unit conditions drawn
        assert_eq!(e[1], 5);
        assert_eq!(&e[2..6], &[75, 0, 0, 0]);
        assert_eq!(&e[8..12], &[76, 77, 78, 70]);
        assert_eq!(rng.borrow().log, vec![31; 5]);
    }

    /// Ledger row 0b4d529:ja1logic:19223 (op 346, sector_start.md "Item reset" on ground entry g: entries are 12
    /// bytes): the reset works on entry g at byte 12 g and leaves the other entries alone.
    #[test]
    fn op_346_resets_ground_entry_g() {
        let mut sold = vec![0u8; SLOTS * REC];
        let mut it = vec![0u8; ITEM_TYPES * 12];
        it[12 * 7 + 6] = 5;
        let mut w = world(&mut sold, &it);
        let rng = rng(vec![5, 6, 7, 8]);
        let h = Ja1Host { ctx: &rng as *const _ as *mut c_void, random: Some(rnd), call: None, host_ops: 0 };
        let mut ground = vec![0xAAu8; 200 * 12];
        for g in 0..200 {
            ground[12 * g] = 7;
        }
        w.ground_items = ground.as_mut_ptr();
        let before = ground.clone();
        crate::ai::usetile::ground_item_reset(&mut w, &h, 3);
        for (i, (&a, &b)) in before.iter().zip(&ground).enumerate() {
            assert_eq!(b, if (39..=41).contains(&i) { 0 } else { a }, "byte {i}");
        }
        assert_eq!(&ground[36..48], &[7, 0xAA, 0xAA, 0, 0, 0, 0xAA, 0xAA, 0xAA, 0xAA, 0xAA, 0xAA]);
    }

    // ---- batch 2 ----
    struct Map {
        tiles: Vec<u8>,
        dirs: Vec<u8>,
        t0c: Vec<u8>,
        t1c: Vec<u8>,
    }
    fn map() -> Map {
        let mut tiles = vec![0u8; 3136 * 8];
        for t in 0..3136 {
            tiles[t * 8 + 5] = 0x40;
        }
        let mut dirs = vec![0u8; 257 * 2];
        for (i, k) in [-56i16, -55, 1, 57, 56, 55, -1, -57].iter().enumerate() {
            dirs[(i + 1) * 2..(i + 1) * 2 + 2].copy_from_slice(&k.to_le_bytes());
        }
        Map { tiles, dirs, t0c: vec![0u8; 256], t1c: vec![0u8; 256] }
    }
    fn world2(sold: &mut [u8], it: &[u8], m: &mut Map) -> Ja1World {
        let mut w = world(sold, it);
        w.tiles = m.tiles.as_mut_ptr();
        w.dir_steps = m.dirs.as_ptr();
        w.t0_class = m.t0c.as_ptr();
        w.t1_class = m.t1c.as_ptr();
        w
    }
    fn get16(sold: &[u8], r: usize, o: usize) -> u16 {
        u16::from_le_bytes([sold[r * REC + o], sold[r * REC + o + 1]])
    }

    // ---- batch 9 ----
    #[test]
    fn replace_soldier_water_tile() {
        let mut sold = vec![0u8; SLOTS * REC];
        let it = vec![0u8; ITEM_TYPES * 12];
        let mut m = map();
        let t2 = {
            let mut v = vec![0u8; 256];
            v[7] = 0x22;
            v
        };
        let s = 6 * REC;
        sold[s + 0x114..s + 0x116].copy_from_slice(&100u16.to_le_bytes());
        sold[s + 0x183] = 100;
        sold[s + 0x18A] = 10;
        sold[s + 0x1F3] = 3;
        m.tiles[100 * 8..100 * 8 + 3].copy_from_slice(&[0x98, 0x05, 0x07]);
        m.t0c[0x98] = 9;
        m.t1c[5] = 3;
        let mut w = world2(&mut sold, &it, &mut m);
        w.t2_class = t2.as_ptr();
        let r = rng(vec![]);
        let h = lib_host(&r);
        ja1_replace_soldier(&mut w, &h, 6);
        assert_eq!(get16(&sold, 6, 0xF4), 100);
        assert_eq!(sold[s + 0x1F3], 0);
        assert_eq!(get16(&sold, 6, 0x1E8), 0x36);
        assert_eq!(&sold[s + 0x90..s + 0x94], &(0x36u32 * 0x50).to_le_bytes());
        assert_eq!(get16(&sold, 6, 0xD2), 100); // Frame delay step 2: 0 * 10 + 10 * 10
        assert_eq!(sold[s + 0x138], 1);
        assert_eq!(&sold[s + 0x1B2..s + 0x1B8], &[0x98, 9, 0x05, 3, 0x07, 0x22]);
        assert!(r.borrow().calls.is_empty() && r.borrow().log.is_empty());
        // Byte 1 = 0x1C is not water: code 4.
        m.tiles[100 * 8 + 1] = 0x1C;
        let mut w = world2(&mut sold, &it, &mut m);
        ja1_replace_soldier(&mut w, &h, 6);
        assert_eq!(get16(&sold, 6, 0x1E8), 4);
    }

    #[test]
    fn reserve_ap_kinds_and_bonus() {
        let mut sold = vec![0u8; SLOTS * REC];
        let it = vec![0u8; ITEM_TYPES * 12];
        let mut m = map();
        let s = 2 * REC;
        let r = rng(vec![]);
        // Op 17 stays a host call here (batch 12 tests the library Action AP).
        let h = Ja1Host { host_ops: 1 << op::ACTION_AP, ..lib_host(&r) };
        let mut run = |sold: &mut Vec<u8>, ty: u8, ap_left: u8, b16a: u8| {
            sold[s] = ty;
            sold[s + 0x1A5] = ap_left;
            sold[s + 0x16A] = b16a;
            sold[s + 0xEF] = 0xAA;
            let mut w = world2(sold, &it, &mut m);
            ja1_reserve_ap(&mut w, &h, 2);
            sold[s + 0xEF]
        };
        assert_eq!(run(&mut sold, 0x17, 0, 0), 6); // kind 5
        assert_eq!(run(&mut sold, 0x60, 0, 0), 6); // kind 3
        assert_eq!(run(&mut sold, 0x0F, 0, 0), 0);
        assert_eq!(run(&mut sold, 0x00, 0, 0), 0);
        assert!(r.borrow().calls.is_empty());
        assert_eq!(run(&mut sold, 0x05, 10, 0), 3);
        assert_eq!(run(&mut sold, 0x05, 10, 1), 7);
        assert_eq!(run(&mut sold, 0x05, 5, 1), 5);
        assert_eq!(run(&mut sold, 0x16, 0x80, 1), 3); // kind 4; s8 apLeft -128 not above 3
        assert_eq!(r.borrow().calls, vec![(17, vec![2, 0xC41, 1]); 4]);
    }

    // ---- batch 10 ----
    #[test]
    fn radio_result_steps() {
        let mut sold = vec![0u8; SLOTS * REC];
        let it = vec![0u8; ITEM_TYPES * 12];
        let mut m = map();
        let mut ground = vec![0u8; GROUND * 12];
        let mut sectors = vec![0u8; SECTOR_ROWS * SECTOR_ROW];
        let mut story = vec![0u8; 32];
        let mut plants = vec![0u8; 48];
        let mut done = vec![0u8; SECTORS];
        for t in 0..3136 {
            m.tiles[t * 8 + 6] = 0xFF;
        }
        let r = rng(vec![]);
        // Op 44 stays a host call here (batch 11 tests the library Quest check).
        let h = Ja1Host { host_ops: 1 << op::QUEST_CHECK, ..lib_host(&r) };
        macro_rules! run {
            ($cur:expr) => {{
                let mut w = world2(&mut sold, &it, &mut m);
                w.ground_items = ground.as_mut_ptr();
                w.sectors = sectors.as_mut_ptr();
                w.story = story.as_mut_ptr();
                w.plants = plants.as_mut_ptr();
                w.mission_done = done.as_mut_ptr();
                w.cur_sector = $cur;
                w.mission_target = 0;
                let v = ja1_radio_result(&mut w, &h);
                (v, w.mission_target)
            }};
        }
        // 8. Nothing: 0, mission_target 0xC41.
        assert_eq!(run!(5), (0, 0xC41));
        // 7. A living enemy with the radio in item 1.
        sold[0x29 * REC + 0x182] = 1;
        sold[0x29 * REC + 0x183] = 9;
        sold[0x29 * REC + 0x0C] = 0x40;
        assert_eq!(run!(5).0, 4);
        sold[0x29 * REC + 0x188] = 1;
        assert_eq!(run!(5).0, 0);
        // 1 and 6. The first tile whose ground entry is type 0x72.
        m.tiles[400 * 8 + 6] = 2;
        m.tiles[500 * 8 + 6] = 3;
        m.tiles[600 * 8 + 6] = 3;
        ground[3 * 12] = 0x72;
        assert_eq!(run!(5), (4, 500));
        // 5. Mission done.
        done[5] = 1;
        assert_eq!(run!(5), (0, 500));
        // 4. Plants 1..3 only (plant 0 is not read).
        plants[4] = 5;
        plants[8] = 3;
        plants[24 + 4] = 5;
        plants[24 + 8] = 2;
        assert_eq!(run!(5).0, 0);
        plants[24 + 8] = 3;
        assert_eq!(run!(5).0, 2);
        // 3. Sector 0x0B asks quest 0x1C9.
        r.borrow_mut().open = 1;
        assert_eq!(run!(0x0B).0, 3);
        r.borrow_mut().open = 0;
        assert_eq!(run!(0x0B).0, 0);
        assert_eq!(r.borrow().calls, vec![(44, vec![0x1C9]); 2]);
        // 2. Story slot: the first 0xAE entry of row cur.
        story[0x0B] = 1;
        story[0x17] = 5;
        let row = SECTOR_ROW * 5;
        sectors[row + 0x9F + 1] = 0xAE;
        sectors[row + 0x82 + 2..row + 0x82 + 4].copy_from_slice(&0x123u16.to_le_bytes());
        sectors[row + 0x9F + 3] = 0xAE;
        assert_eq!(run!(5), (1, 0x123));
        story[0x15] = 1;
        assert_eq!(run!(5).0, 2);
    }

    // ---- batch 11 ----
    #[test]
    fn quest_check_conditions() {
        let mut sold = vec![0u8; SLOTS * REC];
        let it = vec![0u8; ITEM_TYPES * 12];
        let mut story = vec![0u8; 32];
        story[0x0E] = 1;
        let r = rng(vec![]);
        let h = lib_host(&r);
        macro_rules! q {
            ($cur:expr, $quest:expr, $safe:expr) => {{
                let mut w = world(&mut sold, &it);
                w.story = story.as_mut_ptr();
                w.cur_sector = $cur;
                w.safe_opened = $safe;
                ja1_quest_check(&mut w, &h, $quest)
            }};
        }
        assert_eq!(q!(0x0B, 0x1C9, 0), 1);
        assert_eq!(q!(0x0B, 0x7FFF_01C9, 0), 1); // low 16 bits only
        assert_eq!(q!(0x0B, 0x1CA, 0), 0);
        assert_eq!(q!(0x0C, 0x1C9, 0), 0);
        assert_eq!(q!(0x0B, 0x1C9, 1), 0);
        story[0x1E] = 1;
        assert_eq!(q!(0x0B, 0x1C9, 0), 0);
        story[0x1E] = 0;
        story[0x0E] = 2;
        assert_eq!(q!(0x0B, 0x1C9, 0), 0);
        story[0x0E] = 1;
        // A merc carrying 0x7C in item 11 (+0x84): 0; an absent merc (+0x182 = 0) does not count.
        sold[3 * REC + 0x84] = 0x7C;
        assert_eq!(q!(0x0B, 0x1C9, 0), 1);
        sold[3 * REC + 0x182] = 1;
        assert_eq!(q!(0x0B, 0x1C9, 0), 0);
        sold[3 * REC + 0x84] = 0x6E;
        assert_eq!(q!(0x0B, 0x1C9, 0), 0);
        // Slot 8 is not a merc.
        sold[3 * REC + 0x84] = 0;
        sold[8 * REC + 0x182] = 1;
        sold[8 * REC] = 0x6E;
        assert_eq!(q!(0x0B, 0x1C9, 0), 1);
        assert!(r.borrow().calls.is_empty());
    }

    #[test]
    fn radio_result_runs_library_quest_check() {
        let mut sold = vec![0u8; SLOTS * REC];
        let it = vec![0u8; ITEM_TYPES * 12];
        let mut m = map();
        let mut story = vec![0u8; 32];
        story[0x0E] = 1;
        let r = rng(vec![]);
        let h = lib_host(&r);
        let mut w = world2(&mut sold, &it, &mut m);
        w.story = story.as_mut_ptr();
        w.cur_sector = 0x0B;
        assert_eq!(ja1_radio_result(&mut w, &h), 3);
        w.safe_opened = 1;
        assert_eq!(ja1_radio_result(&mut w, &h), 0);
        assert!(r.borrow().calls.is_empty());
    }

    #[test]
    fn setup_sector_runs_library_radio_result() {
        // Op 33 unclaimed: no op 33 host call (the rest of batch 8 as in setup_sector_call_order).
        let mut sold = vec![0u8; SLOTS * REC];
        let it = vec![0u8; ITEM_TYPES * 12];
        let mut m = map();
        let mut w = world2(&mut sold, &it, &mut m);
        let r = rng(vec![]);
        let h = lib_host(&r);
        ja1_setup_sector(&mut w, &h, 30 * 56 + 30, 1);
        assert!(r.borrow().calls.iter().all(|c| c.0 != 33));
        assert_eq!(w.mission_step, 0);
    }

    #[test]
    fn merc_entry_library_ops_make_no_host_call() {
        let mut sold = vec![0u8; SLOTS * REC];
        let it = vec![0u8; ITEM_TYPES * 12];
        let mut m = map();
        for k in 0..8 {
            sold[k * REC + 0x17A] = k as u8;
            sold[k * REC + 0x138] = 1;
        }
        for k in 0..2 {
            sold[k * REC + 0x182] = 1;
            sold[k * REC + 0x183] = 50;
        }
        let mut w = world2(&mut sold, &it, &mut m);
        w.selected = 0;
        let r = rng(vec![]);
        let h = lib_host(&r);
        let tile = 30 * 56 + 30;
        ja1_merc_entry(&mut w, &h, tile, 1);
        assert!(r.borrow().calls.iter().all(|c| c.0 != 51 && c.0 != 58));
        // Replace soldier ran on both mercs.
        assert_eq!(get16(&sold, 0, 0xF4), (tile - 56) as u16);
        assert_eq!(get16(&sold, 1, 0x1E8), 4);
    }

    #[test]
    fn merc_entry_slot_past_the_soldiers_does_not_go_to_the_host() {
        let mut sold = vec![0u8; SLOTS * REC];
        let it = vec![0u8; ITEM_TYPES * 12];
        let mut m = map();
        let mut w = world2(&mut sold, &it, &mut m);
        w.selected = 0x40;
        let r = rng(vec![]);
        let h = lib_host(&r);
        ja1_merc_entry(&mut w, &h, 30 * 56 + 30, 1);
        assert!(r.borrow().calls.iter().all(|c| c.0 != 58));
    }

    #[test]
    fn guard_tile_steps_and_turns() {
        let mut sold = vec![0u8; SLOTS * REC];
        let it = vec![0u8; ITEM_TYPES * 12];
        let mut m = map();
        let start = 28 * 56 + 28;
        m.tiles[(start + 2) * 8 + 5] = 3; // occupied 2 steps east
        let mut w = world2(&mut sold, &it, &mut m);
        let r = rng(vec![]);
        let h = host(&r);
        // East is taken, so the next direction (SE) gives start + 2 × 57.
        assert_eq!(ja1_guard_tile(&mut w, &h, start as i32, 3, 2), start as i32 + 114);
        assert_eq!(ja1_guard_tile(&mut w, &h, start as i32, 3, 28), start as i32);
        // Water class 3 on every tile in reach of dist 1 pushes dist to 2.
        m.t0c[0] = 3;
        m.tiles[(start + 2) * 8 + 5] = 0x40;
        let mut w = world2(&mut sold, &it, &mut m);
        assert_eq!(ja1_guard_tile(&mut w, &h, start as i32, 1, 1), start as i32);
        assert!(r.borrow().log.is_empty() && r.borrow().calls.is_empty());
    }

    #[test]
    fn place_merc_fields_and_call() {
        let mut sold = vec![0u8; SLOTS * REC];
        let it = vec![0u8; ITEM_TYPES * 12];
        let mut m = map();
        sold[5 * REC + 0x17A] = 5;
        sold[5 * REC + 0x18E] = 0x90; // s8 -112: not above +0x191
        sold[5 * REC + 0x19C] = 6;
        let mut w = world2(&mut sold, &it, &mut m);
        let r = rng(vec![3]);
        let h = host(&r);
        ja1_place_merc(&mut w, &h, 5, 0x10123, 0x107);
        let s = &sold[5 * REC..6 * REC];
        assert_eq!(get16(&sold, 5, 0x114), 0x123);
        assert_eq!(get16(&sold, 5, 0x124), 0x123);
        assert_eq!((s[0x1E0], s[0x9C], s[0x1A1], s[0x1A2], s[0x1BD], s[0x138]), (5, 1, 7, 7, 0x6A, 1));
        assert_eq!((s[0x1D4], s[0x1D3]), (4, 3));
        assert_eq!(m.tiles[0x123 * 8 + 5], 5);
        assert_eq!(m.tiles[0x123 * 8 + 4], 0x80);
        assert_eq!(r.borrow().log, vec![5]);
        assert_eq!(r.borrow().calls, vec![(51, vec![5])]);
    }

    #[test]
    fn guard_draw_order() {
        let mut sold = vec![0u8; SLOTS * REC];
        let it = vec![0u8; ITEM_TYPES * 12];
        let mut m = map();
        sold[9 * REC + 0x17A] = 9;
        let mut w = world2(&mut sold, &it, &mut m);
        w.day = 10;
        let r = rng(vec![]);
        let h = host(&r);
        let tile = 20 * 56 + 3;
        ja1_init_guard(&mut w, &h, 9, tile, 2, 0x103, 0x456);
        let s = &sold[9 * REC..10 * REC];
        let mut want = vec![8, 4, 6, 15];
        want.extend([26; 10]);
        want.extend([16, 16, 100, 2, 31, 100, 100, 31]);
        assert_eq!(r.borrow().log, want);
        assert_eq!((s[0x192], s[0x18B], s[0x18D], s[0x191], s[0x1D4], s[0x1B1]), (1, 40, 20, 30, 3, 0));
        assert_eq!((s[0], s[0xC], s[0x1BD], s[0x17B]), (1, 0x10, 0x6B, 0x17));
        assert_eq!(get16(&sold, 9, 0x12E), 0x456);
        assert_eq!(m.tiles[tile as usize * 8 + 5], 9);
        assert_eq!(r.borrow().calls, vec![(51, vec![9])]);
    }

    #[test]
    fn merc_entry_two_mercs_swap_lead() {
        let mut sold = vec![0u8; SLOTS * REC];
        let it = vec![0u8; ITEM_TYPES * 12];
        let mut m = map();
        for k in 0..8 {
            sold[k * REC + 0x17A] = k as u8;
            sold[k * REC + 0x138] = 1;
        }
        for k in 0..2 {
            sold[k * REC + 0x182] = 1;
            sold[k * REC + 0x183] = 50;
        }
        let mut w = world2(&mut sold, &it, &mut m);
        w.selected = 0;
        let r = rng(vec![]);
        let h = host(&r);
        let tile = 30 * 56 + 30;
        ja1_merc_entry(&mut w, &h, tile, 1);
        // Merc 1 went north of the entry tile; then the selected merc and merc 1 change places.
        assert_eq!(get16(&sold, 0, 0x114), (tile - 56) as u16);
        assert_eq!(get16(&sold, 1, 0x114), tile as u16);
        assert_eq!(sold[2 * REC + 0x138], 0);
        let calls: Vec<i32> = r.borrow().calls.iter().map(|c| c.0 * 100 + c.1[0]).collect();
        assert_eq!(calls, vec![5100, 5800, 5101, 5100, 5101]);
    }

    #[test]
    fn xra_enemy_start_tile() {
        let mut sold = vec![0u8; SLOTS * REC];
        let it = vec![0u8; ITEM_TYPES * 12];
        let mut m = map();
        let s = 40 * REC;
        sold[s + 0x17A] = 40;
        sold[s + 0x116..s + 0x118].copy_from_slice(&0x200u16.to_le_bytes());
        sold[s + 0x3C] = 0x31;
        sold[s + 0x17B] = 0xFE;
        sold[s + 0x54..s + 0x5A].copy_from_slice(&[0, 9, 9, 0, 9, 9]);
        sold[s + 0x60..s + 0x66].copy_from_slice(&[7, 9, 9, 0, 9, 9]);
        let mut w = world2(&mut sold, &it, &mut m);
        let r = rng(vec![2, 1, 3]);
        let h = host(&r);
        ja1_xra_enemy(&mut w, &h, 40);
        assert_eq!(r.borrow().log, vec![4, 3, 8]);
        assert_eq!(get16(&sold, 40, 0x12E), 0x200);
        assert_eq!(&sold[s + 0x54..s + 0x5A], &[0, 0, 0, 0, 9, 9]);
        assert_eq!(&sold[s + 0x60..s + 0x66], &[7, 9, 9, 0, 0, 0]);
        assert_eq!((sold[s + 0x3C], sold[s + 0x17B], sold[s + 0x17C], sold[s + 0x1E0]), (0x30, 2, 5, 2));
        assert_eq!(m.tiles[0x200 * 8 + 5], 40);
        assert_eq!(r.borrow().calls, vec![(51, vec![40])]);
    }

    // ---- batches 3 and 4 ----
    fn ground_map() -> Map {
        let mut m = map();
        for t in 0..3136 {
            m.tiles[t * 8 + 6] = 0xFF;
        }
        m
    }

    #[test]
    fn put_item_moves_off_a_taken_tile() {
        let mut sold = vec![0u8; SLOTS * REC];
        let mut it = vec![0u8; ITEM_TYPES * 12];
        it[12 * 9 + 6] = 3; // stack class 3: unit conditions filled
        let mut m = ground_map();
        let mut ground = vec![0u8; 200 * 12];
        ground[0] = 5; // entry 0 in use, on tile `start`, with byte 2 > 0
        ground[2] = 50;
        let start = 10 * 56 + 10;
        m.tiles[start * 8 + 6] = 0;
        let mut w = world2(&mut sold, &it, &mut m);
        w.ground_items = ground.as_mut_ptr();
        let r = rng(vec![]);
        let h = host(&r);
        let item = [9u8, 3, 80, 0, 0, 0, 0, 0, 0, 0, 0, 0];
        ja1_put_item(&mut w, &h, start as i32, item.as_ptr(), 0xFF, 0, 0, 1);
        // Taken tile: the item goes one step north into free entry 1; byte 6 of start keeps entry 0 (its byte 6 is 0).
        let dest = start - 56;
        assert_eq!(m.tiles[dest * 8 + 6], 0);
        assert_eq!(m.tiles[start * 8 + 6], 1);
        assert_eq!(&ground[12..24], &[9, 3, 80, 0, 0, 0, 0xFF, 0, 80, 80, 0, 0]);
        assert!(r.borrow().log.is_empty() && r.borrow().calls.is_empty());
    }

    /// Ledger row c2aea50:ja1logic:2228: PutItem 0x3FC78 fills the unit conditions of a stack at 0x3FDDB..0x3FDFE
    /// with no bound (`cmp byte [eax+0xEB168],0; jne` / `mov [eax+0xEB168],cl` / `cmp edx,ecx; jl`), so for ground
    /// entry 199 the bytes after the 2400-byte table (g_textBuf 0xEBAC0) are filled when they are 0.
    #[test]
    fn put_item_stack_199_fills_into_the_text_buffer() {
        let mut sold = vec![0u8; SLOTS * REC];
        let mut it = vec![0u8; ITEM_TYPES * 12];
        it[12 * 9 + 6] = 3;
        let mut m = ground_map();
        let mut ground = vec![0u8; 200 * 12];
        for k in 0..199 {
            ground[12 * k] = 5;
        }
        let mut text = vec![0u8; 128];
        text[1] = 7;
        let start = 10 * 56 + 10;
        let mut w = world2(&mut sold, &it, &mut m);
        w.ground_items = ground.as_mut_ptr();
        w.text_buf = text.as_mut_ptr();
        let r = rng(vec![]);
        let h = host(&r);
        let item = [9u8, 8, 80, 0, 0, 0, 0, 0, 0, 0, 0, 0];
        ja1_put_item(&mut w, &h, start as i32, item.as_ptr(), 0xFF, 1, 0, 0);
        assert_eq!(&ground[12 * 199..], &[9, 8, 80, 0, 0, 0, 0xFF, 0, 80, 80, 80, 80]);
        assert_eq!(&text[..4], &[80, 7, 80, 0]);
    }

    /// Ledger row c2aea50:ja1logic:3250: port rule (SPEC GAP), slot 48 names no record, so op 329
    /// gives 0 with no random draw and no write past the 48 records (the original's callers pass only free enemy slots).
    #[test]
    fn enemy_walk_in_slot_48_does_nothing() {
        let mut sold = vec![0xAAu8; (SLOTS + 1) * REC];
        let it = vec![0u8; ITEM_TYPES * 12];
        let mut m = map();
        let mut w = world2(&mut sold[..SLOTS * REC], &it, &mut m);
        let r = rng(vec![]);
        let h = host(&r);
        assert_eq!(ja1_enemy_walk_in(&mut w, &h, SLOTS as i32, 5), 0);
        assert!(r.borrow().log.is_empty());
        assert!(sold[SLOTS * REC..].iter().all(|&b| b == 0xAA));
    }

    #[test]
    fn put_item_note_once() {
        let mut sold = vec![0u8; SLOTS * REC];
        let it = vec![0u8; ITEM_TYPES * 12];
        let mut m = ground_map();
        let mut ground = vec![0u8; 200 * 12];
        let mut notes = vec![0u8; 30];
        notes[4] = 0x70;
        let mut w = world2(&mut sold, &it, &mut m);
        w.ground_items = ground.as_mut_ptr();
        w.notes = notes.as_mut_ptr();
        let r = rng(vec![]);
        let h = host(&r);
        let item = [0x70u8, 1, 100, 0, 0, 0, 0, 0, 0, 0, 0, 0];
        ja1_put_item(&mut w, &h, 100, item.as_ptr(), 0xFF, 1, 1, 0);
        ja1_put_item(&mut w, &h, 200, item.as_ptr(), 0xFF, 1, 1, 0);
        assert_eq!(notes[5], 1);
        assert_eq!(m.tiles[100 * 8 + 6], 0);
        assert_eq!(m.tiles[200 * 8 + 6], 0xFF);
        assert_eq!(ground[12], 0);
    }

    #[test]
    fn take_ground_item_keep() {
        let mut sold = vec![0u8; SLOTS * REC];
        let it = vec![0u8; ITEM_TYPES * 12];
        let mut m = ground_map();
        let mut ground = vec![0u8; 200 * 12];
        ground[3 * 12..4 * 12].copy_from_slice(&[0x20, 1, 40, 0, 0, 0, 0xFF, 7, 7, 7, 7, 7]);
        let mut held = vec![0u8; 12];
        let mut blast = vec![0u8; 60];
        for k in 0..30 {
            blast[2 * k..2 * k + 2].copy_from_slice(&(500u16).to_le_bytes());
        }
        let mut codes = vec![0u8; 3136 * 2];
        m.tiles[500 * 8 + 6] = 3;
        m.tiles[500 * 8 + 1] = 0x86;
        m.t0c[0] = 1; // ground class 1: move code 1200 everywhere once t1 is cleared
        let mut w = world2(&mut sold, &it, &mut m);
        w.ground_items = ground.as_mut_ptr();
        w.held = held.as_mut_ptr();
        w.blast_list = blast.as_mut_ptr();
        w.move_codes = codes.as_mut_ptr();
        let r = rng(vec![]);
        let h = host(&r);
        ja1_take_ground_item(&mut w, &h, 500, 1);
        assert_eq!(r.borrow().calls, vec![(85, vec![])]);
        assert_eq!(&held[..], &[0x20, 1, 100, 0, 0, 0, 0xFF, 7, 7, 7, 7, 7]);
        assert_eq!(&ground[36..48], &[0, 0, 0, 0, 0, 0, 0, 7, 7, 7, 7, 7]);
        assert_eq!((m.tiles[500 * 8 + 6], m.tiles[500 * 8 + 1]), (0xFF, 0));
        assert_eq!(&blast[0..4], &[0x41, 0x0C, 0xF4, 0x01]);
        assert_eq!(&codes[1000..1002], &1200u16.to_le_bytes());
        assert_eq!(&codes[6270..6272], &1200u16.to_le_bytes());
    }

    #[test]
    fn take_ground_patch_connected() {
        let mut sold = vec![0u8; SLOTS * REC];
        let it = vec![0u8; ITEM_TYPES * 12];
        let mut m = ground_map();
        let mut ground = vec![0u8; 200 * 12];
        let mut listed = vec![0u8; 32];
        listed[0] = 0x44;
        // A patch of type 0x33 on tiles 300, 301, 357; tile 302 holds type 0x44 (another type).
        for (g, t, ty) in [(0usize, 300usize, 0x33u8), (1, 301, 0x33), (2, 357, 0x33), (3, 302, 0x44)] {
            ground[12 * g] = ty;
            m.tiles[t * 8 + 6] = g as u8;
        }
        let mut w = world2(&mut sold, &it, &mut m);
        w.ground_items = ground.as_mut_ptr();
        w.listed = listed.as_ptr();
        let r = rng(vec![]);
        let h = host(&r);
        ja1_take_ground_patch(&mut w, &h, 300, 0x33);
        for t in [300, 301, 357] {
            assert_eq!(m.tiles[t * 8 + 6], 0xFF);
        }
        assert_eq!(m.tiles[302 * 8 + 6], 3);
        assert_eq!(ground[36], 0x44);
        // A listed type stays (keep 0).
        ja1_take_ground_item(&mut w, &h, 302, 0);
        assert_eq!(m.tiles[302 * 8 + 6], 3);
        assert!(r.borrow().calls.is_empty());
    }

    // ---- batch 5 ----
    #[test]
    fn soldier_start_enemy() {
        let mut sold = vec![0u8; SLOTS * REC];
        let it = vec![0u8; ITEM_TYPES * 12];
        let mut m = map();
        let r0 = 41 * REC;
        sold[r0 + 0x17A] = 41;
        sold[r0 + 0x182] = 1;
        sold[r0 + 0x1D4] = 3;
        sold[r0 + 0x192] = 3;
        sold[r0 + 0x1A1] = 6;
        sold[r0 + 0x114..r0 + 0x116].copy_from_slice(&(57u16 * 3).to_le_bytes());
        m.tiles[171 * 8 + 2] = 4;
        m.t1c[0] = 9;
        let mut t2 = vec![0u8; 256];
        t2[4] = 0x77;
        let mut rows = vec![0u8; 256 * 8];
        rows[2] = 9; // animation 0: fixed 9 ticks per frame
        let mut watch = vec![0u8; 48];
        let mut last_e = vec![0u8; 640];
        let mut rem_e = vec![0xAAu8; 320];
        let mut w = world2(&mut sold, &it, &mut m);
        w.cur_sector = 3;
        w.busy = 2;
        w.t2_class = t2.as_ptr();
        w.anim_rows = rows.as_ptr();
        w.enemy_watch = watch.as_mut_ptr();
        w.last_seen_e = last_e.as_mut_ptr();
        w.remember_e = rem_e.as_mut_ptr();
        let r = rng(vec![123, 10]);
        let h = host(&r);
        ja1_soldier_start(&mut w, &h, 41);
        let s = &sold[r0..r0 + REC];
        assert_eq!(&s[0x16E..0x177], b"ENEMY 41\0");
        assert_eq!(s[0xBA], 3);
        assert_eq!(r.borrow().log, vec![300, 100]);
        assert_eq!(&s[0xA8..0xAC], &123u32.to_le_bytes());
        // Wakes: random 10 < 25 + (3 - 1) * 5.
        assert_eq!(s[0xB9], 1);
        assert_eq!(watch[41], 1);
        assert_eq!(w.busy, 1);
        assert_eq!(get16(&sold, 41, 0x116), 3 * 16 + 5);
        assert_eq!(get16(&sold, 41, 0x118), 3 * 14 + 5);
        assert_eq!((s[0x1B5], s[0x1B6], s[0x1B7]), (9, 4, 0x77));
        assert_eq!(get16(&sold, 41, 0xD2), 9);
        assert_eq!((s[0x19E], s[0x179], s[0x1B0], s[0x1CF], s[0x1A5]), (8, 0x40, 1, 6, 0));
        assert_eq!(&s[0xA4..0xA8], &70u32.to_le_bytes());
        assert_eq!(&last_e[80..82], &[0x41, 0x0C]);
        assert_eq!(&last_e[158..160], &[0x41, 0x0C]);
        assert_eq!((last_e[79], last_e[160]), (0, 0));
        assert!(rem_e[40..80].iter().all(|&b| b == 0));
        assert_eq!(rem_e[80], 0xAA);
    }

    #[test]
    fn soldier_start_merc_and_tapper() {
        let mut sold = vec![0u8; SLOTS * REC];
        let it = vec![0u8; ITEM_TYPES * 12];
        let mut m = map();
        // Merc in record 3: Max AP 11, an empty item 4 with stale bytes.
        let r0 = 3 * REC;
        sold[r0 + 0x17A] = 3;
        sold[r0 + 0x182] = 1;
        sold[r0 + 0x17E] = 8;
        for (o, v) in [(0x183, 50u8), (0x184, 50), (0x192, 2), (0x18B, 50), (0x18C, 50), (0x18A, 100)] {
            sold[r0 + o] = v;
        }
        sold[r0 + 0x30..r0 + 0x3C].copy_from_slice(&[0, 5, 5, 5, 5, 5, 5, 5, 5, 5, 5, 5]);
        sold[r0 + 0x24] = 0x27;
        // Tapper in record 20, not in the sector.
        sold[20 * REC + 0x17A] = 0x10;
        let mut w = world2(&mut sold, &it, &mut m);
        let r = rng(vec![]);
        let h = host(&r);
        ja1_soldier_start(&mut w, &h, 3);
        ja1_soldier_start(&mut w, &h, 20);
        let s = &sold[r0..r0 + REC];
        assert_eq!((s[0xBA], s[0x1A5], s[0x17E], s[0xB9], s[0x1B0]), (0, 11, 0, 0, 0));
        assert_eq!(s[0x19E], 8); // 7 for level 2, + 1 for item 3 type 0x27
        assert!(s[0x30..0x3C].iter().all(|&b| b == 0));
        assert_eq!(s[0x16E], 0);
        let t = &sold[20 * REC..21 * REC];
        assert_eq!(&t[0x16E..0x177], b"TAPPER16\0");
        assert_eq!((t[0xBA], t[0x194]), (2, 1));
        assert_eq!(r.borrow().log, vec![300]);
    }

    // ---- batch 6 ----
    #[test]
    fn tile_patch_records() {
        let mut sold = vec![0u8; SLOTS * REC];
        let it = vec![0u8; ITEM_TYPES * 12];
        let mut m = map();
        m.t1c[0xA0] = 1; // object class 1: move code 2000
        let mut codes = vec![0u8; 3136 * 2];
        let mut text = vec![0xEEu8; 128];
        let mut w = world2(&mut sold, &it, &mut m);
        w.move_codes = codes.as_mut_ptr();
        w.text_buf = text.as_mut_ptr();
        let r = rng(vec![]);
        // record 1: tile 100, object 0x9F (+1 with cl); record 2: tile 0 (not counted); record 3: short read.
        r.borrow_mut().stream = vec![100, 0, 0x9F, 0, 0, 0, 5, 0, 7];
        let h = host(&r);
        assert_eq!(ja1_tile_patch(&mut w, &h, 0, 7, 3, 1, 1), 2);
        assert_eq!(&text[..19], b"MISC\\CRATE07.BIN\0\xEE\xEE");
        assert_eq!((m.tiles[100 * 8 + 1], m.tiles[100 * 8 + 3]), (0xA0, 0x73));
        assert_eq!(&codes[200..202], &2000u16.to_le_bytes());
        let ops: Vec<i32> = r.borrow().calls.iter().map(|c| c.0).collect();
        assert_eq!(ops, vec![86, 87, 87, 87, 87, 87, 87, 88]);
        assert_eq!(r.borrow().calls[0].1, vec![0, 7, 3]);
        // Record 3: a 1-byte read gives rec[0] = 7; rec[1] and rec[2..3] keep record 2's 0 and 5, 0.
        assert_eq!(m.tiles[7 * 8 + 1], 5);
        r.borrow_mut().open = -1;
        assert_eq!(ja1_tile_patch(&mut w, &h, 3, 120, 3, 0, 0), -1);
        assert_eq!(&text[..17], b"MISC\\LIVEPL120.BI");
    }

    #[test]
    fn item_patch_and_plant_sector() {
        let mut sold = vec![0u8; SLOTS * REC];
        let it = vec![0u8; ITEM_TYPES * 12];
        let mut m = map();
        for t in 0..3136 {
            m.tiles[t * 8 + 6] = 0xFF;
        }
        let mut ground = vec![0u8; 200 * 12];
        let mut ib = vec![0u8; 12];
        let mut text = vec![0u8; 128];
        let mut notes = vec![0u8; 30];
        let mut plants = vec![0u8; 48];
        plants[24 + 4] = 9; // plant 2 in sector 9
        plants[24 + 8] = 2;
        plants[24 + 9] = 0;
        plants[24 + 10] = 1;
        let mut story = vec![0u8; 32];
        story[5] = 2; // live plant
        let mut sectors = vec![0u8; 80 * 9];
        let mut w = world2(&mut sold, &it, &mut m);
        w.ground_items = ground.as_mut_ptr();
        w.item_buf = ib.as_mut_ptr();
        w.text_buf = text.as_mut_ptr();
        w.notes = notes.as_mut_ptr();
        w.plants = plants.as_mut_ptr();
        w.story = story.as_mut_ptr();
        w.sector_state = sectors.as_mut_ptr();
        w.cur_sector = 9;
        let r = rng(vec![]);
        r.borrow_mut().stream = vec![0x2C, 1, 0x20, 1, 90, 0, 0, 0, 0xFF, 0, 0, 0, 0, 0];
        let h = host(&r);
        ja1_plant_sector(&mut w, &h);
        // Not owned: the item is put exactly on tile 0x12C, the plant state becomes 3.
        assert_eq!(m.tiles[0x12C * 8 + 6], 0);
        assert_eq!(&ground[0..7], &[0x20, 1, 90, 0, 0, 0, 0xFF]);
        assert_eq!(plants[24 + 8], 3);
        let ops: Vec<i32> = r.borrow().calls.iter().map(|c| c.0).collect();
        assert_eq!(ops, vec![86, 88, 86, 87, 87, 88]);
        assert_eq!(r.borrow().calls[2].1, vec![1, 9, 1]);
        assert_eq!(&text[..15], b"MISC\\ITEM09.BIN");
        ja1_plant_done(&mut w, &h);
        assert_eq!(plants[24 + 8], 0);
        assert_eq!(r.borrow().calls.last().unwrap(), &(89, vec![9, 0, 0x51]));
    }

    #[test]
    fn crate_drop_day_and_lost() {
        let mut sold = vec![0u8; SLOTS * REC];
        let it = vec![0u8; ITEM_TYPES * 12];
        let mut m = map();
        let crates = vec![5u8, 3, 0, 0, 5, 4, 0, 0, 6, 3, 0, 0, 5, 1, 0, 0];
        let mut state = vec![1u8, 0, 0, 3];
        let mut shown = vec![0xFFu8, 0xFF, 0xFF, 0xFF];
        let mut sup = vec![1u8, 2, 3, 4];
        let mut text = vec![0u8; 128];
        let mut w = world2(&mut sold, &it, &mut m);
        w.crates = crates.as_ptr();
        w.crate_state = state.as_mut_ptr();
        w.crate_shown = shown.as_mut_ptr();
        w.supplies = sup.as_mut_ptr();
        w.text_buf = text.as_mut_ptr();
        w.cur_sector = 5;
        w.day = 3;
        let r = rng(vec![]);
        let h = host(&r);
        ja1_crate_drops(&mut w, &h);
        // Drop 0 (day 3, state 1, not owned, not kept): lost to the enemy. Drop 1 (day 4): nothing yet.
        // Drop 2: another sector. Drop 3 (day 1 < 3, state 3, shown -1): tiles (1, 1), shown 1.
        assert_eq!(state, vec![3, 0, 0, 3]);
        assert_eq!(shown, vec![1, 0xFF, 0xFF, 1]);
        assert_eq!(sup, vec![11, 17, 13, 9]);
        let ops: Vec<i32> = r.borrow().calls.iter().map(|c| c.0).collect();
        assert_eq!(ops, vec![86, 88, 86, 88, 86, 88]);
        ja1_crate_lost(&mut w, &h, 0);
        assert_eq!(sup, vec![11, 17, 13, 9]);
    }

    // ---- batch 7 ----
    fn blast_map(m: &mut Map) {
        for t in 0..3136 {
            m.tiles[t * 8 + 6] = 0xFF;
        }
    }

    #[test]
    fn tmp_file_reads_tiles_and_ground() {
        let mut sold = vec![0u8; SLOTS * REC];
        let it = vec![0u8; ITEM_TYPES * 12];
        let mut m = map();
        let mut gi = vec![0u8; GROUND * 12];
        let mut w = world2(&mut sold, &it, &mut m);
        w.ground_items = gi.as_mut_ptr();
        w.cur_sector = 7;
        let r = rng(vec![]);
        r.borrow_mut().stream = (0..3136 * 5 + 2400).map(|i| (i % 251) as u8).collect();
        let h = host(&r);
        assert_eq!(ja1_tmp_file(&mut w, &h), 1);
        assert_eq!(&m.tiles[8..16], &[0, 5, 6, 7, 8, 0x40, 9, 0]);
        assert_eq!(gi[0], ((3136 * 5) % 251) as u8);
        {
            let c = &r.borrow().calls;
            assert_eq!(c[0], (92, vec![7]));
            assert_eq!(c.len(), 1 + 3136 * 5 + 2);
            assert_eq!(c[c.len() - 2], (87, vec![0x960]));
            assert_eq!(c[c.len() - 1].0, 88);
        }
        r.borrow_mut().open = -1;
        assert_eq!(ja1_tmp_file(&mut w, &h), 0);
    }

    #[test]
    fn ground_blast_bombs_and_mines() {
        let mut sold = vec![0u8; SLOTS * REC];
        let it = vec![0u8; ITEM_TYPES * 12];
        let mut m = map();
        blast_map(&mut m);
        let mut gi = vec![0u8; GROUND * 12];
        let mut secs = vec![0u8; 60 * 0xB1];
        // Two 0x3F bombs (entries 0, 1) and two mines (entries 2, 3).
        for (g, (ty, t)) in [(0x3Fu8, 100usize), (0x3F, 200), (0x11, 300), (0x11, 400)].iter().enumerate() {
            gi[12 * g] = *ty;
            gi[12 * g + 1] = 2;
            m.tiles[t * 8 + 6] = g as u8;
        }
        let mut w = world2(&mut sold, &it, &mut m);
        w.ground_items = gi.as_mut_ptr();
        w.sectors = secs.as_mut_ptr();
        w.cur_sector = 4;
        w.bomb_count = 2;
        secs[4 * 0xB1 + 0xB0] = 5;
        let r = rng(vec![]);
        let mut h = host(&r);
        // Ops 90 and 91 go through the library's dispatch: bit 63 sends them (and every op from 63 on) to this host.
        h.host_ops |= 1 << 63;
        // A bomb: only the first tile goes off; bomb_count reaches 3, so flag becomes 1.
        ja1_ground_blast(&mut w, &h, 0x3F, 0, 300, 9, 0xFF);
        assert_eq!(w.bomb_count, 3);
        assert_eq!(r.borrow().calls, vec![(90, vec![9, 100, 3, 1]), (91, vec![0xC41, 0x3F, 1, 300, 9, -1])]);
        // Mines: both taken away (the count stays above 0), the low power is rolled once.
        r.borrow_mut().calls.clear();
        ja1_ground_blast(&mut w, &h, 0x11, 0, 50, 9, 0);
        assert_eq!(r.borrow().log, vec![1000]);
        assert!(r.borrow().calls.is_empty());
        assert_eq!(secs[4 * 0xB1 + 0xB0], 3);
    }

    // ---- batch 8 ----
    #[test]
    fn setup_sector_call_order() {
        let mut sold = vec![0u8; SLOTS * REC];
        let it = vec![0u8; ITEM_TYPES * 12];
        let mut m = map();
        blast_map(&mut m);
        let mut gi = vec![0u8; GROUND * 12];
        let mut secs = vec![0u8; 60 * 0xB1];
        let mut state = vec![0u8; 80 * 9];
        let mut calls = vec![0xAAu8; 8];
        let mut blast = vec![0u8; 60];
        let mut text = vec![0u8; 128];
        let mut mc = vec![0u8; 3136 * 2];
        let mut w = world2(&mut sold, &it, &mut m);
        w.ground_items = gi.as_mut_ptr();
        w.sectors = secs.as_mut_ptr();
        w.sector_state = state.as_mut_ptr();
        w.calls = calls.as_mut_ptr();
        w.blast_list = blast.as_mut_ptr();
        w.text_buf = text.as_mut_ptr();
        w.move_codes = mc.as_mut_ptr();
        w.cur_sector = 3;
        w.bomb_count = 5;
        w.selected = 2;
        let r = rng(vec![]);
        r.borrow_mut().open = -1;
        let h = host(&r);
        // Not owned: the XRA load runs (no tmp file); the reads come back empty.
        ja1_setup_sector(&mut w, &h, 0x63C, 2);
        assert_eq!(w.bomb_count, 0);
        assert_eq!(w.anim_signal, -1);
        assert_eq!(w.alarm, 0);
        assert_eq!(w.merc_to_show, 2);
        assert_eq!(calls, vec![0x41, 0x0C, 0, 0xAA, 0x41, 0x0C, 0, 0xAA]);
        assert_eq!(&blast[0..2], &[0x41, 0x0C]);
        assert_eq!(text, vec![0u8; 128]); // kind 4 does not write the path
        {
            let c = &r.borrow().calls;
            assert_eq!(c[0], (93, vec![]));
            assert_eq!(c[1], (92, vec![3]));
            assert_eq!(c[2], (86, vec![4, 0, 0]));
            assert_eq!(c[3], (96, vec![3 * 0x2E98]));
            assert_eq!(c[4], (87, vec![0x200]));
            let ops: Vec<i32> = c.iter().map(|e| e.0).filter(|&o| o != 87).collect();
            assert_eq!(ops, vec![93, 92, 86, 96, 88, 33, 95]);
            assert_eq!(c.iter().filter(|e| e.0 == 87).count(), 16 + 3136 + 50);
        }
        // Owned, with a set-off count of 0: no XRA load.
        state[9 * 3] = 1;
        r.borrow_mut().calls.clear();
        ja1_setup_sector(&mut w, &h, 0x63C, 2);
        assert_eq!(w.alarm, 1);
        let ops: Vec<i32> = r.borrow().calls.iter().map(|e| e.0).collect();
        assert_eq!(ops, vec![93, 92, 33, 95]);
    }

    #[test]
    fn enter_sector_selects_replays_and_scrolls() {
        let mut sold = vec![0u8; SLOTS * REC];
        let it = vec![0u8; ITEM_TYPES * 12];
        let mut m = map();
        blast_map(&mut m);
        let mut gi = vec![0u8; GROUND * 12];
        let mut secs = vec![0u8; 60 * 0xB1];
        let mut state = vec![0u8; 80 * 9];
        let mut calls = vec![0u8; 8];
        let mut blast = vec![0u8; 60];
        let mut text = vec![0u8; 128];
        let mut mc = vec![0u8; 3136 * 2];
        // Record 0: absent (+0x17E = 1). Record 1: in the sector, +0x17E = 8, no health. Record 2: selectable.
        sold[0x17E] = 1;
        sold[REC + 0x182] = 1;
        sold[REC + 0x17E] = 8;
        sold[2 * REC + 0x182] = 1;
        sold[2 * REC + 0x183] = 30;
        // Owned sector (state byte 0 = 1, no XRA load) with state byte 2 set.
        state[9 * 3] = 1;
        state[9 * 3 + 2] = 1;
        let mut w = world2(&mut sold, &it, &mut m);
        w.ground_items = gi.as_mut_ptr();
        w.sectors = secs.as_mut_ptr();
        w.sector_state = state.as_mut_ptr();
        w.calls = calls.as_mut_ptr();
        w.blast_list = blast.as_mut_ptr();
        w.text_buf = text.as_mut_ptr();
        w.move_codes = mc.as_mut_ptr();
        w.cur_sector = 3;
        w.view_w = 224;
        w.view_h = 168;
        w.opt_onscreen = 1;
        w.view_x = 5000;
        let r = rng(vec![]);
        r.borrow_mut().open = -1;
        let h = host(&r);
        ja1_enter_sector(&mut w, &h, 0x63C);
        assert_eq!((w.selected, w.idle_day), (2, 0));
        assert_eq!(sold[0x138], 0);
        assert_eq!(sold[REC + 0x138], 0);
        assert_eq!(sold[2 * REC + 0x138], 1);
        let c = r.borrow().calls.clone();
        assert_eq!(c[0], (89, vec![3, 3, 0x58]));
        // Step 3, then: Time compression (0) (ops 198 mouse_wait, 315 mouse_release); time_compress = 0.
        assert_eq!((c[1].0, c[2].0), (198, 315));
        assert_eq!((w.time_compress, w.tick_step), (0, 10));
        assert_eq!(c[3].0, 93);
        // The view starts far off, so the selected merc's tile (after the start) is outside it: the view follows.
        let t = get16(&sold, 2, 0x114) as i16 as i32;
        let (px, py) = ((t % 56) * 16, (t / 56) * 14);
        // JumpViewToTile? 0x3F490 ends in ClampView 0x24830: the original keeps the view on the map (loadsector_diff).
        assert_eq!(w.view_x, (px - 0x70 + 16).clamp(0, 0x380 - w.view_w));
        let mut y = py - 84 + 14;
        while y % 14 != 0 {
            y -= 1;
        }
        assert_eq!(w.view_y, y.clamp(0, 0x310 - w.view_h));

        // opt_onscreen 2: m = 0xA0; a point inside the view leaves it as it is.
        w.opt_onscreen = 2;
        w.view_x = 5000;
        ja1_enter_sector(&mut w, &h, 0x63C);
        let t = get16(&sold, 2, 0x114) as i16 as i32;
        assert_eq!(w.view_x, ((t % 56) * 16 - 0xA0 + 16).clamp(0, 0x380 - w.view_w));
        let (vx, vy) = (w.view_x, w.view_y);
        ja1_enter_sector(&mut w, &h, 0x63C);
        if get16(&sold, 2, 0x114) as i16 as i32 == t {
            assert_eq!((w.view_x, w.view_y), (vx, vy));
        }

        // No merc selectable, state byte 2 = 0: no op 89, selected = 8, the fixed view.
        state[9 * 3 + 2] = 0;
        sold[2 * REC + 0x183] = 0;
        r.borrow_mut().calls.clear();
        ja1_enter_sector(&mut w, &h, 0x63C);
        assert_eq!((w.selected, w.idle_day), (8, 1));
        assert_eq!((w.view_x, w.view_y), (336, 308));
        assert!(r.borrow().calls.iter().all(|e| e.0 != 89));

        // Ledger row ja1logic:2916: no merc selectable but state byte 2 set: still no op 89. LoadTacticalSector?
        // 0x2B079 `cmp eax,8; jge 0x2B0B1` skips RecordReplayEvent 0x3FA04 when selected = 8.
        state[9 * 3 + 2] = 1;
        r.borrow_mut().calls.clear();
        ja1_enter_sector(&mut w, &h, 0x63C);
        assert_eq!((w.selected, w.idle_day), (8, 1));
        assert!(r.borrow().calls.iter().all(|e| e.0 != 89));
    }
}

#[cfg(test)]
mod guard_world_tests {
    use super::*;

    fn host() -> Ja1Host {
        Ja1Host { ctx: std::ptr::null_mut::<std::os::raw::c_void>(), random: None, call: None, host_ops: 0 }
    }

    /// Port rule (SPEC GAP): a slot or record outside 0..47, or a null soldiers buffer, does nothing.
    #[test]
    fn guard_slot_entry_points() {
        let mut sold = vec![0xAAu8; (SLOTS + 1) * REC];
        let mut w: Box<Ja1World> = Box::new(unsafe { std::mem::zeroed() });
        let h = host();
        let mut hit = false;
        with_slot(&mut *w, &h, 0, |_, _| hit = true);
        assert!(!hit, "null soldiers buffer");
        w.soldiers = sold.as_mut_ptr();
        with_slot(&mut *w, &h, SLOTS as i32, |_, _| hit = true);
        assert!(!hit, "slot 48");
        with_slot(&mut *w, &h, SLOTS as i32 - 1, |_, s| hit = s == SLOTS - 1);
        assert!(hit, "slot 47");
        ja1_slot_reset(&mut *w, &h, SLOTS as i32, 0);
        ja1_equip(&mut *w, &h, SLOTS as i32, 1);
        assert!(sold[SLOTS * REC..].iter().all(|&b| b == 0xAA), "record 48 untouched");
    }

    #[test]
    fn guard_self_rec_and_buf_put() {
        let mut sold = vec![0u8; SLOTS * REC];
        let mut w: Box<Ja1World> = Box::new(unsafe { std::mem::zeroed() });
        w.soldiers = sold.as_mut_ptr();
        let c = Ctx { w: &mut *w, h: std::ptr::null() };
        unsafe { *(*c.w).soldiers.add(0x17A) = 47 };
        assert_eq!(c.self_rec(0), Some(47));
        unsafe { *(*c.w).soldiers.add(0x17A) = 48 };
        assert_eq!(c.self_rec(0), None);
        let mut buf = [0u8; 6];
        let p = unsafe { buf.as_mut_ptr().add(1) };
        Ctx::buf_put(p, 4, 3, 5);
        Ctx::buf_put(p, 4, 4, 6);
        Ctx::buf_put(p, 4, -1, 7);
        assert_eq!(buf, [0, 0, 0, 0, 5, 0]);
    }
}
