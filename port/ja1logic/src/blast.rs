//! Blasts and gas clouds (docs/spec/combat.md "Blast, exact steps"; C interface in docs/spec/port_api.md):
//! `ja1_blast`, `ja1_blast_tile` and `ja1_gas_tick` (library op 161 gas_tick). A child module of `ai`; the
//! damage, practice, direction, panel and object calls are ops through `Ai::call`. Also combat.md "Explosion, exact
//! steps": `ja1_explode` (library op 90 explode), `ja1_effect_hit` (the front-end's explosion effect record) and
//! Gas start.
use super::search::{s16, step};
use super::ledger::workable;
use super::{op, Ai, Ja1Host, Ja1World, TILES};

const NONE: i32 = 0xC41;
const ITEM_TYPES_LEN: usize = 136 * 12;
const ABSORB_LEN: usize = 288 * 2;
const NO_BREAK_LEN: usize = 10;
const GAS_LEN: usize = 5 * 6;
const CLASS_LEN: usize = 256;
const SECTOR_STATE_LEN: usize = 80 * 9;
const SECTORS_LEN: usize = 60 * 0xB1;
const STORY_LEN: usize = 43;
const BLAST_LIST_LEN: usize = 60;
const SOLDIERS_LEN: usize = 48 * 512;

// SPEC GAP: an index outside a world table reads 0 and a write there is dropped.
fn get(p: *const u8, len: usize, i: i64) -> u8 {
    if p.is_null() || !(0..len as i64).contains(&i) {
        return 0;
    }
    unsafe { p.add(i as usize).read() }
}
fn put(p: *mut u8, len: usize, i: i64, v: u8) {
    if p.is_null() || !(0..len as i64).contains(&i) {
        return;
    }
    unsafe { p.add(i as usize).write(v) }
}
fn get16(p: *const u8, len: usize, i: i64) -> u16 {
    get(p, len, i) as u16 | (get(p, len, i + 1) as u16) << 8
}
fn put16(p: *mut u8, len: usize, i: i64, v: u16) {
    put(p, len, i, v as u8);
    put(p, len, i + 1, (v >> 8) as u8);
}

/// The blast procedures' view of the world: `Ai` on slot 0 for host calls, tiles and tables.
struct B {
    w: *mut Ja1World,
    h: *const Ja1Host,
    a: Ai,
}

impl B {
    fn new(w: *mut Ja1World, h: *const Ja1Host) -> Option<B> {
        Ai::new(w, h, 0).map(|a| B { w, h, a })
    }
    fn w(&self) -> &mut Ja1World {
        unsafe { &mut *self.w }
    }
    fn item_byte(&self, i: i64) -> u8 {
        get(self.w().item_types, ITEM_TYPES_LEN, i)
    }
    fn no_break(&self) -> bool {
        get(self.w().no_break, NO_BREAK_LEN, self.w().tileset as i8 as i64) != 0
    }
    fn tile_or(&self, t: i32, i: usize, set: u8, keep: u8) {
        let p = self.w().tiles;
        if !p.is_null() && (0..TILES as i32).contains(&t) {
            unsafe {
                let q = p.add(t as usize * 8 + i);
                q.write(q.read() & keep | set);
            }
        }
    }

    // ---- Blast tile ----
    fn blast_tile(&self, c: i32, t: i32, ring: i32, kind: u32) {
        // 1.
        let (mut dmg, mut stun, mut fx, mut flag) = (0i32, 0i32, 0u8, 1u8);
        match kind {
            2 => {
                dmg = self.a.random(26) + 25;
                fx = 4;
            }
            3 => {
                dmg = self.a.random(51) + 50;
                fx = 4;
            }
            4 | 5 => {
                self.tile_or(t, 5, 0x80, 0xFF);
                stun = self.a.random(1001) + 2000;
                fx = 6;
            }
            6 | 7 => {
                stun = self.a.random(2001) + 5000;
                dmg = 1;
                fx = 6;
            }
            8 | 9 => {
                stun = self.a.random(2001) + 4000;
                dmg = self.a.random(16) + 15;
                fx = 6;
            }
            0x63 => self.tile_or(t, 5, 0, 0x7F),
            _ => {}
        }
        // 2.
        let o = self.a.tb(t, 5) & 0x7F;
        if o >= 0x30 {
            return;
        }
        let p = match Ai::new(self.w, self.h, o as i32) {
            Some(p) => p,
            None => return,
        };
        if p.rd8(0x182) == 0 || p.rd8(0x138) == 0 || p.rd8(0x183) == 0 {
            return;
        }
        // 3.
        if ring < 5 {
            dmg -= self.a.cdiv(dmg.wrapping_mul(ring).wrapping_mul(20), 100);
            stun -= self.a.cdiv(stun.wrapping_mul(ring).wrapping_mul(20), 100);
        } else {
            dmg = self.a.cdiv(dmg, 10);
            stun = self.a.cdiv(stun, 10);
        }
        dmg = dmg.max(0);
        stun = stun.max(0);
        // 4.
        let blaster = self.w().blaster;
        if (dmg != 0 || stun != 0) && blaster < 8 {
            self.a.call(op::STAT_PRACTICE, &[blaster as i32, 6, 1, 0]);
        }
        // 5.
        if p.anim() >= 4 && s16(p.rd16(0x114) as i32) != c {
            let d = self.a.call(
                op::DIRECTION,
                &[s16(p.rd16(0x116) as i32), s16(p.rd16(0x118) as i32), (c % 56) * 16 + 8, (c / 56) * 14 + 7],
            );
            p.wr8(0x1A2, d as u8);
        }
        // 6.
        if dmg != 0 && p.s8(0xBA) != 4 {
            self.a.call(op::TAKE_DAMAGE, &[o as i32, 2500, dmg, fx as i32, blaster as i32, s16(c)]);
            if fx == 4 {
                self.blast_wear(&p, dmg);
            }
        }
        // 7.
        if p.rd8(0x183) != 0 && stun != 0 && p.s8(0xBA) != 4 {
            if p.rd8(0x24) == 0x28 {
                stun = self.a.cdiv((100 - p.s8(0x26)).wrapping_mul(stun), 100);
            }
            if stun != 0 {
                p.rest(stun.wrapping_neg(), 1);
                if dmg == 0 {
                    self.a.call(op::EFFECT_SLOT, &[p.rd8(0x17A) as i32, 3]);
                    flag = 0;
                }
                if fx == 6 && (self.a.t0_class(self.a.tb(s16(p.rd16(0x114) as i32), 0)) == 3 || kind == 4) {
                    p.wr8(0x1DB, 1);
                }
                p.wr8(0x1C1, 1);
            }
        }
        // 8.
        p.wr8(0x1C4, 1);
        // 9.
        if dmg == 0 && blaster < 0x40 {
            if stun != 0 {
                p.wr8(0x1DA, blaster);
            }
            if p.s8(0x183) >= 15 {
                p.hit_reaction(blaster, flag, self.a.cdiv(stun, 300) as u8);
            }
        }
    }

    // ---- Wear amount ----
    fn wear_amount(&self, i: u8, dmg: i32) -> u8 {
        let i = i as i64;
        if self.item_byte(12 * i + 7) == 0 {
            return 0;
        }
        let absorb = |k: i64| get16(self.w().absorb, ABSORB_LEN, k) as i16 as i32;
        let mut d = dmg;
        if (0x23..=0x26).contains(&i) {
            d = (d - absorb(0x1E + 2 * i)).max(0);
        }
        if (0x2D..=0x31).contains(&i) {
            d = (d - absorb(2 * i)).max(0);
        }
        if self.item_byte(12 * i + 12) != 0 {
            d = self.a.cdiv(d, 2);
        }
        if d > 1 { self.a.random(s16(d)) as u8 } else { 0 }
    }

    // ---- Blast wear ----
    fn blast_wear(&self, p: &Ai, dmg: i32) {
        let lose = |off: usize, item: u8| {
            let v = p.rd8(off).wrapping_sub(self.wear_amount(item, dmg));
            p.wr8(off, if (v as i8) < 1 { 1 } else { v });
        };
        for e in (0..12).map(|k| 12 * k) {
            let item = p.rd8(e);
            if item != 0 {
                if p.s8(e + 2) < 0 {
                    continue;
                }
                lose(e + 2, item);
                if self.item_byte(12 * item as i64 + 6) > 1 && p.s8(e + 1) > 1 {
                    for c in 0..4 {
                        if p.s8(e + 8 + c) > 0 {
                            lose(e + 8 + c, item);
                        }
                    }
                }
            }
            let att = p.rd8(e + 3);
            if att != 0 {
                lose(e + 5, att);
            }
        }
        let me = p.rd8(0x17A);
        if me < 8 && me == self.w().selected {
            self.a.call(op::ITEM_PANEL, &[me as i32, 0, 1]);
        }
    }

    // ---- Blast ----
    fn blast(&self, c: i32, r: i32, kind: u32) {
        self.blast_tile(c, c, 0, kind);
        for d in 1..=8 {
            // 1.
            let mut lim = r;
            if s16(lim) < 1 {
                continue;
            }
            let mut side = lim.wrapping_sub(1) as u8;
            let mut cur = c;
            let mut i: u16 = 1;
            loop {
                // 2.
                let nxt = s16(step(s16(cur), self.a.dir_step(d)));
                let ok = if nxt == s16(cur) {
                    false
                } else if self.a.t1_class(self.a.tb(nxt, 1)) == 6 {
                    lim = lim.wrapping_sub(1);
                    (i as i16) <= s16(lim) as i16
                } else {
                    self.a.u_obj_block(nxt, d as u8, None) == 0
                };
                if !ok {
                    if kind == 3 && (i as i16) < 2 && self.a.tile_standable(nxt) < 0 && !self.no_break() {
                        self.a.call(op::BREAK_OBJECT, &[nxt, self.a.tb(nxt, 1) as i32, 1]);
                    }
                    break;
                }
                // 3.
                self.blast_tile(c, nxt, i as i16 as i32, kind);
                cur = nxt;
                // 4.
                if side != 0 {
                    let d2 = if d == 8 { 1 } else { d + 1 };
                    let mut f = nxt;
                    for _ in 0..side {
                        let t2 = s16(step(s16(f), self.a.dir_step(d2)));
                        if t2 != s16(f) && self.a.u_obj_block(t2, d2 as u8, None) == 0 {
                            self.blast_tile(c, t2, i as i16 as i32, kind);
                            f = t2;
                        }
                    }
                }
                // 5.
                side = side.wrapping_sub(1);
                i = i.wrapping_add(1);
                if (i as i16) > s16(lim) as i16 {
                    break;
                }
            }
        }
    }

    // ---- Explode ----
    fn explode(&self, thrower: u8, t: i32, kind: i32, hit: i32) {
        // 1.
        let kind = if kind == 10 { 3 } else { kind };
        let (size, loud, mut vol) = match kind {
            2 => (2, 25, 30),
            3 => (3, 50, 50),
            // 4..9, and the other kinds (the original reads left-over stack bytes there; combat.md Explode step 1).
            _ => (4, 10, 30),
        };
        // 2.
        let (ox, oy) = match size {
            2 => (-14, -15),
            3 => (-36, -34),
            _ => (-16, -18),
        };
        let x = (t % 56 * 16 + ox) as i16 as i32;
        let y = (t / 56 * 14 + oy) as i16 as i32;
        // 3.
        let w = self.w();
        if !rect_visible((w.view_x, w.view_y, w.view_w, w.view_h), x, y, x + 40, y + 30) {
            vol = (vol - 20) & 0xFF;
        }
        // 4.
        self.a.call(op::SFX, &[0x0C, 0x2B11, vol as u8 as i8 as i32, 1]);
        // 5.
        if size == 2 && self.a.tile_standable(t) < 0 && !self.no_break() {
            self.a.call(op::BREAK_OBJECT, &[t, self.a.tb(t, 1) as i32, 1]);
            if thrower < 8 {
                put(self.w().soldiers, SOLDIERS_LEN, thrower as i64 * 512 + 0x9D, 1);
            }
        }
        // 6.
        let (look, count) = if kind < 4 { (2, 0x15) } else { (3, 0x0A) };
        let fx = self.a.call(op::PUFF, &[t, x, y, look, count, kind as u8 as i8 as i32, hit as u8 as i8 as i32]) as u8;
        self.w().fx_id = fx;
        self.w().anim_signal = fx as i8 as i32;
        // 7.
        self.a.call(op::MAKE_NOISE, &[thrower as i32, t, 0, loud, 6]);
        // 8.
        self.w().blaster = if thrower < 0x30 { thrower } else { 0x40 };
        // 9. (BlastListRemove? 0x3B718)
        let bl = self.w().blast_list;
        for i in 0..30i64 {
            if get16(bl, BLAST_LIST_LEN, 2 * i) as i16 as i32 == t {
                put16(bl, BLAST_LIST_LEN, 2 * i, NONE as u16);
                break;
            }
        }
    }

    // ---- Gas start ----
    fn gas_start(&self, t: i32, kind: u8) -> i32 {
        let mut turns: i32 = if kind == 4 { 3 } else { 5 };
        if get(self.w().t0_class, CLASS_LEN, self.a.tb(t, 0) as i64) != 3 {
            turns -= self.a.random(2) + 1;
        }
        let g = self.w().gas_clouds;
        for k in 0..5i64 {
            if get16(g, GAS_LEN, 6 * k) as i16 as i32 == NONE {
                put16(g, GAS_LEN, 6 * k, t as u16);
                put16(g, GAS_LEN, 6 * k + 2, turns as u16);
                put16(g, GAS_LEN, 6 * k + 4, if kind == 4 { 3 } else { 5 });
                return k as i32;
            }
        }
        -1
    }

    fn rebuild_move_codes(&self) {
        let w = self.w();
        // SPEC GAP: a null tiles, t0_class, t1_class or move_codes buffer skips the rebuild.
        if w.tiles.is_null() || w.t0_class.is_null() || w.t1_class.is_null() || w.move_codes.is_null() {
            return;
        }
        unsafe {
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

    // ---- Effect hit ----
    fn effect_hit(&self, t: i32, kind: i32, power: i8) {
        // 1.
        if self.a.tb(t, 1) == 0x86 {
            self.tile_or(t, 1, 0, 0);
            self.rebuild_move_codes();
        }
        // 2.
        let obj = self.a.tb(t, 1);
        let s = self.w().cur_sector;
        if kind == 3 && (obj == 0x35 || obj == 0x36) {
            let st = self.w().sector_state;
            let i = 9 * s as i64;
            put(st, SECTOR_STATE_LEN, i + 6, get(st, SECTOR_STATE_LEN, i + 6).wrapping_sub(1));
            if obj == 0x35 {
                put(st, SECTOR_STATE_LEN, i + 8, get(st, SECTOR_STATE_LEN, i + 8).wrapping_sub(1));
            }
            let si = s as i8 as i32;
            self.a.call(op::REPLAY_EVENT, &[si, si, if obj == 0x35 { 0x56 } else { 0x55 }]);
            workable(self.w, s);
        }
        // 3.
        if (kind == 4 || kind == 5) && self.gas_start(t, kind as u8) == -1 {
            return;
        }
        // 4.
        if (kind == 2 || kind == 3) && power > 0 {
            self.a.call(op::EXPLODE_ART, &[t, (kind == 3) as i32]);
            let story = self.w().story;
            if s as i32 == get(story, STORY_LEN, 0x17) as i8 as i32 {
                for j in 0..5i64 {
                    let v = get16(self.w().sectors, SECTORS_LEN, 0xB1 * s as i64 + 0x82 + 2 * j) as i16 as i32;
                    if v == t {
                        self.a.call(op::STORY_HIT, &[]);
                    }
                }
            }
        } else {
            self.a.call(op::TAKE_GROUND_ITEM, &[t, 0]);
        }
        // 5.
        if power > 0 {
            let r = match kind {
                2 | 4 => 2,
                3 | 5 | 7 | 9 => 4,
                _ => 3,
            };
            self.blast(t, r, kind as u32);
        }
    }

    // ---- Sector tick (turns.md) ----
    fn sector_tick(&self) {
        // 1.
        let d = get(self.w().splash, 60, self.w().cur_sector as i64) as i8 as i32;
        let n = if d == 0 {
            self.a.call(op::DIV_ZERO, &[]);
            0
        } else {
            1000 / d
        };
        // 2.
        for _ in 0..n.max(0) {
            let t = self.a.random(0xC40);
            self.w().ripple_tile = t;
            let o = self.a.tb(t, 5) & 0x7F;
            if self.a.tb(t, 0) < 0x97 || self.a.tb(t, 1) != 0 || o >= 0x30 {
                continue;
            }
            let r = o as i64 * 512;
            let sol = self.w().soldiers;
            let k = get(sol, SOLDIERS_LEN, r + 0x1F3) as i64;
            if get(sol, SOLDIERS_LEN, r + 0x183) as i8 > 0 && get16(sol, SOLDIERS_LEN, r + 0x1E8 + 2 * k) < 0x3C {
                self.a.call(op::SET_ANIM, &[o as i32, 0x3D, 0, 1]);
                return;
            }
        }
    }

    // ---- Gas tick ----
    fn gas_tick(&self) {
        let g = self.w().gas_clouds;
        // SPEC GAP: a null gas_clouds buffer has no clouds (not five clouds on tile 0).
        if g.is_null() {
            return;
        }
        for k in 0..5i64 {
            let e = 6 * k;
            let tile = get16(g, GAS_LEN, e) as i16 as i32;
            if tile == NONE {
                continue;
            }
            let radius = get16(g, GAS_LEN, e + 4) as i16 as i32;
            self.blast(tile, radius, 4);
            let left = get16(g, GAS_LEN, e + 2).wrapping_sub(1);
            put16(g, GAS_LEN, e + 2, left);
            if (left as i16) <= 0 {
                let tile = get16(g, GAS_LEN, e) as i16 as i32;
                let radius = get16(g, GAS_LEN, e + 4) as i16 as i32;
                self.blast(tile, radius, 0x63);
                put16(g, GAS_LEN, e, NONE as u16);
            }
        }
    }
}

// ================= combat.md Springing a ground item, exact steps =================
const CALLS_LEN: usize = 8;
const MISSION_DONE_LEN: usize = 80;
const SILENT: [u8; 6] = [0x14, 0x16, 0x18, 0x1A, 0x1C, 0x1D];
/// Op 129 step text number of the trigger's "click" subtitle (PlayEmptyClick 0x1ABAC, `s_click_0009accd`).
const CLICK_TEXT: i32 = 6;

impl B {
    fn ge(&self, g: usize, o: usize) -> u8 {
        super::shared::ground_byte(self.w, 12 * g + o)
    }
    fn ge_put(&self, g: usize, o: usize, v: u8) {
        super::shared::ground_byte_put(self.w, 12 * g + o, v)
    }

    fn trigger_item(&self, slot: i32, tile: i32, g: u8, armed: u8) -> i32 {
        let g = g as usize;
        let rec = |o: usize| if (0..48).contains(&slot) { self.a.rd8_of(slot as usize, o) } else { 0 };
        let me = rec(0x17A);
        // 1.
        let (mut r, mut reset) = (1, false);
        let t = self.ge(g, 0);
        let click = || {
            self.play_empty_click(slot);
        };
        let spend = || {
            self.ge_put(g, 0, 0x40);
            let v = self.a.random(100);
            self.ge_put(g, 1, 1);
            self.ge_put(g, 2, (v + 1) as u8);
            self.ge_put(g, 6, 1);
        };
        // 2.
        match t {
            0x11 => {
                if (self.ge(g, 2) as i8) < 0 {
                    click();
                    self.a.call(op::EXPLODE, &[me as i32, tile, 2, 1]);
                } else {
                    r = 0;
                }
            }
            0x13 | 0x2C => {
                click();
                if t == 0x13 {
                    if (self.ge(g, 1) as i8) > 0 {
                        self.a.call(op::EFFECT, &[NONE, 0x29, self.ge(g, 1) as i32, 500, 0x41, 0]);
                    } else {
                        self.a.call(op::TIMED_EFFECT, &[0x29, 0, 500, 0x41, 0]);
                    }
                } else if rec(0xBA) as i8 == 4 {
                    self.a.call(op::STAGE_EFFECTS, &[]);
                } else {
                    self.a.call(op::TIMED_EFFECT, &[0x3F, self.ge(g, 1) as i8 as i32, 800, 0x41, 0]);
                }
                // Spent.
                if armed != 0 && me < 8 {
                    spend();
                    reset = true;
                    if self.a.tb(tile, 1) == 0x86 {
                        self.tile_or(tile, 1, 0, 0);
                        self.rebuild_move_codes();
                    }
                }
            }
            0x72 => {
                click();
                self.a.call(op::MISSION_BLAST, &[1000]);
                let w = self.w();
                w.mission_step = 0;
                w.mission_slot = 0x40;
                w.mission_target = NONE as u16;
                if armed != 0 && me < 8 {
                    spend();
                    reset = true;
                }
            }
            0x44 => {
                self.a.call(op::CALL_ENEMIES, &[tile, 0x44]);
                self.a.call(op::TAKE_PATCH, &[tile, 0x44]);
            }
            0x52 | 0x70 => {
                if t == 0x52 {
                    click();
                }
                self.a.call(op::TOGGLE_DOORS, &[(t == 0x70) as i32, self.ge(g, 2) as i32]);
                self.a.call(op::TAKE_PATCH, &[tile, t as i32]);
            }
            0x29 | 0x3F | 0x71 => {
                if armed != 0 {
                    click();
                    self.a.call(op::EXPLODE, &[me as i32, tile, 3, 1]);
                } else {
                    r = 0;
                }
            }
            _ => {
                let k = crate::sector::blast_kind(t);
                if (self.ge(g, 2) as i8) < 0 && armed != 0 {
                    if !SILENT.contains(&t) {
                        click();
                    }
                    self.a.call(op::EXPLODE, &[me as i32, tile, k as i32, 1]);
                } else {
                    r = 0;
                }
            }
        }
        // 3.
        if r == 1 {
            if reset {
                self.a.call(op::PICK_UP, &[slot, tile, 1]);
            } else {
                self.a.call(op::TAKE_GROUND_ITEM, &[tile, 0]);
            }
        }
        // 4.
        r
    }

    /// PlayEmptyClick 0x1ABAC (EAX = soldier): the click of a sprung ground item that has no effect. A soldier with
    /// +0x9C above 0 (signed) plays the click sound, then waits 50 ms - or shows the "click" subtitle for 500 ms
    /// when Subtitles (0x1163BD) is on. A merc (profile below 0x40) then takes the view, cries "BAD", plays the
    /// voice sample and waits 100 ms. The waits are the original's busy wait 0x67250 (op 37 delay).
    fn play_empty_click(&self, slot: i32) {
        let rec = |o: usize| if (0..48).contains(&slot) { self.a.rd8_of(slot as usize, o) } else { 0 };
        let profile = rec(0x179);
        if (rec(0x9C) as i8) > 0 {
            self.a.call(op::SFX, &[0x23, 0x2B11, 0x58, 1]);
            if self.w().opt_step_text == 0 {
                self.a.call(op::DELAY, &[50]);
            } else {
                self.a.call(op::STEP_TEXT, &[CLICK_TEXT]);
            }
        }
        if profile < 0x40 {
            self.a.call(op::FOCUS, &[rec(0x17A) as i32, 1]);
            self.a.call(op::CRY, &[profile as i32, -1, 0, 1, 1]);
            self.a.call(op::VOICE, &[0x2B11, 0x58, 1]);
            self.a.call(op::DELAY, &[100]);
        }
    }
}

/// Op 108 trigger_item for `Ai::dispatch`.
pub(super) fn trigger_item(w: *mut Ja1World, h: *const Ja1Host, slot: i32, tile: i32, g: u8, armed: u8) -> i32 {
    let mut r = 0;
    with(w, h, |b| r = b.trigger_item(slot, tile, g, armed));
    r
}

/// combat.md Mission blast (op 32).
pub(super) fn mission_blast(w: *mut Ja1World, h: *const Ja1Host, power: i32) {
    with(w, h, |b| {
        let cur = b.w().cur_sector;
        b.a.call(op::TIMED_EFFECT, &[if cur == 11 { 0x6E } else { 0x71 }, 0, power, 0x41, 0]);
        put(b.w().mission_done, MISSION_DONE_LEN, cur as i64, 1);
    })
}

/// combat.md Enemy call (op 340): the enemies' side call goes to the tile with level 0xC.
pub(super) fn call_enemies(w: *mut Ja1World, tile: i32) {
    if w.is_null() {
        return;
    }
    let c = unsafe { (*w).calls };
    put16(c, CALLS_LEN, 4, tile as u16);
    put(c, CALLS_LEN, 6, 0xC);
}

#[no_mangle]
pub extern "C" fn ja1_mission_blast(w: *mut Ja1World, h: *const Ja1Host, power: i32) {
    mission_blast(w, h, power)
}
#[no_mangle]
pub extern "C" fn ja1_call_enemies(w: *mut Ja1World, _h: *const Ja1Host, tile: i32, _kind: i32) {
    call_enemies(w, tile)
}
#[no_mangle]
pub extern "C" fn ja1_toggle_doors(w: *mut Ja1World, h: *const Ja1Host, mode: i32, value: i32) {
    let _ = std::panic::catch_unwind(|| super::doors::toggle_doors(w, h, mode as u8, value as u8));
}
#[no_mangle]
pub extern "C" fn ja1_trigger_item(w: *mut Ja1World, h: *const Ja1Host, slot: i32, tile: i32, g: i32, armed: i32) -> i32 {
    trigger_item(w, h, slot, tile, g as u8, armed as u8)
}

fn with(w: *mut Ja1World, h: *const Ja1Host, f: impl FnOnce(&B)) {
    if let Some(b) = B::new(w, h) {
        let _ = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| f(&b)));
    }
}

/// Op 90 explode for `Ai::dispatch`.
pub(super) fn explode(w: *mut Ja1World, h: *const Ja1Host, thrower: u8, tile: i32, kind: i32, hit: i32) {
    with(w, h, |b| b.explode(thrower, tile, kind, hit))
}

/// Op 160 sector_tick for `Ai::dispatch`.
pub(super) fn sector_tick(w: *mut Ja1World, h: *const Ja1Host) {
    with(w, h, |b| b.sector_tick())
}

/// Op 161 gas_tick for `Ai::dispatch`.
pub(super) fn gas_tick(w: *mut Ja1World, h: *const Ja1Host) {
    with(w, h, |b| b.gas_tick())
}

#[no_mangle]
pub extern "C" fn ja1_blast(w: *mut Ja1World, h: *const Ja1Host, centre: i32, radius: i32, kind: i32) {
    with(w, h, |b| b.blast(centre, radius, kind as u32))
}
#[no_mangle]
pub extern "C" fn ja1_blast_tile(w: *mut Ja1World, h: *const Ja1Host, centre: i32, tile: i32, ring: i32, kind: i32) {
    with(w, h, |b| b.blast_tile(centre, tile, ring, kind as u32))
}
#[no_mangle]
pub extern "C" fn ja1_gas_tick(w: *mut Ja1World, h: *const Ja1Host) {
    with(w, h, |b| b.gas_tick())
}
#[no_mangle]
pub extern "C" fn ja1_explode(w: *mut Ja1World, h: *const Ja1Host, thrower: i32, tile: i32, kind: i32, hit: i32) {
    with(w, h, |b| b.explode(thrower as u8, tile, kind, hit))
}
#[no_mangle]
pub extern "C" fn ja1_effect_hit(w: *mut Ja1World, h: *const Ja1Host, tile: i32, kind: i32, power: i32) {
    with(w, h, |b| b.effect_hit(tile, kind, power as u8 as i8))
}
#[no_mangle]
pub extern "C" fn ja1_sector_tick(w: *mut Ja1World, h: *const Ja1Host) {
    with(w, h, |b| b.sector_tick())
}

/// RectVisible? 0x1FBDC: false when the rectangle lies wholly outside the view (x, y, width, height).
pub fn rect_visible(view: (i32, i32, i32, i32), x0: i32, y0: i32, x1: i32, y1: i32) -> bool {
    let (vx, vy, vw, vh) = view;
    !(x1 < vx || y1 < vy || x0 > vx.wrapping_add(vw).wrapping_sub(1) || y0 > vy.wrapping_add(vh).wrapping_sub(1))
}

/// `rect_visible` for rect_diff.
#[no_mangle]
pub extern "C" fn ja1_rect_visible(vx: i32, vy: i32, vw: i32, vh: i32, x0: i32, y0: i32, x1: i32, y1: i32) -> i32 {
    rect_visible((vx, vy, vw, vh), x0, y0, x1, y1) as i32
}

#[cfg(test)]
mod guard_tests {
    use super::*;

    #[test]
    fn guard_get_range() {
        // Port rule (SPEC GAP): an index outside the table or a null table reads 0; the byte past the end is 9.
        let buf = [7u8, 1, 2, 3, 4, 9];
        let p = buf[1..].as_ptr();
        assert_eq!(get(p, 4, 3), 4);
        assert_eq!(get(p, 4, 4), 0);
        assert_eq!(get(std::ptr::null(), 4, 4), 0);
        assert_eq!(get(p, 4, -1), 0);
    }

    #[test]
    fn guard_put_range() {
        // Port rule (SPEC GAP): a write outside the table or to a null table is dropped.
        let mut buf = [0u8; 6];
        let p = unsafe { buf.as_mut_ptr().add(1) };
        put(p, 4, 3, 5);
        put(p, 4, 4, 6);
        put(std::ptr::null_mut(), 4, 4, 6);
        put(p, 4, -1, 8);
        assert_eq!(buf, [0, 0, 0, 0, 5, 0]);
    }
}
