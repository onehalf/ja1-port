//! Slice 2c part 1: attack options (docs/spec/ai.md "Attack options": Expected damage, Gun option op 24, Melee hit
//! chance, Knife damage, Knife option op 26; Throw damage, Throw AP, Throw chance, Throw option op 25; C interface in docs/spec/port_api.md).
//!
//! A child module of `ai`, like `search`. Inner calls are host calls: tile_dist (7), fire_line (42), chance_to_hit
//! (46), and direction (6) / fire_line inside Soldier power. action_ap (17) and shot_ap (45) are library ops from
//! sector_start.md batches 12 and 13 on (Action AP and Shot AP, at the end of this file).
//! Arithmetic is 32-bit signed with C division (Rust `/` = `cdiv`).
use super::search::{px, py, s16, world_dist};
use super::{f, op, run, Ai, Ja1Host, Ja1World, ITEM_TYPES, NONE, NO_SLOT};

const WEAPONS_LEN: usize = 256 * 6;
const ABSORB_LEN: usize = 288 * 2;
const SEES: usize = 0x13A;
const HELMET: usize = 0x018;
const BODY_ARMOUR: usize = 0x03C;
const VEST_POCKET: usize = 0x048;
const WOUNDS: usize = 0x185;
const AGILITY: usize = 0x18B;
const DEXTERITY: usize = 0x18C;
const LEVEL: usize = 0x192;
const B1B9: usize = 0x1B9;
const MARKSMANSHIP: usize = 0x191;
const ITEM3: usize = 0x024; // inventory entry 3: byte 0 = item type, byte 2 = condition

/// The option record (port_api.md "Option record").
#[derive(Clone, Copy)]
pub(super) struct Rec {
    valid: u8,
    slot: u8,
    aim: u8,
    chance: u8,
    score: i32,
    tile: u16,
}
impl Rec {
    fn empty() -> Rec {
        Rec { valid: 0, slot: NO_SLOT, aim: 0, chance: 0, score: 0, tile: NONE }
    }
    pub(super) fn bytes(&self) -> [u8; 10] {
        let mut b = [0u8; 10];
        b[0] = self.valid;
        b[1] = self.slot;
        b[2] = self.aim;
        b[3] = self.chance;
        b[4..8].copy_from_slice(&self.score.to_le_bytes());
        b[8..10].copy_from_slice(&self.tile.to_le_bytes());
        b
    }
}

/// Step i of both options: does a new option (chance, score) replace the record?
fn replaces(rec: &Rec, chance: i32, score: i32) -> bool {
    let prev = rec.chance as i32;
    if chance <= prev {
        return false;
    }
    !(prev != 0 && s16(chance * 100 / prev - 100) < 50 && score < rec.score)
}

impl Ai {
    fn u8r(&self, r: usize, off: usize) -> i32 {
        self.rd8_of(r, off) as i32
    }
    fn s8r(&self, r: usize, off: usize) -> i32 {
        self.rd8_of(r, off) as i8 as i32
    }
    fn s16r(&self, r: usize, off: usize) -> i32 {
        s16(self.rd8_of(r, off) as i32 | (self.rd8_of(r, off + 1) as i32) << 8)
    }
    /// range(t) = byte 11 of item type t.
    pub(super) fn range(&self, t: u8) -> i32 {
        // SPEC GAP: an item number of 136 and up (past the item table) or a null table has range 0.
        let p = unsafe { (*self.w).item_types };
        if p.is_null() || t as usize >= ITEM_TYPES {
            return 0;
        }
        unsafe { p.add(t as usize * 12 + 11).read() as i32 }
    }
    /// b5(t) = byte 5 of weapons entry t.
    fn b5(&self, t: u8) -> i32 {
        // SPEC GAP: the signedness of b5 is not stated; it is read as u8.
        let p = unsafe { (*self.w).weapons };
        if p.is_null() {
            return 0;
        }
        let i = t as usize * 6 + 5;
        if i >= WEAPONS_LEN { 0 } else { unsafe { p.add(i).read() as i32 } }
    }
    /// absorb[i] (s16).
    fn absorb(&self, i: usize) -> i32 {
        let p = unsafe { (*self.w).absorb };
        if p.is_null() || 2 * i + 1 >= ABSORB_LEN {
            return 0;
        }
        unsafe { (p.add(2 * i).read() as u16 | (p.add(2 * i + 1).read() as u16) << 8) as i16 as i32 }
    }
    fn alive_a(&self, o: usize) -> bool {
        self.rd8_of(o, f::IN_CUR_SECTOR) != 0 && self.rd8_of(o, f::IN_PLAY) != 0 && self.rd8_of(o, f::HP) != 0
    }
    /// Opponents (slot, index j) by aiType, and `own` for the knife option's reverse sight flag.
    fn opponents_a(&self) -> (Vec<(usize, usize)>, usize) {
        if self.s8(f::AI_TYPE) >= 3 {
            ((0..0x28).map(|i| (i, i)).collect(), 0x28)
        } else {
            ((0x28..0x30).map(|i| (i, i - 0x28)).collect(), 0)
        }
    }

    // ================= Expected damage =================
    pub(super) fn expected_damage(&self, o: usize, c: u8) -> i32 {
        let me = self.s;
        // 1.
        let d = s16(world_dist(
            self.s16r(me, f::X2),
            self.s16r(me, f::Y2),
            self.s16r(o, f::X2),
            self.s16r(o, f::Y2),
        ));
        // 2.
        let t = self.rd8(f::ITEMS);
        // Range 0: div_zero (op 47), quotient 0.
        let q = self.cdiv(d * 50, s16(self.range(t) * 15));
        // 3.
        let base = (self.b5(t) * (100 - s16(q) + c as i32 / 4) / 100) as u8 as i32;
        // 4. Fan fix F43 (#70 row 93; 1.15m ExpectedDamage 0x72151 / 0x72181 / 0x721B6 -> tails 0x8ADDE /
        // 0x8AE03 / 0x8AE4F): helmet, item 0x31 and body armour count only at condition >= 9 (signed bytes).
        let worn = crate::fanfix::on(43);
        let ok = |at: usize| !worn || self.s8r(o, at + 2) >= 9;
        let mut helm = 0;
        let ht = self.rd8_of(o, HELMET);
        if ht != 0 && ok(HELMET) {
            helm = (self.s8r(o, HELMET + 2) * self.absorb(ht as usize + 15) / 100) as u8 as i32;
        }
        // 5.
        let mut body = 0;
        if self.rd8_of(o, VEST_POCKET) == 0x31 && ok(VEST_POCKET) {
            body = (body + self.s8r(o, VEST_POCKET + 2) * self.absorb(0x31) / 100) as u8 as i32;
        }
        let bt = self.rd8_of(o, BODY_ARMOUR);
        if bt != 0 && ok(BODY_ARMOUR) {
            body = (body + self.s8r(o, BODY_ARMOUR + 2) * self.absorb(bt as usize) / 100) as u8 as i32;
        }
        // 6.
        let v = (base - (helm * 15 + body * 80) / 100) as u8 as i8 as i32;
        v.max(1)
    }

    // ================= Gun option (op 24) =================
    pub(super) fn gun_option(&self) -> Rec {
        let me = self.slot();
        // 1.
        let mut rec = Rec::empty();
        // 2.
        let (opps, _) = self.opponents_a();
        let my = s16(self.rd16(f::TILE) as i32);
        // 3.
        for (o, j) in opps {
            // a.
            if !self.alive_a(o) || self.s8(SEES + j) != 1 {
                continue;
            }
            let ot = self.s16r(o, f::TILE);
            // b.
            let cost = (self.call(op::ACTION_AP, &[me, ot, 1]) as u8) as i32;
            if cost > self.ap() {
                continue;
            }
            // c.
            let line = (self.call(op::FIRE_LINE, &[me, ot, 0]) as u8) as i32;
            if line == 0 {
                continue;
            }
            // d.
            let r = self.call(op::SHOT_AP, &[me, NONE as i32]) as i32;
            let sap = (r.wrapping_sub(1) as u8) as i32;
            // e.
            let top = (self.ap() - cost).min(4);
            let (mut best, mut aim, mut chance) = (0, 0, 0);
            let mut a = 0;
            while a <= top {
                let c = (self.call(op::CHANCE_TO_HIT, &[me, ot, a]) as u8) as i32;
                // a + sap = 0: div_zero (op 47), quotient 0.
                let v = s16(self.cdiv(c * self.ap(), a + sap));
                if v > best {
                    best = v;
                    aim = a;
                    chance = c;
                }
                a += 1;
            }
            // f.
            if best == 0 {
                continue;
            }
            let eff = (line * chance / 100) as u8 as i32;
            if eff == 0 {
                continue;
            }
            // g.
            let p = s16(self.soldier_power(o, my, 1));
            // h.
            let score = best
                .wrapping_mul(self.expected_damage(o, chance as u8))
                .wrapping_mul(eff)
                .wrapping_mul(p);
            // i.
            if !replaces(&rec, eff, score) {
                continue;
            }
            // j.
            rec = Rec {
                valid: 1,
                slot: o as u8,
                aim: aim as u8,
                chance: eff as u8,
                score,
                tile: self.rd16_of(o, f::TILE),
            };
        }
        // 4.
        rec
    }

    // ================= Melee hit chance =================
    fn skill(&self, r: usize, first: usize, second: usize, k: u8) -> i32 {
        let mut v = s16(
            s16(self.s8r(r, first) * 3 + self.s8r(r, second) + self.s8r(r, f::HP_MAX) + self.s8r(r, LEVEL) * 10) / 6,
        );
        if k != 0 {
            v = s16(v + k as i32 * 5);
        }
        // SPEC GAP: "if u8 R byte +0x1B9 = 4: v = s16(v − 20), and if v < 1 then v = 1" — the lower limit of 1
        // is read as part of the +0x1B9 = 4 case only.
        if self.u8r(r, B1B9) == 4 {
            v = s16(v - 20);
            if v < 1 {
                v = 1;
            }
        }
        let (h, mh) = (self.s8r(r, f::HP), self.s8r(r, f::HP_MAX));
        if h < mh {
            let half = ((self.u8r(r, f::HP_MAX) - self.u8r(r, f::HP) - self.u8r(r, WOUNDS)) as u8 as i32) / 2;
            // Fan fix F06 (#70 row 88, switch HEALTHY): bandages lower the loss (1.15m 0x56B66 -> 0x3C007 for the
            // attacker, 0x56C4C -> 0x3C025 for the defender).
            let lost = if crate::fanfix::on(6) { mh - h - half } else { half + mh - h };
            // s8 maxHealth × 3 can be 0: div_zero (op 47), quotient 0.
            v = s16(v - self.cdiv(v * 2 * lost, mh * 3));
        }
        let e = self.s8r(r, f::ENERGY);
        if e < 100 {
            v = s16(v - (100 - e) * v / 200);
        }
        v
    }
    pub(super) fn melee_hit_chance(&self, o: usize, k: u8) -> i32 {
        // 1.
        let t = self.rd8(f::ITEMS);
        if t != 0x0F && t != 0x10 {
            return 0;
        }
        // 3.
        let a = self.skill(self.s, DEXTERITY, AGILITY, k);
        let d = self.skill(o, AGILITY, DEXTERITY, 0);
        // 4.
        // a + d = 0: div_zero (op 47), quotient 0 (then clamped to 1).
        let mut v = self.cdiv(a * 100, a + d);
        if self.s >= 0x28 {
            // SPEC GAP: "−10, 0, +10 for difficulty 0, 1, 2"; a difficulty above 2 uses +10.
            v += match self.w_difficulty() { 0 => -10, 1 => 0, _ => 10 };
        }
        s16(v).clamp(1, 99)
    }

    // ================= Knife damage =================
    pub(super) fn knife_damage(&self, o: usize, c: u8, sneak: bool) -> i32 {
        let me = self.s;
        // 1.
        let t = self.rd8(f::ITEMS);
        let mut b = (self.b5(t) + (self.u8r(me, LEVEL) - 1) * 2) as u8;
        // 2.
        let lost = ((self.u8r(me, f::HP_MAX) - self.u8r(me, f::HP) - self.u8r(me, WOUNDS)) as u8) as i8 as i32;
        b = (b as i8 as i32 + (lost / 2 + self.s8r(me, f::HP) - 50) / 5) as u8;
        // 3.
        b = (b as i8 as i32 + c as i32 / 10) as u8;
        // 4.
        b = (self.s8r(me, f::ITEMS + 2) * b as i8 as i32 / 100) as u8;
        // 5.
        // Fan fix F43 (#70 row 93; 1.15m KnifeDamage 0x7246E / 0x7249C -> tails 0x8AE16 / 0x8AE61): item 0x31 and
        // body armour count only at condition >= 9 (signed bytes).
        if !sneak {
            let ok = |at: usize| !crate::fanfix::on(43) || self.s8r(o, at + 2) >= 9;
            if self.rd8_of(o, VEST_POCKET) == 0x31 && ok(VEST_POCKET) {
                b = (b as i8 as i32 - self.s8r(o, VEST_POCKET + 2) * self.absorb(0x31) / 100) as u8;
            }
            let bt = self.rd8_of(o, BODY_ARMOUR);
            if bt != 0 && ok(BODY_ARMOUR) {
                b = (b as i8 as i32 - self.absorb(bt as usize) * self.s8r(o, BODY_ARMOUR + 2) / 100) as u8;
            }
        }
        // 6.
        (b as i8 as i32).max(1)
    }

    // ================= Knife option (op 26) =================
    pub(super) fn knife_option(&self) -> Rec {
        let me = self.slot();
        // 1.
        let mut rec = Rec::empty();
        let (opps, own) = self.opponents_a();
        let my = s16(self.rd16(f::TILE) as i32);
        // SPEC GAP: the gun option says "ap ... read again where used"; the knife option says only "ap = s8 apLeft"
        // (step 1). ap is read once here and kept.
        let ap = self.ap();
        // 2.
        for (o, j) in opps {
            // a.
            if !self.alive_a(o) || self.s8(SEES + j) != 1 {
                continue;
            }
            let ot = self.s16r(o, f::TILE);
            // b.
            let cost = (self.call(op::ACTION_AP, &[me, ot, 1]) as u8) as i32;
            if cost > ap {
                continue;
            }
            // c.
            let r = self.call(op::SHOT_AP, &[me, NONE as i32]) as i32;
            let sap = (r.wrapping_sub(1) as u8) as i32;
            // d.
            // SPEC GAP: "O byte +0x13A + me − own" with aiType ≥ 3 but me < 0x28 (or aiType < 3 and me ≥ 0x28)
            // gives an offset outside the sight flags; it is read as computed (any byte of O's record).
            let off = SEES as i32 + me - own as i32;
            let mut sneak = false;
            if (self.rd8_of(o, off as usize) as i8) != 1 {
                let td = self.call(op::TILE_DIST, &[my, ot]) as i32;
                if td == 1 {
                    let diff = ((my - ot).abs() as u8) as i32;
                    sneak = diff == 1 || diff == 56;
                }
            }
            // e.
            let top = (ap - cost).min(4);
            let (mut best, mut aim, mut chance) = (0, 0, 0);
            let mut a = 0;
            while a <= top {
                let c = if sneak { 99 } else { self.melee_hit_chance(o, a as u8) };
                // sap + a = 0: div_zero (op 47), quotient 0.
                let v = s16(self.cdiv(ap * c, sap + a));
                if v > best {
                    best = v;
                    aim = a;
                    chance = c;
                }
                a += 1;
            }
            // f.
            if best == 0 {
                continue;
            }
            // g.
            let p = s16(self.soldier_power(o, my, 0));
            // h.
            let score = self
                .knife_damage(o, chance as u8, sneak)
                .wrapping_mul(best)
                .wrapping_mul(chance)
                .wrapping_mul(p);
            // i.
            if !replaces(&rec, chance, score) {
                continue;
            }
            // j.
            rec = Rec {
                valid: 1,
                slot: o as u8,
                aim: aim as u8,
                chance: chance as u8,
                score,
                tile: self.rd16_of(o, f::TILE),
            };
        }
        rec
    }
    // ================= Throw damage =================
    /// ground(T) = t0_class[byte 0 of tile T].
    fn ground(&self, t: i32) -> u8 {
        self.t0_class(self.tb(t, 0))
    }
    pub(super) fn throw_damage(&self, o: usize, t: i32) -> i32 {
        let me = self.s;
        // 1.
        let (mut blast, mut gas): (u8, u8) = match self.rd8(f::ITEMS) {
            0x18 => (10, 60),
            0x1A => (22, 50),
            0x1C => (37, 0),
            0x16 => (0, if self.ground(t) == 3 { 75 } else { 37 }),
            _ => (0, 0),
        };
        // 2. Fan fix F43 (#70 row 93; 1.15m ThrowDamage 0x722AE -> tail 0x8ADF0, 0x722DE -> 0x8AE3A, 0x7231D ->
        // 0x8AD6D): item 0x31 and the body armour need condition >= 9, the gas mask >= 10 (signed bytes).
        let worn = crate::fanfix::on(43);
        if blast != 0 {
            if self.rd8_of(o, VEST_POCKET) == 0x31 && (!worn || self.s8r(o, VEST_POCKET + 2) >= 9) {
                blast = (blast as i8 as i32 - self.absorb(0x31) * self.s8r(o, VEST_POCKET + 2) / 100) as u8;
            }
            let bt = self.rd8_of(o, BODY_ARMOUR);
            if bt != 0 && (!worn || self.s8r(o, BODY_ARMOUR + 2) >= 9) {
                blast = (blast as i8 as i32 - self.s8r(o, BODY_ARMOUR + 2) * self.absorb(bt as usize) / 100) as u8;
            }
            if (blast as i8) < 1 {
                blast = 1;
            }
        }
        // 3.
        if gas != 0 && self.rd8_of(o, ITEM3) == 0x28 && (!worn || self.s8r(o, ITEM3 + 2) >= 10) {
            gas = ((100 - self.s8r(o, ITEM3 + 2)) * gas as i8 as i32 / 100) as u8;
        }
        // 4.
        let v = gas as i8 as i32 / 3 + blast as i8 as i32;
        // 5.
        let units = self.s8r(me, f::ITEMS + 1);
        let cond = if units == 1 {
            self.s8r(me, f::ITEMS + 2)
        } else {
            self.s8r(me, ((6 + units) & 0x1FF) as usize)
        };
        // 6.
        let r = ((v as u8 as i8 as i32 * cond / 100) as u8) as i8 as i32;
        // 7.
        if self.ground(t) >= 8 { r / 2 } else { r / 2 + r }
    }

    // ================= Throw AP =================
    pub(super) fn throw_ap(&self) -> i32 {
        if self.item_kind(self.rd8(f::ITEMS)) != super::KIND_THROW {
            return 0;
        }
        let m = self.max_ap(self.s) as u8 as i32;
        let div = self.s8(DEXTERITY) / 2 * 2 + 100;
        let v = ((self.cdiv(m * 200, div) + 1) / 2) as u8 as i32;
        let v = v.min(m);
        if v == 0 { 1 } else { v }
    }

    // ================= Throw chance =================
    pub(super) fn throw_chance(&self, t: i32, k: u8) -> i32 {
        self.throw_chance_far(t, k).0
    }
    /// Throw chance and the 1.15m "too far" flag: true when the distance is more than reach + 16 (1.15m ThrowChance
    /// leaves ebp = 15 - (d - reach) and `inc ebp` sets SF, which ThrowOption 0x71C68 tests; fan fix F58).
    pub(super) fn throw_chance_far(&self, t: i32, k: u8) -> (i32, bool) {
        let me = self.s;
        let t = s16(t);
        // 1.
        let mut v = (self.s8(MARKSMANSHIP) + self.s8(DEXTERITY)) / 2;
        if t == self.s16r(me, f::LAST_TARGET_TILE) {
            v += 10;
        }
        if k != 0 {
            v += k as i32 * 10;
        }
        // Fan fix F42 (#70 row 89; 1.15m 0x58BFE -> tail 0x8AE8E): a merc being bandaged (+0x1B9 = 4) gets the
        // same -20 as in ChanceToHit / MeleeChance, lower limit 1 (1.13b: no throw penalty).
        if crate::fanfix::on(42) && self.rd8(B1B9) == 4 {
            v = (v - 20).max(1);
        }
        if me >= 0x28 {
            // Difficulty above 2 uses +10 (as the melee hit chance).
            v += match self.w_difficulty() { 0 => -10, 1 => 0, _ => 10 };
        }
        // 2.
        let mut d = s16(world_dist(self.s16r(me, f::X2), self.s16r(me, f::Y2), px(t), py(t)));
        // Fan fix F43 (#70 row 93; 1.15m 0x58C50 -> tail 0x8AD3A): sun goggles need condition >= 10.
        if self.rd8(ITEM3) == 0x27 && (!crate::fanfix::on(43) || self.rd8(ITEM3 + 2) as i8 >= 10) {
            d -= d / 10;
        }
        // 3.
        let (h, mh, e) = (self.s8(f::HP), self.s8(f::HP_MAX), self.s8(f::ENERGY));
        let mut reach = self.range(self.rd8(f::ITEMS)) * 15 + h - 50;
        reach = (reach - (100 - e) * reach / 200).max(0);
        // 4.
        v += reach / 2 - d;
        let far = d - reach > 16;
        if d > reach {
            v /= 2;
        }
        // 5.
        if h < mh {
            let half = ((self.u8r(me, f::HP_MAX) - self.u8r(me, f::HP) - self.u8r(me, WOUNDS)) as u8 as i32) / 2;
            // Fan fix F06 (#70 row 88, switch HEALTHY; 1.15m 0x58D2F -> 0x3BFED): bandages lower the loss.
            let lost = if crate::fanfix::on(6) { (mh - h) - half } else { half + (mh - h) };
            let q = self.cdiv(v * 2 * lost, mh * 3) as u8 as i32;
            v -= (100 - (self.s8(LEVEL) - 1) * 10) * q / 100;
        }
        // 6.
        if e < 100 {
            let q = ((100 - e) * v / 200) as u8 as i32;
            v -= (100 - (self.s8(DEXTERITY) - 10)) * q / 100;
        }
        // 7.
        (v.clamp(1, 99), far)
    }

    // ================= Throw option (op 25) =================
    pub(super) fn throw_option(&self) -> Rec {
        let me = self.s;
        // 1.
        let mut rec = Rec::empty();
        let (allies, opps): (Vec<usize>, Vec<(usize, usize)>) = if me >= 0x28 {
            ((0x28..0x30).collect(), (0..0x28).map(|i| (i, i)).collect())
        } else {
            ((0..0x28).collect(), (0x28..0x30).map(|i| (i, i - 0x28)).collect())
        };
        // 2.
        let ally_tiles: Vec<u16> = allies.into_iter().filter(|&a| self.alive_a(a)).map(|a| self.rd16_of(a, f::TILE)).collect();
        let my = s16(self.rd16(f::TILE) as i32);
        let ap = self.ap();
        // 3.
        let mut targets: Vec<(u16, usize, i32)> = vec![];
        for (o, j) in opps {
            if self.alive_a(o) && self.s8(SEES + j) == 1 {
                let t = self.rd16_of(o, f::TILE);
                targets.push((t, o, s16(self.soldier_power(o, my, 0))));
            }
        }
        // 4.
        let slot = self.slot();
        for &(t, k, _) in &targets {
            let tt = t as i16 as i32;
            let row = tt / 56;
            let col = tt - row * 56;
            let (left, right, up, down) =
                (col.min(2) as u8, 2.min(55 - col) as u8, row.min(2) as u8, 2.min(55 - row) as u8);
            for dy in (up.wrapping_neg() as i8 as i32)..=(down as i8 as i32) {
                for dx in (left.wrapping_neg() as i8 as i32)..=(right as i8 as i32) {
                    let c = s16(tt + dx + dy * 56);
                    // a.
                    let cost = (self.call(op::ACTION_AP, &[slot, c, 1]) as u8) as i32;
                    if cost > ap {
                        continue;
                    }
                    // b.
                    let line = (self.call(op::FIRE_LINE, &[slot, c, 0]) as u8) as i32;
                    if line == 0 {
                        continue;
                    }
                    // c.
                    if ally_tiles.iter().any(|&a| (self.call(op::TILE_DIST, &[a as i16 as i32, c]) as i32) <= 4) {
                        continue;
                    }
                    // d.
                    let mut dmg: u16 = 0;
                    for &(ot, ok, p) in &targets {
                        let dist = (self.call(op::TILE_DIST, &[ot as i16 as i32, c]) as u8) as i32;
                        if dist <= 3 {
                            let hit = (self.throw_damage(ok, c) as u8) as i32;
                            let part = (((100 - dist * 20) * hit / 100) as u8) as i32;
                            dmg = (dmg as i32).wrapping_add(part.wrapping_mul(p)) as u16;
                        }
                    }
                    // e.
                    let tac = self.throw_ap();
                    // f.
                    let (mut best, mut aim, mut chance) = (0, 0, 0);
                    let top = (ap - cost).min(4);
                    let mut a = 0;
                    while a <= top {
                        let (ch, far) = self.throw_chance_far(c, a as u8);
                        // Fan fix F58 (#70 row 37; 1.15m ThrowOption 0x71C68): a target more than 16 past the
                        // throw reach counts as chance 0.
                        let ch = if far && crate::fanfix::on(58) { 0 } else { (ch as u8) as i32 };
                        let ch = ((ch + ch / 2) as u8 as i32).min(100);
                        let v = s16(self.cdiv(ap * ch, tac + a));
                        if v > best {
                            best = v;
                            aim = a;
                            chance = ch;
                        }
                        a += 1;
                    }
                    // g.
                    if best == 0 {
                        continue;
                    }
                    let eff = (chance * line / 100) as u8 as i32;
                    if eff == 0 {
                        continue;
                    }
                    // h.
                    let score = eff.wrapping_mul(best).wrapping_mul(dmg as i16 as i32);
                    if score <= rec.score {
                        continue;
                    }
                    // i.
                    rec = Rec { valid: 1, slot: k as u8, aim: aim as u8, chance: eff as u8, score, tile: c as u16 };
                }
            }
        }
        rec
    }
}

// ================= Action AP (sector_start.md batch 12, op 17 as a library op) =================
impl Ai {
    /// ShotAP(t): the low byte of host op 45 shot_ap(slot, t).
    fn shot_ap_at(&self, t: i32) -> i32 {
        (self.call(op::SHOT_AP, &[self.slot(), t]) & 0xFF) as i32
    }
    /// weapons[6 × type + 2]; SPEC GAP: a null weapons table reads 0.
    fn weapon_ap(&self, t: u8) -> i32 {
        let p = unsafe { (*self.w).weapons };
        if p.is_null() {
            return 0;
        }
        unsafe { p.add(t as usize * 6 + 2).read() as i32 }
    }

    /// "Action AP" (`ja1_action_ap`): a byte 0..255. tile = signed 32-bit, flag = its low byte.
    pub(super) fn action_ap(&self, tile: i32, flag: u8) -> i32 {
        let slot = self.slot();
        let me = s16(self.rd16(f::TILE) as i32);
        let kind = self.item_kind(self.rd8(f::ITEMS));
        // 1.
        if kind == super::KIND_THROW {
            return self.throw_ap() & 0xFF;
        }
        // 2.
        if kind == super::KIND_KNIFE {
            let mut c = 0i32;
            let mut tile = tile;
            // a.
            if tile < NONE as i32 {
                let dist = self.call(op::TILE_DIST, &[me, tile]) as i32;
                let near = dist <= 1 && (me % 56 == tile % 56 || me / 56 == tile / 56);
                if !near {
                    if self.call(op::REACH_AP, &[slot, tile, 0xF]) as i32 != 0 {
                        let spot = s16(self.w_mission_spot() as i32);
                        c = (self.call(op::PATH_AP, &[slot, spot]) & 0xFF) as i32;
                        tile = NONE as i32;
                    } else {
                        c = 0x63;
                    }
                }
            }
            // b.
            return (c + self.shot_ap_at(tile)) & 0xFF;
        }
        // 3.
        if kind != super::KIND_GUN {
            return 0x63;
        }
        // 4.
        let sap = self.shot_ap_at(tile);
        let a = self.rd16(f::ANIM + 2 * self.rd8(f::ANIM_IDX) as usize);
        // a.
        if !matches!(a, 0x1B | 0x1F | 0x23) {
            return (sap + self.weapon_ap(self.rd8(f::ITEMS))) & 0xFF;
        }
        // b.
        if flag == 0 {
            return sap;
        }
        // c.
        if tile != NONE as i32 {
            let args = [(me % 56) * 16 + 5, (me / 56) * 14 + 5, (tile % 56) * 16 + 5, (tile / 56) * 14 + 5];
            if self.call(op::DIRECTION, &args) as i32 == self.s8(f::FACING) {
                return sap;
            }
        }
        // d.
        (sap + 1) & 0xFF
    }
}

// ================= Shot AP (sector_start.md batch 13, op 45 as a library op) =================
impl Ai {
    /// weapons[6 × y + i]; SPEC GAP: a null weapons table reads 0; the bytes are read as unsigned.
    fn weapon_byte(&self, y: u8, i: usize) -> i32 {
        let p = unsafe { (*self.w).weapons };
        if p.is_null() {
            return 0;
        }
        unsafe { p.add(y as usize * 6 + i).read() as i32 }
    }

    /// "Shot AP" (`ja1_shot_ap`): a byte. tile = signed 32-bit. Only host call: 47 div_zero.
    pub(super) fn shot_ap(&self, tile: i32) -> i32 {
        let y = self.rd8(f::ITEMS);
        // 1.
        if !(1..=0x10).contains(&y) {
            return 0;
        }
        // 2.
        let d0 = s16(self.rd16(0x0D0) as i32);
        let c = (tile != d0 || d0 == NONE as i32) as i32;
        // 3.
        let m = self.max_ap(self.s) as u8 as i32;
        // 4.
        let k = if self.weapon_byte(y, 0) == 4 {
            ((self.s8(AGILITY) + self.s8(DEXTERITY)) / 2) as u8 as i32
        } else {
            self.rd8(MARKSMANSHIP) as i32
        };
        // 5.
        let d = (k / 2 + 50) * self.weapon_byte(y, 3) / 2;
        // 6.
        let q = self.cdiv(200 * m, d);
        // 7.
        let mut b = (c + (q + 1) / 2) as u8 as i32;
        if b > m {
            b = m;
        }
        if b == 0 {
            b = 1;
        }
        b
    }
}

/// MeleeChance 0x56A78 of attacker `slot` on record `defender` with aim `aim` (u8 1..99, 0 without a knife or
/// fist item). Used by the KnifeOption / melee paths inside the library; the C entry is for the fan fix test
/// (fanfix_diff.py melee, #70 F06). A defender outside 0..47 gives 0.
#[no_mangle]
pub extern "C" fn ja1_melee_chance(w: *mut Ja1World, h: *const Ja1Host, slot: i32, defender: i32, aim: i32) -> i32 {
    if !(0..48).contains(&defender) {
        return 0;
    }
    run(w, h, slot, 0, |a| a.melee_hit_chance(defender as usize, aim as u8))
}

/// ThrowChance 0x58BB0 of `slot` at tile `tile` with aim `aim` (1..99). Used inside the library (ThrowOption, the
/// throw); the C entry is for the fan fix test (fanfix_diff.py throw, #70 F06).
#[no_mangle]
pub extern "C" fn ja1_throw_chance(w: *mut Ja1World, h: *const Ja1Host, slot: i32, tile: i32, aim: i32) -> i32 {
    run(w, h, slot, 0, |a| a.throw_chance(tile, aim as u8))
}

// SPEC GAP: on a null world / host or a slot outside 0..47 the result is 0 (no host call).
#[no_mangle]
pub extern "C" fn ja1_shot_ap(w: *mut Ja1World, h: *const Ja1Host, slot: i32, tile: i32) -> i32 {
    run(w, h, slot, 0, |a| a.shot_ap(tile))
}

// SPEC GAP: on a null world / host or a slot outside 0..47 the result is 0 (no host call).
#[no_mangle]
pub extern "C" fn ja1_action_ap(w: *mut Ja1World, h: *const Ja1Host, slot: i32, tile: i32, flag: i32) -> i32 {
    run(w, h, slot, 0, |a| a.action_ap(tile, flag as u8))
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn record_bytes_and_replace_rule() {
        let r = Rec::empty();
        assert_eq!(r.bytes(), [0, 0x40, 0, 0, 0, 0, 0, 0, 0x41, 0x0C]);
        let r = Rec { valid: 1, slot: 3, aim: 0, chance: 40, score: 1000, tile: 5 };
        assert!(!replaces(&r, 40, 9999)); // not higher
        assert!(!replaces(&r, 50, 999)); // +25 % and a lower score
        assert!(replaces(&r, 50, 1000));
        assert!(replaces(&r, 60, 0)); // +50 %
    }
}

#[cfg(test)]
mod guard_world_tests {
    use super::*;
    fn host() -> Ja1Host {
        Ja1Host { ctx: std::ptr::null_mut::<std::os::raw::c_void>(), random: None, call: None, host_ops: 0 }
    }

    #[test]
    fn guard_absorb_and_range() {
        // Port rule (SPEC GAP): an absorb index past the 288-word table, or an item of 136 and up, reads 0.
        let mut ab = vec![0u8; ABSORB_LEN + 4];
        ab[2 * 287] = 7;
        ab[ABSORB_LEN] = 9;
        let mut it = vec![0u8; (ITEM_TYPES + 1) * 12];
        it[135 * 12 + 11] = 4;
        it[136 * 12 + 11] = 6;
        let mut w: Box<Ja1World> = Box::new(unsafe { std::mem::zeroed() });
        w.absorb = ab.as_mut_ptr();
        w.item_types = it.as_mut_ptr();
        let h = host();
        let ai = Ai { w: &mut *w, h: &h, s: 0 };
        assert_eq!(ai.absorb(287), 7);
        assert_eq!(ai.absorb(288), 0);
        assert_eq!(ai.range(135), 4);
        assert_eq!(ai.range(136), 0);
    }
}
