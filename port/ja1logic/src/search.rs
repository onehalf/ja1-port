//! Slice 2b: the search procedures (docs/spec/ai.md "Search procedures"; C interface in docs/spec/port_api.md).
//!
//! A child module of `ai`: it extends the same `Ai` view of one soldier. Inner calls of a library op (for example
//! patrol_tile asking can_stand_at) go through `Ai::call`, the one dispatch point that applies the host_ops rule.
//! Host-only ops used here: tile_dist (7), path_ap (40), find_path (41), and random.
use super::{f, op, Ai, NONE, NO_SLOT, SLOTS, TILES};

/// World buffer sizes (port_api.md "World", slice 2b fields).
const KNOWLEDGE_LEN: usize = 64;
const FRESHER_LEN: usize = 2304;
const LAST_SEEN_M_LEN: usize = 16 * 8 * 2;
const LAST_SEEN_E_LEN: usize = 8 * 40 * 2;
const CALLS_LEN: usize = 8;
const PATHS_LEN: usize = SLOTS * 80;
const DIR_STEPS_LEN: usize = 257 * 2;
const CLASS_LEN: usize = 256;

/// Soldier record offsets used only by the searches (defs/types.toml struct.soldier).
const PATH_LEN: usize = 0x0D4;
const KNOWN_TILE: usize = 0x108;
const CALL_TILE: usize = 0x122;
const SEES: usize = 0x13A;
const SLOT_BYTE: usize = 0x17A;
const CALL_LEVEL: usize = 0x1C0;
const ITEM3_TYPE: usize = 0x024;

/// Byte i of a world buffer of `len` bytes; null or out of range reads as 0.
fn buf8(p: *const u8, len: usize, i: i64) -> u8 {
    if p.is_null() || i < 0 || i as u64 >= len as u64 {
        return 0;
    }
    unsafe { p.add(i as usize).read() }
}
/// Little-endian u16 entry `i` of a world buffer of `len` bytes (2 bytes per entry).
fn buf16(p: *const u8, len: usize, i: i64) -> u16 {
    let b = i.wrapping_mul(2);
    buf8(p, len, b) as u16 | (buf8(p, len, b.wrapping_add(1)) as u16) << 8
}

pub(super) fn s16(v: i32) -> i32 {
    v as i16 as i32
}
fn u16v(v: i32) -> u16 {
    v as u16
}

/// Tile parts (C division: a negative tile gives a negative row and column).
fn row(t: i32) -> i32 {
    t / 56
}
fn column(t: i32) -> i32 {
    t - row(t) * 56
}
pub(super) fn px(t: i32) -> i32 {
    column(t) * 16 + 5
}
pub(super) fn py(t: i32) -> i32 {
    row(t) * 14 + 5
}

/// World distance: floor(sqrt(dx² + dy²)), dx = |x1 − x2| × 15 / 16, dy = |y1 − y2| × 15 / 14 (Distance 0x1FD28).
pub(super) fn world_dist(x1: i32, y1: i32, x2: i32, y2: i32) -> i32 {
    let dx = (x1 as i64 - x2 as i64).abs() * 15 / 16;
    let dy = (y1 as i64 - y2 as i64).abs() * 15 / 14;
    isqrt((dx * dx + dy * dy) as u64) as i32
}
fn isqrt(n: u64) -> u64 {
    if n < 2 {
        return n;
    }
    let mut x = (n as f64).sqrt() as u64;
    while x * x > n {
        x -= 1;
    }
    while (x + 1) * (x + 1) <= n {
        x += 1;
    }
    x
}

/// step(t, k): the neighbour, or t itself when the step leaves the map.
pub(super) fn step(t: i32, k: i32) -> i32 {
    let n = t.wrapping_add(k);
    let ct = column(t);
    let edge = ct == 0 || ct == 55 || t <= 56 || t >= 0xC08;
    let leaves = n < 0
        || (ct == 0 && column(n) == 55)
        || (ct == 55 && column(n) == 0)
        || (t >= 0xC08 && n >= 0xC40);
    if edge && leaves { t } else { n }
}

/// Leash by orders (unsigned byte).
pub(super) fn leash_of(t1: u8) -> i32 {
    match t1 {
        0 => 1,
        1 => 5,
        2 => 15,
        3 | 4 => 99,
        _ => 0,
    }
}

impl Ai {
    // ---- world buffers (pointers copied at the point of use) ----
    pub(super) fn knowledge(&self, i: usize) -> i32 {
        buf8(unsafe { (*self.w).knowledge }, KNOWLEDGE_LEN, i as i64) as i8 as i32
    }
    /// fresher(b, a): b the older level, a the newer, both signed bytes. Non-zero = a is fresher.
    pub(super) fn fresher(&self, b: i32, a: i32) -> i32 {
        let i = 1152 + 8 * (b as i8 as i64) + a as i8 as i64;
        buf8(unsafe { (*self.w).fresher }, FRESHER_LEN, i) as i32
    }
    pub(super) fn dir_step(&self, d: i32) -> i32 {
        buf16(unsafe { (*self.w).dir_steps }, DIR_STEPS_LEN, d as i64) as i16 as i32
    }
    /// T0ClassOf 0x239C0.
    pub(super) fn t0_class(&self, v: u8) -> u8 {
        buf8(unsafe { (*self.w).t0_class }, CLASS_LEN, v as i64)
    }
    pub(super) fn t1_class(&self, v: u8) -> u8 {
        buf8(unsafe { (*self.w).t1_class }, CLASS_LEN, v as i64)
    }
    fn path_dir(&self, k: i32) -> i32 {
        buf8(unsafe { (*self.w).paths }, PATHS_LEN, self.s as i64 * 80 + k as i64) as i32
    }
    /// Side call (tile, level): mercs' side for me < 0x28, else the enemies'.
    fn side_call(&self) -> (u16, u8) {
        let p = unsafe { (*self.w).calls } as *const u8;
        let base = if self.s < 0x28 { 0 } else { 4 };
        (buf16(p, CALLS_LEN, base / 2), buf8(p, CALLS_LEN, base + 2))
    }
    fn last_spotter(&self) -> u8 {
        unsafe { (*self.w).last_spotter }
    }

    /// Byte i of tile t (signed tile number); outside 0..3135 reads as 0 (TileGround 0x23530 = byte 0,
    /// TileByte2? 0x23A70 = byte 2, TileOccupant 0x23944 = byte 5 & 0x7F).
    pub(super) fn tb(&self, t: i32, i: usize) -> u8 {
        if !(0..TILES as i32).contains(&t) {
            return 0;
        }
        self.tile_byte(t as u16, i)
    }

    // ---- shared definitions ----
    pub(super) fn leash(&self) -> i32 {
        leash_of(self.orders())
    }
    pub(super) fn post(&self) -> i32 {
        s16(self.rd16(f::HOME_TILE) as i32)
    }
    pub(super) fn rd16_of(&self, slot: usize, off: usize) -> u16 {
        self.rd8_of(slot, off) as u16 | (self.rd8_of(slot, off + 1) as u16) << 8
    }
    fn alive(&self, slot: usize) -> bool {
        self.rd8_of(slot, f::IN_CUR_SECTOR) != 0 && self.rd8_of(slot, f::IN_PLAY) != 0 && self.rd8_of(slot, f::HP) != 0
    }
    /// Opponents (slot i, index j).
    fn opponents(&self) -> Vec<(usize, usize)> {
        if self.s >= 0x28 {
            (0..0x28).map(|i| (i, i)).collect()
        } else {
            (0x28..0x30).map(|i| (i, i - 0x28)).collect()
        }
    }
    fn sight(&self, j: usize) -> i32 {
        self.s8(SEES + j)
    }
    fn last_seen(&self, j: usize) -> u16 {
        if self.s >= 0x28 {
            buf16(unsafe { (*self.w).last_seen_e }, LAST_SEEN_E_LEN, ((self.s - 0x28) * 40 + j) as i64)
        } else {
            // SPEC GAP: a merc-side soldier of slot 16..0x27 indexes past the 16 x 8 table; that reads as 0.
            buf16(unsafe { (*self.w).last_seen_m }, LAST_SEEN_M_LEN, (self.s * 8 + j) as i64)
        }
    }
    /// Known place of opponent i (index j); NONE for "none" (a = 0 and b = 0).
    pub(super) fn known_place(&self, i: usize, j: usize) -> u16 {
        let (a, b) = (self.sight(j), self.knowledge(i));
        if a == 0 && b == 0 {
            NONE
        } else if self.fresher(b, a) != 0 || a == b {
            self.last_seen(j)
        } else {
            self.rd16_of(i, KNOWN_TILE)
        }
    }
    pub(super) fn tile_dist_i(&self, a: i32, b: i32) -> i32 {
        self.call(op::TILE_DIST, &[s16(a), s16(b)]) as i32
    }
    pub(super) fn path_ap(&self, t: i32) -> i32 {
        s16(self.call(op::PATH_AP, &[self.slot(), s16(t)]) as i32)
    }
    pub(super) fn stand(&self, t: i32) -> bool {
        self.call(op::CAN_STAND_AT, &[self.slot(), s16(t)]) & 0xFF != 0
    }

    // ================= Standable tile =================
    pub(super) fn tile_standable(&self, t: i32) -> i32 {
        // 1.
        let o = (self.tb(t, 5) & 0x7F) as usize;
        if o < 0x30 && self.s8_of(o, f::B09C) > 0 {
            return 0;
        }
        // 2.
        let t1 = self.tb(t, 1);
        if self.t1_class(t1) <= 5 {
            return 1;
        }
        // 3.
        match t1 {
            0x3B | 0x41 | 0x5A | 0x60 => 1,
            0x3D | 0x3E | 0x5C | 0x5D => -2,
            0x43 | 0x45 | 0x62 | 0x64 => -1,
            _ => (self.tb(t, 3) == 0x73) as i32,
        }
    }

    /// can_stand_at(S, t), t signed 16-bit. 1 or 0.
    pub(super) fn can_stand_at(&self, t: i32) -> i32 {
        let t = s16(t);
        let (t0, t1) = (self.tb(t, 0), self.tb(t, 1));
        // 1.
        if self.s8(f::AI_TYPE) == 1 && !self.w_combat() && self.t0_class(t0) == 3 {
            return 0;
        }
        // 2.
        if self.tile_standable(t) <= 0 {
            return 0;
        }
        // 3.
        if t0 >= 0x6C && !matches!(t1, 0x1C | 0x1F) {
            return 0;
        }
        // 4.
        if self.tb(t, 5) & 0x80 != 0 && self.rd8(ITEM3_TYPE) != 0x28 {
            return 0;
        }
        // 5.
        let o = self.tb(t, 5) & 0x7F;
        let occ = if o >= 0x30 { NO_SLOT } else { o };
        if occ != NO_SLOT || u16v(t) == self.own_tile() {
            return 0;
        }
        // 6. The host call comes before the walkFailTile test.
        let found = self.call(op::FIND_PATH, &[self.slot(), t, 0]);
        if found == 0 || u16v(t) == self.rd16(f::WALK_FAIL_TILE) {
            return 0;
        }
        // 7.
        1
    }

    // ================= Patrol tile (op 4) =================
    pub(super) fn patrol_tile(&self) -> i32 {
        // 1.
        let r = self.leash();
        let boxed = r < 55;
        let post = self.post();
        // 2.
        let (mut left, mut top, mut wide, mut high) = (0, 0, 0, 0);
        if boxed {
            let (col, rw) = (column(post), row(post));
            left = r.min(col);
            let right = r.min(55 - col);
            top = r.min(rw);
            let bottom = r.min(55 - rw);
            wide = (left + right + 1) as i8 as i32;
            high = (top + bottom + 1) as i8 as i32;
        }
        // 3.
        for _ in 0..10 {
            let t = if boxed {
                let dx = self.random(wide);
                let dy = self.random(high);
                (dx.wrapping_sub(left) as i8 as i32)
                    .wrapping_add(post)
                    .wrapping_add((dy.wrapping_sub(top) as i8 as i32) * 56)
            } else {
                self.random(0xC40)
            };
            if self.stand(s16(t)) {
                return t;
            }
        }
        // 4.
        NONE as i32
    }

    // ================= Buddy tile (op 5) =================
    pub(super) fn buddy_tile(&self) -> i32 {
        // 1.
        let (n, base): (i32, i32) = match self.rd8(f::AI_TYPE) {
            0..=2 => (0x28, 0),
            3 | 4 => (8, 0x28),
            _ => return 0,
        };
        // 2.
        let r = self.leash();
        let mut tried = [false; 256];
        if (base..base + n).contains(&(self.s as i32)) {
            tried[self.s] = true;
        }
        // 3.
        for _ in 0..n - 1 {
            // a.
            let mut k: Option<usize> = None;
            for _ in 0..100_000 {
                let c = (self.random(n).wrapping_add(base) as u8) as usize;
                if !tried[c] {
                    k = Some(c);
                    break;
                }
            }
            let k = match k {
                Some(k) => k,
                None => return 0,
            };
            // b.
            if !self.alive(k) {
                tried[k] = true;
                continue;
            }
            // c.
            let post = self.post();
            if r < 99 {
                if post == NONE as i32 {
                    continue;
                }
                let bt = s16(self.rd16_of(k, f::TILE) as i32);
                if self.tile_dist_i(post, bt).wrapping_sub(1) > r {
                    continue;
                }
            }
            // d.
            for _ in 0..8 {
                let bt = s16(self.rd16_of(k, f::TILE) as i32);
                let d = self.random(8).wrapping_add(1);
                let nb = step(bt, self.dir_step(d));
                if r < 99 && self.tile_dist_i(self.post(), nb) > r {
                    continue;
                }
                if self.stand(nb) {
                    self.set_target(u16v(nb));
                    return 1;
                }
            }
        }
        // 4.
        0
    }

    // ================= Approach tile (op 10) =================
    pub(super) fn approach_tile(&self, target: i32) -> i32 {
        // 1.
        let r = self.leash();
        let mut t = u16v(target);
        if self.own_tile() == t {
            return NONE as i32;
        }
        // 2.
        if !self.stand(s16(t as i32)) {
            let mut ok = false;
            for _ in 0..8 {
                let d = self.random(8);
                let nb = step(s16(t as i32), self.dir_step(d.wrapping_add(1)));
                if self.stand(nb) {
                    t = u16v(nb);
                    ok = true;
                    break;
                }
            }
            if !ok {
                return NONE as i32;
            }
        }
        // 3.
        if r < 99 && self.tile_dist_i(self.post(), s16(t as i32)) > r {
            t = self.own_tile();
            let mut k = 0;
            // pathLen and paths are read again before each pass (the count test runs before the first pass too).
            while k < s16(self.rd16(PATH_LEN) as i32) {
                let nb = step(s16(t as i32), self.dir_step(self.path_dir(k) + 1));
                if self.tile_dist_i(self.post(), nb) > r {
                    break;
                }
                if self.stand(nb) {
                    t = u16v(nb);
                }
                k += 1;
            }
        }
        // 4.
        if t == self.own_tile() { NONE as i32 } else { t as i32 }
    }

    // ================= Dry tile (op 3) =================
    pub(super) fn find_dry_tile(&self) -> i32 {
        // 1.
        let m = s16(self.own_tile() as i32);
        let (col, rw) = (column(m), row(m));
        let mut best = 1000;
        let mut result = NONE;
        // 2.
        for big_r in [5, 10, 15, 20, 25] {
            let left = big_r.min(col) as i8 as i32;
            let right = big_r.min(55 - col) as i8 as i32;
            let up = big_r.min(rw) as i8 as i32;
            let down = big_r.min(55 - rw) as i8 as i32;
            for dy in -up..=down {
                for dx in -left..=right {
                    let t = s16(dx + m + dy * 56);
                    if self.stand(t) {
                        let ap = self.path_ap(t);
                        if ap != 0 && ap < best {
                            best = ap;
                            result = u16v(t);
                        }
                    }
                }
            }
            if result != NONE {
                break;
            }
        }
        // 3.
        result as i32
    }

    // ================= Target tile (op 23) =================
    pub(super) fn target_tile(&self) -> i32 {
        let m = s16(self.own_tile() as i32);
        let mut best = 1500;
        let mut result = NONE;
        for (i, j) in self.opponents() {
            if !self.alive(i) || (self.sight(j) == 0 && self.knowledge(i) == 0) {
                continue;
            }
            let t = self.known_place(i, j);
            if t == self.own_tile() {
                continue;
            }
            let tt = s16(t as i32);
            let d = s16(world_dist(px(m), py(m), px(tt), py(tt)));
            if d < best {
                best = d;
                result = t;
            }
        }
        result as i32
    }

    // ================= Noise tile (op 2) =================
    /// Returns (tile, loudness as u16).
    pub(super) fn noise_tile(&self) -> (u16, u16) {
        // 1.
        let m = s16(self.own_tile() as i32);
        let mut best: i32 = -10000;
        let mut result = NONE;
        let mut offer = |target: i32, factor: i32, value: u16| {
            let v = s16((self.tile_dist_i(m, target) as u8 as i32).wrapping_mul(factor));
            if v > best {
                best = v;
                result = value;
            }
        };
        // 2.
        for (i, j) in self.opponents() {
            if !self.alive(i) {
                continue;
            }
            let a = self.sight(j);
            if a < 0 {
                let ls = self.last_seen(j);
                offer(s16(ls as i32), a, ls);
            }
            let b = self.knowledge(i);
            if b < 0 {
                let kt = self.rd16_of(i, KNOWN_TILE);
                offer(s16(kt as i32), b, kt);
            }
        }
        // 3.
        let ct = self.rd16(CALL_TILE);
        if s16(ct as i32) != NONE as i32 {
            if ct == self.own_tile() {
                self.wr16(CALL_TILE, NONE);
                self.wr8(CALL_LEVEL, 0);
            } else {
                offer(s16(ct as i32), (self.rd8(CALL_LEVEL) >> 1) as i32 - 6, ct);
            }
        }
        // 4.
        let (sct, scl) = self.side_call();
        let t = s16(sct as i32);
        if t != NONE as i32 && u16v(t) != self.own_tile() {
            offer(t, (scl >> 1) as i32 - 6, u16v(t));
        }
        // 5.
        (result, best as u16)
    }

    // ================= News value (op 8) =================
    pub(super) fn news_value(&self) -> i32 {
        // 1.
        let side = self.side_call().1 as i32;
        let own = self.rd8(CALL_LEVEL) as i32;
        let mut v = if own > side { own - side } else { 0 };
        // 2.
        for (i, j) in self.opponents() {
            v += self.fresher(self.knowledge(i), self.sight(j));
        }
        // 3.
        v as u8 as i32
    }

    // ================= Calling ally (op 11) =================
    pub(super) fn ally_call_tile(&self) -> i32 {
        let allies = if self.s < 0x28 { 0..0x28 } else { 0x28..0x30 };
        let mut best = 1000;
        let mut result = NONE;
        for a in allies {
            if a == self.s || !self.alive(a) {
                continue;
            }
            if self.rd8_of(a, f::B1C4) == 0 && self.last_spotter() != self.rd8_of(a, SLOT_BYTE) {
                continue;
            }
            let ap = self.path_ap(s16(self.rd16_of(a, f::TILE) as i32));
            if ap != 0 && ap < best {
                best = ap;
                // A's tile read again after the path_ap call.
                result = self.rd16_of(a, f::TILE);
            }
        }
        result as i32
    }

    // ================= Advance tile (op 22) =================
    pub(super) fn advance_tile(&self, weak: i32) -> i32 {
        // 1.
        let mut strong: (i32, u16) = (1000, NONE);
        let mut weakest: (i32, u16) = (1000, NONE);
        // 2.
        for (i, j) in self.opponents() {
            if !self.alive(i) {
                continue;
            }
            // Skip only without a known place (sight flag and knowledge both 0); a stored 0xC41 is a tile here.
            if self.sight(j) == 0 && self.knowledge(i) == 0 {
                continue;
            }
            let t = self.known_place(i, j);
            if t == self.own_tile() {
                continue;
            }
            let ap = self.path_ap(s16(t as i32));
            if ap != 0 {
                let pair = if self.s8_of(i, f::HP) >= 15 { &mut strong } else { &mut weakest };
                if ap < pair.0 {
                    *pair = (ap, t);
                }
            }
        }
        // 3.
        let (mut best, mut result) = (1000, NONE);
        if strong.1 != NONE {
            (best, result) = strong;
        } else if weak as u8 != 0 && weakest.1 != NONE {
            (best, result) = weakest;
        }
        // 4.
        let ct = self.rd16(CALL_TILE);
        if s16(ct as i32) != NONE as i32 {
            if ct == self.own_tile() {
                self.wr16(CALL_TILE, NONE);
                self.wr8(CALL_LEVEL, 0);
            } else {
                let ap = self.path_ap(s16(ct as i32));
                if ap != 0 && ap < best {
                    best = ap;
                    result = ct;
                }
            }
        }
        // 5.
        let t = self.side_call().0;
        if s16(t as i32) != NONE as i32 && t != self.own_tile() {
            let ap = self.path_ap(s16(t as i32));
            if ap != 0 && ap < best {
                result = t;
            }
        }
        // 6.
        result as i32
    }

    /// Run library op `opc` for this soldier with the host-call argument list; the raw 32-bit result as the host
    /// would give it. For noise_tile the loudness goes to `out` (i32, little-endian, as far as it fits).
    pub(super) fn lib_op(&self, opc: i32, args: &[i32], out: &mut [u8]) -> u32 {
        let a1 = args.get(1).copied().unwrap_or(0);
        match opc {
            // soldier_update.md Path ops: op 1 idle, 29 walk_to, 40 path_ap (sneak, byte177, alt all 0), 41 find_path.
            op::IDLE => {
                self.idle();
                0
            }
            op::WALK_TO => {
                self.walk_to(s16(a1));
                0
            }
            op::PATH_AP => self.path_ap_op(s16(a1), 0, 0, 0) as u32,
            op::FIND_PATH => self.find_path_op(s16(a1), args.get(2).copied().unwrap_or(0)) as u32,
            op::NOISE_TILE => {
                let (t, loud) = self.noise_tile();
                let b = (loud as i16 as i32).to_le_bytes();
                let n = out.len().min(4);
                out[..n].copy_from_slice(&b[..n]);
                t as u32
            }
            // combat.md Doors: op 70 door_fx (slot, tile, byte task).
            op::DOOR_FX => {
                self.door_fx(a1, args.get(2).copied().unwrap_or(0) as u8);
                0
            }
            // combat.md Reload: op 190 reloaded (slot); op 102 reload_check is the same procedure.
            op::RELOADED | op::RELOAD_CHECK => self.reload() as u32,
            // ai.md Noise: op 211 hears (slot, tile, byte strength, byte kind), 212 noise_int (slot).
            op::HEARS => {
                self.hears(a1, args.get(2).copied().unwrap_or(0) as u8, args.get(3).copied().unwrap_or(0) as u8);
                0
            }
            op::NOISE_INT => {
                self.noise_interrupt();
                0
            }
            op::FIND_DRY_TILE => self.find_dry_tile() as u32,
            op::PATROL_TILE => self.patrol_tile() as u32,
            op::BUDDY_TILE => self.buddy_tile() as u32,
            op::NEWS_VALUE => self.news_value() as u32,
            op::APPROACH_TILE => self.approach_tile(a1) as u32,
            op::ALLY_CALL_TILE => self.ally_call_tile() as u32,
            op::CAN_STAND_AT => self.can_stand_at(a1) as u32,
            op::ADVANCE_TILE => self.advance_tile(a1) as u32,
            op::TARGET_TILE => self.target_tile() as u32,
            op::OPP_STRENGTH => self.situation_score() as u32,
            op::RETREAT_TILE => self.retreat_tile() as u32,
            op::SEARCH_TILE => self.search_tile(a1) as u32,
            op::FIRE_LINE => self.line_of_fire(a1, args.get(2).copied().unwrap_or(0) as u8) as u32,
            op::SIGHT_LINE => self.sight_line(args) as u32,
            op::GUN_OPTION | op::THROW_OPTION | op::KNIFE_OPTION => {
                let r = match opc {
                    op::GUN_OPTION => self.gun_option(),
                    op::THROW_OPTION => self.throw_option(),
                    _ => self.knife_option(),
                };
                let b = r.bytes();
                let n = out.len().min(10);
                out[..n].copy_from_slice(&b[..n]);
                0
            }
            op::ANIM_TURN => {
                self.anim_turn_done(a1 as u8);
                0
            }
            op::ANIM_FRAME => {
                self.anim_frame();
                0
            }
            // sector_start.md batch 13: Shot AP (slot, tile).
            op::SHOT_AP => self.shot_ap(a1) as u32,
            // sector_start.md batch 12: Action AP (slot, tile, byte flag).
            op::ACTION_AP => self.action_ap(a1, args.get(2).copied().unwrap_or(0) as u8) as u32,
            // sector_start.md batch 9: Replace soldier and Reserve AP run the sector code on this slot.
            op::REPLACE => {
                crate::sector::ja1_replace_soldier(self.w, self.h, self.slot());
                0
            }
            // combat.md End of a task (op 50, byte flag) and Walking cost (op 13).
            op::END_TASK => {
                self.end_task(a1 & 0xFF);
                0
            }
            op::MOVE_AP => self.move_ap_cost() as u32,
            // ai.md Soldier busy (op 27: slot).
            op::BUSY => self.busy() as u32,
            // ai.md Swap hands (op 16: slot, byte a, byte b).
            op::SWAP_HAND => {
                self.swap_hand(a1 as i8, args.get(2).copied().unwrap_or(0) as i8);
                0
            }
            // combat.md Use cost, exact steps (op 121: slot, spot, tile, byte g, byte kind).
            op::USE_COST => {
                let g = |k: usize| args.get(k).copied().unwrap_or(0);
                self.use_cost(s16(g(1)), s16(g(2)), g(3) as u8, g(4) as u8) as u32
            }
            // ai.md Turning toward a direction (op 28: byte d, int ticks).
            op::TURN => self.turn_to(a1 as u8, args.get(2).copied().unwrap_or(0)) as u32,
            op::RESERVE_AP => {
                crate::sector::ja1_reserve_ap(self.w, self.h, self.slot());
                0
            }
            op::ANIM_STEP => {
                self.anim_step();
                0
            }
            // ai.md Animation end (op 83; the byte argument is not used).
            op::ANIM_END => {
                self.anim_end();
                0
            }
            // soldier_update.md: op 97 anim_next (ai.md Next word), op 98 walk_blocked, op 119 ai_think (One tick).
            op::ANIM_NEXT => {
                self.next_word();
                0
            }
            op::WALK_BLOCKED => {
                let a2 = args.get(2).copied().unwrap_or(0);
                self.walk_blocked(s16(a1), a2 as u8);
                0
            }
            // soldier_update.md: op 111 enemy_step (Enemy step seen), op 117 step_noise (Step noise).
            op::ENEMY_STEP => {
                self.enemy_step_seen();
                0
            }
            op::STEP_NOISE => {
                self.step_noise();
                0
            }
            // soldier_update.md: op 122 throw_follow (Throw follow-up).
            op::THROW_FOLLOW => {
                self.throw_follow();
                0
            }
            // select.md: op 120 low_ap_stop = Select merc (int slot, int force, int quote).
            op::LOW_AP_STOP => {
                let a2 = args.get(2).copied().unwrap_or(0);
                self.select_merc(args.first().copied().unwrap_or(0), a1 as u8, a2 as u8) as u32
            }
            op::AI_THINK => {
                self.tick();
                0
            }
            // soldier_update.md: op 53 stop_anim (Stop animation), 137 active_frame, 138 soldier_frame (One frame).
            op::STOP_ANIM => {
                self.stop_anim(a1 as u8);
                0
            }
            // soldier_update.md Merc view: op 62 reveal_view (slot, byte facing, byte range, byte think).
            op::REVEAL_VIEW => {
                let a2 = args.get(2).copied().unwrap_or(0);
                let a3 = args.get(3).copied().unwrap_or(0);
                self.merc_view(a1 as u8, a2 as u8, a3 as u8);
                0
            }
            // ai.md Sightings as steps: op 35 perceive (slot, int age), 34 share_sightings (slot, int flag), 115
            // sight_check (slot, slot), 63 enemy_turned, 64 guard_turned (slot).
            op::PERCEIVE => {
                self.perceive(a1 as u8);
                0
            }
            op::SHARE_SIGHTINGS => {
                self.share_sightings(a1 as u8);
                0
            }
            op::SIGHT_CHECK => self.sight_check((a1 as u32).min(0xFFFF) as usize) as u32,
            op::ENEMY_TURNED => {
                self.enemy_turned();
                0
            }
            op::GUARD_TURNED => {
                self.guard_turned();
                0
            }
            op::ACTIVE_FRAME => {
                self.active_frame();
                0
            }
            op::SOLDIER_FRAME => {
                self.soldier_frame();
                0
            }
            // soldier_update.md Move order: op 140 (slot, tile, marker, sneak, byte177, alt, confirm, squad); the
            // library ignores confirm.
            op::MOVE_ORDER => {
                let g = |k: usize| args.get(k).copied().unwrap_or(0) as u8;
                self.move_order(s16(a1), g(2), g(3), g(4), g(5), g(7)) as u32
            }
            // turns.md: op 141 start_turn (slot), op 31 spend_ap (slot, amount, tire).
            op::START_TURN => {
                self.start_turn();
                0
            }
            op::SPEND_AP => {
                self.spend_ap(a1, args.get(2).copied().unwrap_or(0));
                0
            }
            // combat.md First aid: op 151 aid_tick (slot), op 191 first_aid (slot, tile, byte task, byte b).
            op::AID_TICK => {
                self.aid_tick();
                0
            }
            op::FIRST_AID => self.first_aid_start(a1, args.get(2).copied().unwrap_or(0) as u8),
            // turns.md Rest on spare AP: op 152 turn_rest (slot).
            op::TURN_REST => {
                self.turn_rest();
                0
            }
            // shooting.md: op 81 launch, 136 projectile_step, 39 fire (slot).
            op::LAUNCH => {
                self.launch();
                0
            }
            op::PROJECTILE_STEP => {
                self.projectile_step();
                0
            }
            op::FIRE => {
                self.fire_now();
                0
            }
            // shooting.md batch 2: op 110 take_damage (slot, height, damage, kind, attacker, tile).
            op::TAKE_DAMAGE => {
                let g = |k: usize| args.get(k).copied().unwrap_or(0);
                self.take_damage(g(1), g(2), g(3) as u8, g(4) as u8, g(5) as u16);
                0
            }
            // shooting.md batch 3: op 175 bullet_damage (slot), 180 hit_location (slot, height, damage), 173
            // shot_alert (slot, byte flag), 182 interrupt_check (slot, opponent, byte old).
            op::BULLET_DAMAGE => self.bullet_damage() as u32,
            op::HIT_LOCATION => self.hit_location(a1, args.get(2).copied().unwrap_or(0)) as u32,
            op::SHOT_ALERT => {
                self.shot_alert(a1 as u8);
                0
            }
            op::ASK_MERC_INTERRUPT => self.merc_contest((a1 as u32).min(0xFFFF) as usize),
            op::ASK_INTERRUPT => self.enemy_contest((a1 as u32).min(0xFFFF) as usize),
            op::INTERRUPT_CHECK => {
                let o = (a1 as u32).min(0xFFFF) as usize;
                self.interrupt_check(o, args.get(2).copied().unwrap_or(0) as u8);
                0
            }
            // shooting.md batch 4: op 78 kill (slot, quiet), 82 merc_dies (slot, quiet).
            op::KILL => {
                self.kill(a1);
                0
            }
            op::MERC_DIES => {
                self.merc_dies(a1);
                0
            }
            // shooting.md batch 6: op 77 melee_hit (slot).
            op::MELEE_HIT => {
                self.melee_hit();
                0
            }
            // combat.md Getting up after a collapse: op 76 revive_roll (slot, byte 0).
            op::REVIVE_ROLL => {
                self.revive_roll();
                0
            }
            // shooting.md batch 5: op 79 start_attack (slot, tile), 80 attack_go (slot, tile not used), 46
            // chance_to_hit (slot, tile, byte aim).
            op::START_ATTACK => {
                self.start_attack(a1);
                0
            }
            op::ATTACK_GO => {
                self.attack_go();
                0
            }
            // click.md: op 30 click (slot, tile, byte item), 15 attack_ready (slot, tile), 14 reach_ap (slot, tile,
            // byte flag), 68 gun_jam (slot).
            op::CLICK => self.click(a1, args.get(2).copied().unwrap_or(0)) as u32,
            op::ATTACK_READY => self.attack_ready_op(a1) as u32,
            op::REACH_AP => self.reach_tile(a1, args.get(2).copied().unwrap_or(0) as u8) as u32,
            op::GUN_JAM => {
                self.gun_jam();
                0
            }
            op::CHANCE_TO_HIT => self.chance_to_hit(a1, args.get(2).copied().unwrap_or(0) as u8) as u32,
            op::SET_ANIM => {
                // args: slot, int code (low 16 bits), byte keep, byte reset.
                let a2 = args.get(2).copied().unwrap_or(0);
                let a3 = args.get(3).copied().unwrap_or(0);
                self.anim_set(a1 as u16, a2 as u8, a3 as u8);
                0
            }
            op::KNOCKDOWN_ANIM => {
                // shooting.md: op 181 knockdown_anim (slot, byte next 0x28, byte cur 0x2B, byte 4): QueueTwoAnims
                // 0x17F5C queues cur then next and clears +0x1FE (the fourth argument is not used).
                let next = args.get(1).copied().unwrap_or(0) as u16;
                let cur = args.get(2).copied().unwrap_or(0) as u16;
                self.queue_two_anims(next, cur);
                0
            }
            _ => 0,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn step_edges() {
        assert_eq!(step(100, 1), 101);
        assert_eq!(step(0, -56), 0); // n < 0
        assert_eq!(step(56, -1), 56); // column 0 (t <= 56): n = 55 has column 55
        assert_eq!(step(112, -1), 112); // column 0 -> column 55 on the row above
        assert_eq!(step(111, 1), 111); // column 55 -> column 0
        assert_eq!(step(0xC40 - 1, 1), 0xC40 - 1); // t >= 0xC08 and n >= 0xC40
        assert_eq!(step(200, -57), 143);
    }
    #[test]
    fn distance() {
        assert_eq!(world_dist(0, 0, 16, 0), 15);
        assert_eq!(world_dist(0, 0, 0, 14), 15);
        assert_eq!(world_dist(0, 0, 16, 14), 21);
        assert_eq!(isqrt(15), 3);
        assert_eq!(isqrt(16), 4);
    }
    #[test]
    fn parts() {
        assert_eq!((row(-1), column(-1)), (0, -1));
        assert_eq!((row(-57), column(-57)), (-1, -1));
        assert_eq!((px(57), py(57)), (21, 19));
    }
}

#[cfg(test)]
mod guard_tests {
    use super::*;

    /// Distance 0x1FD28 in the original (Unicorn, ./t emu 0x1FD28 0 0 X Y, 2026-11-18): (2,1) -> 1, (1,1) -> 1,
    /// (16,14) -> 21, (5,5) -> 6, (100,3) -> 93. (2,1) is the dx² + dy² = 2 case.
    #[test]
    fn world_dist_original_values() {
        let d = |x: i32, y: i32| world_dist(0, 0, x, y);
        assert_eq!([d(2, 1), d(1, 1), d(16, 14), d(5, 5), d(100, 3)], [1, 1, 21, 6, 93]);
    }

    #[test]
    fn guard_buf8_range() {
        // Port rule (SPEC GAP): an index outside the table or a null table reads 0; the byte past the end is 9.
        let buf = [7u8, 1, 2, 3, 4, 9];
        let p = buf[1..].as_ptr();
        assert_eq!(buf8(p, 4, 3), 4);
        assert_eq!(buf8(p, 4, 4), 0);
        assert_eq!(buf8(std::ptr::null(), 4, 4), 0);
        assert_eq!(buf8(p, 4, -1), 0);
    }
}
