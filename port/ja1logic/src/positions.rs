//! Slice 2b part 2, batch 2: "Positions" (Box, Retreat tile op 21, Position score, Search position op 12) and
//! "Sector value" (op 9) of docs/spec/ai.md; C interface in docs/spec/port_api.md.
//!
//! A child module of `ai`, like `search` and `strength`. Inner calls: can_stand_at (18) is a library op through
//! `Ai::call`; direction (6), tile_dist (7), path_ap (40), fire_line (42), connected_home (43) and quest_check
//! (44) are host calls. Arithmetic is 32-bit signed with C division (Rust `/` = `cdiv`).
use super::search::{leash_of, px, py, s16, world_dist};
use super::{f, op, Ai, NONE, SECTORS};

const SEES: usize = 0x13A;
pub(crate) const STORY_LEN: usize = 32;
const PLANTS_LEN: usize = 4 * 12;
const CRATES_LEN: usize = 4 * 4;
const NEVER: i32 = -100;

/// Byte i of a world buffer of `len` bytes; null or out of range reads as 0.
fn buf8(p: *const u8, len: usize, i: usize) -> u8 {
    if p.is_null() || i >= len {
        return 0;
    }
    unsafe { p.add(i).read() }
}

/// dist(a, b) for two tiles = s16(D(px(a), py(a), px(b), py(b))).
fn dist(a: i32, b: i32) -> i32 {
    s16(world_dist(px(a), py(a), px(b), py(b)))
}

/// The Box: (left, right, up, down), each kept as a byte.
#[derive(Clone, Copy)]
struct BoxSides {
    left: u8,
    right: u8,
    up: u8,
    down: u8,
}

/// A record's tile, px and py as saved by Position score step 1.
struct Saved {
    slot: usize,
    tile: u16,
    px: u16,
    py: u16,
}

impl Ai {
    fn wr8_of(&self, slot: usize, off: usize, v: u8) {
        if slot >= super::SLOTS || off >= super::REC {
            return;
        }
        unsafe {
            let p = (*self.w).soldiers;
            if !p.is_null() {
                p.add(slot * super::REC + off).write(v);
            }
        }
    }
    fn wr16_of(&self, slot: usize, off: usize, v: u16) {
        self.wr8_of(slot, off, v as u8);
        self.wr8_of(slot, off + 1, (v >> 8) as u8);
    }
    fn r16_of(&self, slot: usize, off: usize) -> u16 {
        self.rd8_of(slot, off) as u16 | (self.rd8_of(slot, off + 1) as u16) << 8
    }
    fn counts_p(&self, r: usize) -> bool {
        self.rd8_of(r, f::IN_CUR_SECTOR) != 0 && self.rd8_of(r, f::IN_PLAY) != 0 && self.s8_of(r, f::HP) >= 15
    }
    /// Opponents here: for me ≥ 0x28 slots 0..15 (j = slot), else 0x28..0x2F (j = slot − 0x28).
    fn opponents_p(&self) -> Vec<(usize, usize)> {
        if self.s >= 0x28 {
            (0..16).map(|i| (i, i)).collect()
        } else {
            (0x28..0x30).map(|i| (i, i - 0x28)).collect()
        }
    }
    /// "Known": u8 sight flag of S on the opponent ≠ 0 or u8 knowledge[slot] ≠ 0.
    fn known_p(&self, i: usize, j: usize) -> bool {
        self.rd8(SEES + j) != 0 || self.knowledge(i) != 0
    }
    /// power(O) = s16(Soldier power of O against my, F = 0).
    fn power_p(&self, o: usize, my: i32) -> i32 {
        s16(self.soldier_power(o, my, 0))
    }
    fn ap_ok(&self, ap: i32) -> bool {
        !(ap == 0 || (self.s8(f::ALERT) == 3 && ap > self.ap()))
    }

    // ================= Box =================
    fn box_sides(&self, my: i32) -> BoxSides {
        // 1.
        // SPEC GAP: "r = 3, 5, 7 for difficulty 0, 1, 2"; a difficulty above 2 uses 7 (as the decisions do).
        let mut r: i32 = match self.w_difficulty() { 0 => 3, 1 => 5, _ => 7 };
        if self.s8(f::ALERT) == 3 {
            let half = (self.ap() / 2) as u8 as i32;
            r = r.min(half);
        }
        // 2.
        let row = my / 56;
        let col = my - row * 56;
        BoxSides {
            left: r.min(col) as u8,
            right: r.min(55 - col) as u8,
            up: r.min(row) as u8,
            down: r.min(55 - row) as u8,
        }
    }
    /// Box step 3: the candidates in scan order.
    fn box_scan(my: i32, b: BoxSides) -> Vec<i32> {
        let mut v = vec![];
        let (y0, y1) = ((b.up.wrapping_neg()) as i8 as i32, b.down as i8 as i32);
        let (x0, x1) = ((b.left.wrapping_neg()) as i8 as i32, b.right as i8 as i32);
        for dy in y0..=y1 {
            for dx in x0..=x1 {
                v.push(s16(my + dx + dy * 56));
            }
        }
        v
    }

    // ================= Retreat tile (op 21) =================
    pub(super) fn retreat_tile(&self) -> i32 {
        // my (s16 tile of S) is read once at the start and kept (the spec means it; confirmed).
        let my = s16(self.rd16(f::TILE) as i32);
        // 1.
        let mut near = 1500;
        let mut k: Vec<u16> = vec![];
        // 2.
        for (o, j) in self.opponents_p() {
            if !(self.counts_p(o) && self.known_p(o, j)) {
                continue;
            }
            if self.power_p(o, my) == -999 {
                continue;
            }
            let t = self.known_place(o, j);
            let (x, y) = (s16(self.rd16(f::X2) as i32), s16(self.rd16(f::Y2) as i32));
            let d = s16(world_dist(x, y, px(t as i16 as i32), py(t as i16 as i32)));
            near = near.min(d);
            k.push(t);
        }
        // 3.
        if k.is_empty() {
            return NONE as i32;
        }
        // 4.
        let (mut best, mut result) = (near, NONE);
        for c in Ai::box_scan(my, self.box_sides(my)) {
            if !self.stand(c) {
                continue;
            }
            if !self.ap_ok(self.path_ap(c)) {
                continue;
            }
            let far = k.iter().fold(1500, |far, &t| far.min(dist(c, t as i16 as i32)));
            if far > best {
                best = far;
                result = c as u16;
            }
        }
        // 5.
        result as i32
    }

    // ================= Position score =================
    /// S (slot m) on tile a against O (slot o) on tile b, power p, distance d.
    fn position_score(&self, m: usize, a: i32, o: usize, b: i32, p: i32, d: i32) -> i32 {
        // 1.
        let mut saved: Vec<Saved> = vec![];
        for (slot, t) in [(m, a), (o, b)] {
            let old = self.r16_of(slot, f::TILE);
            if t as u16 == old {
                continue;
            }
            saved.push(Saved { slot, tile: old, px: self.r16_of(slot, f::X2), py: self.r16_of(slot, f::Y2) });
            let st = t as i16 as i32;
            self.wr16_of(slot, f::TILE, t as u16);
            self.wr16_of(slot, f::X2, px(st) as u16);
            self.wr16_of(slot, f::Y2, py(st) as u16);
        }
        // 2.
        let theirs = 100i32.wrapping_sub(self.call(op::FIRE_LINE, &[o as i32, s16(a), 1]) as i32) as u8;
        // 3.
        let raw = self.call(op::FIRE_LINE, &[m as i32, s16(b), 1]) as u8;
        let mine: u8 = if (raw as i8) < 25 { 100 } else { 100u8.wrapping_sub(raw) };
        // 4. First S, then O; an old tile of NONE keeps the new values.
        for sv in &saved {
            if sv.tile != NONE {
                self.wr16_of(sv.slot, f::TILE, sv.tile);
                self.wr16_of(sv.slot, f::X2, sv.px);
                self.wr16_of(sv.slot, f::Y2, sv.py);
            }
        }
        // 5.
        let diff = theirs.wrapping_sub(mine) as i8 as i32;
        (300 - s16(d)).wrapping_mul(s16(p).wrapping_mul(diff))
    }

    // ================= Search position (op 12) =================
    pub(super) fn search_tile(&self, mode: i32) -> i32 {
        let mode = mode as u8;
        let me = self.s;
        // my (s16 tile of S) is read once at the start and kept (the spec means it; confirmed).
        let my = s16(self.rd16(f::TILE) as i32);
        // 1.
        let (mut near, mut near_tile, mut cur) = (1500, NONE, 0i32);
        let mut targets: Vec<(u16, usize, i32)> = vec![];
        // 2.
        for (o, j) in self.opponents_p() {
            if !(self.counts_p(o) && self.known_p(o, j)) {
                continue;
            }
            let t = self.known_place(o, j);
            let d = dist(my, t as i16 as i32);
            if d > 300 {
                continue;
            }
            let p = self.power_p(o, my);
            if p == -999 {
                continue;
            }
            targets.push((t, o, p));
            cur = cur.wrapping_add(self.position_score(me, my, o, t as i32, p, d));
            if d < near {
                near = d;
                near_tile = t;
            }
        }
        // 3.
        if targets.is_empty() {
            return NONE as i32;
        }
        // 4.
        let mut b = self.box_sides(my);
        let r = leash_of(self.orders());
        if self.s8(f::ALERT) != 3 && r < 99 {
            let post = self.post();
            let over = |t: i32| -> i32 { (self.tile_dist_i(s16(t), post) as u8) as i32 - r };
            let v = over(my - b.left as i8 as i32);
            if v > 0 {
                b.left = (b.left as i32 - v) as u8;
            }
            let v = over(my + b.right as i8 as i32);
            if v > 0 {
                b.right = (b.right as i32 - v) as u8;
            }
            let v = over(my - (b.up as i8 as i32) * 56);
            if v > 0 {
                b.up = (b.up as i32 - v) as u8;
            }
            let v = over(my + (b.down as i8 as i32) * 56);
            if v > 0 {
                b.down = (b.down as i32 - v) as u8;
            }
        }
        // 5.
        let (mut best, mut result) = (cur, NONE);
        for c in Ai::box_scan(my, b) {
            if !self.stand(c) {
                continue;
            }
            if mode != 0 {
                let dm = dist(c, near_tile as i16 as i32);
                if (mode == 1 && dm < near) || (mode == 2 && dm > near) {
                    continue;
                }
            }
            if !self.ap_ok(self.path_ap(c)) {
                continue;
            }
            let mut total = 0i32;
            for &(t, o, p) in &targets {
                let st = t as i16 as i32;
                total = total.wrapping_add(self.position_score(me, c, o, t as i32, p, dist(c, st)));
            }
            if total > best {
                best = total;
                result = c as u16;
            }
        }
        // 6.
        if result == NONE {
            return NONE as i32;
        }
        let m = best.wrapping_abs().max(cur.wrapping_abs());
        if m <= 0 || s16(best.wrapping_sub(cur).wrapping_mul(100) / m) < 20 {
            return NONE as i32;
        }
        result as i32
    }

    // ================= Sector value (op 9) =================
    pub(crate) fn story(&self, k: usize) -> u8 {
        buf8(unsafe { (*self.w).story }, STORY_LEN, k)
    }
    /// Byte i of sector n's sectorState row.
    fn sector_row(&self, n: u8, i: usize) -> u8 {
        // SPEC GAP: a sector of 80 and up (past the table) reads as all-zero bytes.
        if n as usize >= SECTORS {
            return 0;
        }
        buf8(unsafe { (*self.w).sector_state }, SECTORS * 9, n as usize * 9 + i)
    }
    fn w_day(&self) -> i32 {
        unsafe { (*self.w).day }
    }

    pub(super) fn sector_value(&self, n: u8, flag: u8) -> i32 {
        let ni = n as i32;
        // 1.
        if self.sector_row(n, 8) != 0 {
            return NEVER;
        }
        // 2.
        // SPEC GAP: "homeDay = 6, 4, 2 for difficulty 0, 1, 2"; a difficulty above 2 uses 2 (as the decisions do).
        let home_day = match self.w_difficulty() { 0 => 6, 1 => 4, _ => 2 };
        if n == 0x3B && home_day >= self.w_day() {
            return NEVER;
        }
        // 3.
        let c = self.story(0x09) as i8 == 2
            && self.story(0x06) as i8 == 2
            && (self.story(0x0B) as i8) < 1
            && (n == 0x11 || n == 0x1B);
        // 4.
        if n == 0x1B && self.sector_row(n, 0) & 1 != 0 && !c {
            return NEVER;
        }
        // 5.
        if n == 0x35 && self.story(0x0D) as i8 == 1 {
            return 1000;
        }
        // 6.
        if c {
            return if n == 0x1B { 1000 } else { 250 };
        }
        // 7.
        let mut v = 30 + self.sector_row(n, 6) as i32 * 5 + self.sector_row(n, 7) as i32 * 2
            + (self.sector_row(n, 3) as i32 / 3) * 3;
        // 8.
        v += (self.call(op::CONNECTED_HOME, &[ni]) as u8) as i32 * 5;
        // 9.
        let r0 = self.sector_row(n, 0);
        for bit in [0x02u8, 0x04, 0x08, 0x10] {
            if r0 & bit == 0 {
                v += 1;
            }
        }
        // 10.
        if matches!(n, 0x1C | 0x1D | 0x26 | 0x30 | 0x31) {
            v += 15;
        }
        // 11.
        if n == 5 {
            v += 20;
        }
        // 12.
        if n == 0x0B {
            let q = self.call(op::QUEST_CHECK, &[0x1C9]) as u8;
            if q != 0 {
                if self.sector_row(n, 2) != 0 {
                    return NEVER;
                }
                v += 25;
            }
        }
        // 13.
        let plants = unsafe { (*self.w).plants };
        for k in 0..4 {
            let b = 12 * k + 4;
            let p = i32::from_le_bytes([
                buf8(plants, PLANTS_LEN, b),
                buf8(plants, PLANTS_LEN, b + 1),
                buf8(plants, PLANTS_LEN, b + 2),
                buf8(plants, PLANTS_LEN, b + 3),
            ]);
            if p == ni {
                v += 25;
            }
        }
        // 14.
        let crates = unsafe { (*self.w).crates };
        let tomorrow = self.w_day().wrapping_add(1);
        if (0..4).any(|k| {
            ni == buf8(crates, CRATES_LEN, 4 * k) as i8 as i32
                && tomorrow == buf8(crates, CRATES_LEN, 4 * k + 1) as i8 as i32
        }) {
            v += 25;
        }
        // 15.
        if self.story(0x0B) as i8 == 1
            && ni == self.story(0x17) as i8 as i32
            && self.story(0x16) == 0
            && self.story(0x15) == 0
        {
            v += 50;
        }
        // 16.
        if n == 0x3B {
            v = v.wrapping_add(self.w_day());
        }
        // 17.
        if n == 0x15 {
            v += 20;
        }
        // 18.
        if n == 0 && self.sector_row(n, 2) != 0 {
            v += 100;
        }
        // 19.
        if flag == 0 {
            v = v.wrapping_sub(25);
        }
        // 20.
        v.clamp(20, 100)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn scan_order() {
        let b = BoxSides { left: 1, right: 1, up: 1, down: 0 };
        assert_eq!(Ai::box_scan(100, b), vec![43, 44, 45, 99, 100, 101]);
        assert_eq!(dist(0, 1), 15);
    }
}

#[cfg(test)]
mod guard_tests {
    use super::*;

    #[test]
    fn guard_buf8_range() {
        // Port rule (SPEC GAP): an index outside the table or a null table reads 0; the byte past the end is 9.
        let buf = [7u8, 1, 2, 3, 4, 9];
        let p = buf[1..].as_ptr();
        assert_eq!(buf8(p, 4, 3), 4);
        assert_eq!(buf8(p, 4, 4), 0);
        assert_eq!(buf8(std::ptr::null(), 4, 4), 0);
    }
}

#[cfg(test)]
mod guard_world_tests {
    use super::*;
    use super::super::{Ja1Host, Ja1World};

    #[test]
    fn guard_wr8_of_range() {
        // Port rule (SPEC GAP): a write to slot 48 and up, or past the 512-byte record, is dropped.
        let mut sol = vec![0u8; 50 * 512];
        let mut w: Box<Ja1World> = Box::new(unsafe { std::mem::zeroed() });
        w.soldiers = sol.as_mut_ptr();
        let h = Ja1Host { ctx: std::ptr::null_mut::<std::os::raw::c_void>(), random: None, call: None, host_ops: 0 };
        let ai = Ai { w: &mut *w, h: &h, s: 0 };
        ai.wr8_of(48, 0, 9);
        ai.wr8_of(0, 512, 9);
        ai.wr8_of(47, 600, 9);
        assert!(sol.iter().all(|&b| b == 0));
        ai.wr8_of(47, 511, 9);
        assert_eq!(sol[48 * 512 - 1], 9);
    }
}
