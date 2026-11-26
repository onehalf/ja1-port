//! Using the hand item on a tile (docs/spec/combat.md "Using the hand item on a tile, exact steps" and "Crowbar,
//! exact steps"): library op 74 use_on_tile, `ja1_use_on_tile`; and "Burying a mine, exact steps": library op 75
//! bury_mine, `ja1_bury_mine`. A child module of `ai`; the prompt, the mine check,
//! the pick-up and the messages are ops through `Ai::call`.
use super::search::s16;
use super::{op, Ai, Ja1Host, Ja1World, TILES};

const NONE: i32 = 0xC41;
const FX_LEN: usize = 5 * 13;
const SECTOR_ROW: usize = 0xB1;
const SECTOR_ROWS: usize = 60;
/// Locked containers per tileset 0..9 (objects 0x9A..0xA4 and 0xA6..0xAA).
const LOCK_A: [u8; 10] = [1, 1, 1, 1, 0, 1, 1, 0, 1, 1];
const LOCK_B: [u8; 10] = [1, 1, 1, 1, 0, 0, 0, 0, 1, 1];
const DISARM: i32 = op::DISARM;
const MINE_SPOTTED: i32 = op::MINE_SPOTTED;
const CROWBAR_BREAKS: i32 = 13;
const MINE_BURIED: i32 = 14;
const OBJ_BLOCK_LEN: usize = 685;
const BLAST_LIST_LEN: usize = 60;
const HELD_LEN: usize = 12;
const STORY_LEN: usize = 35;
const CURSOR_TO_SOLDIER: i32 = op::CURSOR_TO_SOLDIER;
const DISARM_ASK: i32 = op::DISARM_ASK;
const GROUND_ITEM_RESET: i32 = op::GROUND_ITEM_RESET;
const MENU_REDRAW: i32 = op::MENU_REDRAW;
const ITEM_TYPES_LEN: usize = 136 * 12;
/// The four story items: (type, quote, `story` flag offset).
const STORY_PICKUPS: [(u8, i32, i64); 4] = [(0x79, 0x12, 0x1A), (0x86, 0x15, 0x1B), (0x5F, 0x1B, 0x1D), (0x6E, 0x1F, 0x1C)];

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

impl Ai {
    fn ut_w(&self) -> &mut Ja1World {
        unsafe { &mut *self.w }
    }
    /// combat.md Mine spotted, "G for g of 200 or more": entry g ≥ 200 reads the bytes after the 200 entries (the 32-byte
    /// text buffer, 128 bytes, then the profiles; entry 0xFF = profile 5 +0x52.. and profile 6 +0..+3).
    fn ut_ge(&self, g: u8, o: usize) -> u8 {
        self.ge_byte(12 * g as usize + o)
    }
    /// The write of `ut_ge`: past the 200 entries into the text buffer or the profiles.
    fn ut_ge_put(&self, g: u8, o: usize, v: u8) {
        self.ge_byte_put(12 * g as usize + o, v)
    }
    /// Byte i of the ground entries and of the bytes after them (`ut_ge`).
    fn ge_byte(&self, i: usize) -> u8 {
        super::shared::ground_byte(self.w, i)
    }
    fn ge_byte_put(&self, i: usize, v: u8) {
        super::shared::ground_byte_put(self.w, i, v)
    }
    fn ut_tile_put(&self, t: i32, i: usize, v: u8) {
        let p = self.ut_w().tiles;
        if !p.is_null() && (0..TILES as i32).contains(&t) {
            unsafe { p.add(t as usize * 8 + i).write(v) }
        }
    }
    fn ut_ground(&self, t: i32) -> u8 {
        let g = self.tb(t, 6);
        if g >= 200 { 0xFF } else { g }
    }
    fn ut_sector(&self, o: usize) -> u8 {
        let s = self.ut_w().cur_sector as usize;
        if s >= SECTOR_ROWS {
            return 0;
        }
        get(self.ut_w().sectors, SECTOR_ROWS * SECTOR_ROW, (s * SECTOR_ROW + o) as i64)
    }
    fn ut_sector_word(&self, o: usize) -> i32 {
        (self.ut_sector(o) as u16 | (self.ut_sector(o + 1) as u16) << 8) as i16 as i32
    }

    /// Free effect (tile).
    fn free_effect(&self, tile: i32) {
        let p = self.ut_w().timed_fx;
        for f in 0..5usize {
            let b = 13 * f;
            let t = get(p, FX_LEN, (b + 6) as i64) as u16 | (get(p, FX_LEN, (b + 7) as i64) as u16) << 8;
            if t == tile as u16 {
                for k in 0..4 {
                    put(p, FX_LEN, (b + k) as i64, 0xFF);
                }
                put(p, FX_LEN, (b + 6) as i64, NONE as u8);
                put(p, FX_LEN, (b + 7) as i64, (NONE >> 8) as u8);
                put(p, FX_LEN, (b + 0xC) as i64, 0);
                self.ut_w().fuse_beep = 0;
                return;
            }
        }
    }
    /// Mines down.
    fn mines_down(&self) {
        let s = self.ut_w().cur_sector as usize;
        if s >= SECTOR_ROWS {
            return;
        }
        let i = (s * SECTOR_ROW + 0xAF) as i64;
        let p = self.ut_w().sectors;
        put(p, SECTOR_ROWS * SECTOR_ROW, i, get(p, SECTOR_ROWS * SECTOR_ROW, i).wrapping_sub(1));
    }

    /// combat.md "Crowbar, exact steps".
    fn crowbar(&self, tile: i32, flag: u8) -> i32 {
        let slot = self.slot();
        let me = self.rd8(0x17A);
        // 1.
        let v = if self.tile_standable(tile) < 0 {
            super::doors::door_lookup(self.w, tile).wrapping_mul(2)
        } else {
            (0..12)
                .find(|k| self.ut_sector_word(0x30 + 4 * k) == tile)
                .map(|k| self.ut_sector_word(0x30 + 4 * k + 2))
                .unwrap_or(0)
        };
        // 2.
        if v == 0 {
            self.call(op::SPEND_AP, &[slot, 0, 0x32]);
            return 1;
        }
        // 3.
        let hp = self.s8(0x183);
        let missing = if hp < self.s8(0x184) { self.s8(0x184) - hp - self.s8(0x185) } else { 0 };
        let mut f = self.s8(0x18A) * ((hp + missing / 2) / 2) / 100;
        // 4.
        let mut bar = flag == 0x76 && self.rd8(0) == 0x76 && self.s8(2) >= 10;
        // 5.
        if bar {
            let bonus = f * 50 / 100;
            f += bonus;
            let m = (if f < v { f } else { v }) as u8;
            let risk = (m.wrapping_sub(self.rd8(2)) as i8 as i32).max(0);
            if risk != 0 && self.random(100) < risk {
                bar = false;
                f = (f - bonus) / 2;
                let c = self.random(9) + 1;
                self.wr8(2, c as u8);
                self.call(op::FOCUS, &[me as i32, 1]);
                self.call(op::CRY, &[self.rd8(0x179) as i32, -1, 0, 1, 1]);
                self.call(op::VOICE, &[0x2B11, 0x32, 1]);
                self.call(op::MESSAGE, &[CROWBAR_BREAKS]);
            } else if m >= 0x14 {
                let r = self.random((m / 10) as i32);
                self.wr8(2, self.rd8(2).wrapping_sub(r as u8));
            }
            if me == self.ut_w().selected {
                self.call(op::ITEM_PANEL, &[slot, 0, 1]);
            }
        }
        // 6.
        let a = if f < v {
            f
        } else if bar {
            // SPEC GAP: f = 0 divides by zero in the original; a = 0.
            if f == 0 { 0 } else { v.wrapping_mul(v).wrapping_div(f) }
        } else {
            v
        };
        self.call(op::SPEND_AP, &[slot, 0, ((a as u8 as i32) * 0x32) as i16 as i32]);
        // 7.
        if v <= f {
            self.call(op::STAT_PRACTICE, &[slot, 1, (v / 10) as i16 as i32, 0]);
            return 1;
        }
        0
    }

    /// combat.md "Using the hand item on a tile, exact steps" (op 74).
    fn use_on_tile(&self, tile: i32, flag: u8) {
        let slot = self.slot();
        let me = self.rd8(0x17A);
        // 1.
        if tile > 0xC40 {
            return;
        }
        // 2.
        let obj = self.tb(tile, 1);
        let mut g = self.ut_ground(tile);
        let occ = {
            let o = self.tb(tile, 5) & 0x7F;
            if o >= 0x30 { 0x40 } else { o }
        };
        let st = self.tile_standable(tile);
        let (mut practise, mut locked, mut breaks, mut nocrow, mut handled) = (false, false, false, false, false);
        let mut noisy = true;
        let mut spotted = 0u8;
        // 3.
        if !(st < 0 || !(0x37..=0x82).contains(&obj) || s16(self.rd16(0x114) as i32) > tile) {
            g = 0xFF;
        }
        if g < 0xFF {
            handled = (self.ut_ge(g, 6) as i8) > 0;
        }
        // 4.
        let finish = |practise: bool| {
            if practise {
                self.call(op::STAT_PRACTICE, &[slot, 4, 1, 0]);
            }
            self.wr8(0x1A6, 0);
            self.wr16(0x134, NONE as u16);
            self.wr8(0x1A7, 0);
            self.wr8(0x95, 0);
            self.wr16(0x96, NONE as u16);
        };
        // 5.
        if occ < 0x40 {
            if occ < 8 && occ != me {
                self.call(op::MERC_MESSAGE, &[occ as i32, 0xE, 0x40, 0, 1]);
            }
            finish(practise);
            return;
        }
        // 6.
        let ts = self.ut_w().tileset as i8;
        if !handled {
            match obj {
                0x8E => {
                    locked = true;
                    nocrow = true;
                    noisy = ts == 7;
                    if self.ut_w().cur_sector == 0xB {
                        if self.rd8(0) != 0x87 {
                            if self.ut_w().safe_opened == 0 {
                                self.call(op::MERC_MESSAGE, &[slot, 0x17, 0x40, 0, 1]);
                                finish(practise);
                                return;
                            }
                        } else {
                            self.call(op::FOCUS, &[me as i32, 0]);
                            self.call(op::CRY, &[self.rd8(0x179) as i32, -1, 1, 1, 1]);
                            self.call(op::VOICE, &[0x2B11, 0x32, 1]);
                            self.call(op::DELAY, &[1000]);
                            self.ut_w().safe_opened = 1;
                            self.call(op::STAT_PRACTICE, &[slot, 4, 0x32, 0]);
                            self.ut_w().mission_step = 0;
                            self.ut_w().mission_slot = 0x40;
                        }
                    }
                }
                0x9A | 0x9C | 0x9E | 0xA0 | 0xA2 | 0xA4 => {
                    locked = (0..10).contains(&ts) && LOCK_A[ts as usize] != 0;
                }
                0xA6 | 0xA8 | 0xAA => {
                    locked = (0..10).contains(&ts) && LOCK_B[ts as usize] != 0;
                }
                _ => {
                    breaks = st < 0 && flag == 0x76;
                }
            }
        }
        // 7.
        let sound = || {
            if noisy {
                self.call(op::SFX, &[0x22, 0x2B11, 0x32, 1]);
                self.call(op::MAKE_NOISE, &[me as i32, tile, self.rd8(0x1B3) as i32, 3, 2]);
            }
        };
        let obj_up = || self.ut_tile_put(tile, 1, self.tb(tile, 1).wrapping_add(1));
        // Port fix P08 (#2 row 37): the type test of a ground entry needs a real entry (g < 0xFF); 1.13b read entry
        // 0xFF, past the table, in profile 5's kit.
        let no_pickup = |g: u8| !(crate::fanfix::port_on(8) && g == 0xFF) && matches!(self.ut_ge(g, 0), 0x44 | 0x52 | 0x70);
        // 8.
        if locked {
            if g < 0xFF {
                if self.call(DISARM, &[slot, tile, g as i32, 1, 1]) as i32 <= 0 {
                    g = 0xFF;
                } else {
                    if crate::fanfix::on(55) {
                        // F55 (1.15l changelog: free experience from an adjacent object; found by DarzaR): 1.15m
                        // UseOnTile? 0x3D2B6..0x3D2E1 drops the tile from the blast list (BlastListRemove? 0x3B718)
                        // after a disarmed lock trap, before the crowbar test.
                        self.blast_list_remove(tile);
                    }
                    if !nocrow && self.crowbar(tile, flag) != 1 {
                        self.call(op::MERC_MESSAGE, &[slot, 0x17, 0x40, 0, 1]);
                        g = 0xFF;
                    } else {
                        obj_up();
                        sound();
                    }
                }
            } else if !nocrow && self.crowbar(tile, flag) != 1 {
                self.call(op::MERC_MESSAGE, &[slot, 0x17, 0x40, 0, 1]);
            } else {
                sound();
                obj_up();
                self.call(op::QUOTE_EVENT, &[me as i32, 0xFB, 6, NONE, 0]);
            }
        } else if no_pickup(g) {
            if !(crate::fanfix::on(54) && g == 0xFF) {
                self.call(op::TRIGGER_ITEM, &[slot, tile, g as i32, 1]);
            }
        } else if me >= 8 {
            // F54 (1.15l changelog: odd animations when a moving merc uses a nearby object; found by DarzaR):
            // 1.15m UseOnTile? 0x3D412 / 0x3D4B5..0x3D4D5 calls TriggerItem? only for a real ground entry (not 0xFF).
            if !(crate::fanfix::on(54) && g == 0xFF) {
                self.call(op::TRIGGER_ITEM, &[slot, tile, g as i32, 1]);
            }
            finish(practise);
            return;
        } else {
            spotted = self.call(MINE_SPOTTED, &[slot, tile, g as i32]) as u8;
            // F55: 1.15m cave 0x2118B (called at 0x3D436) sets practise only for a real ground entry.
            practise = !crate::fanfix::on(55) || g != 0xFF;
            if breaks {
                if self.crowbar(tile, flag) == 1 {
                    self.call(op::BREAK_OBJECT, &[tile, obj as i32, 1]);
                    self.call(op::QUOTE_EVENT, &[me as i32, 0xFA, 2, NONE, 1]);
                    self.wr8(0x9D, 1);
                } else {
                    self.call(op::QUOTE_EVENT, &[me as i32, 0xF9, 2, NONE, 1]);
                }
            }
        }
        // 9.
        if g < 0xFF {
            let ask = practise;
            if practise && crate::fanfix::on(55) && self.blast_listed(tile) {
                practise = false; // F55: 1.15m 0x3D4F7..0x3D509: a known (blast-listed) trap gives no practice
            }
            if ask {
                let r = self.call(DISARM, &[slot, tile, g as i32, 0, 1]) as i32;
                if r > 0 {
                    self.call(op::PICK_UP, &[slot, tile, 1]);
                    self.free_effect(tile);
                    // F56: 1.15m 0x3D5B6 jle: a negative answer (an effect on the own tile) counts no mine down.
                    if spotted != 0 && (spotted as i8 > 0 || !crate::fanfix::on(56)) {
                        self.mines_down();
                    }
                } else {
                    if r == -1 && practise {
                        self.ut_ge_put(g, 6, 0xFF);
                        practise = false;
                    }
                    if spotted != 0 {
                        let j = self.ut_ground(tile);
                        if j < 0xFF {
                            self.ut_ge_put(j, 2, self.ut_ge(j, 2).wrapping_neg());
                        } else {
                            // F56: 1.15m 0x3D609..0x3D614: a negative answer frees the effect only.
                            if spotted as i8 > 0 || !crate::fanfix::on(56) {
                                self.mines_down();
                            }
                            self.free_effect(tile);
                        }
                    }
                }
            } else if !no_pickup(g) && me < 8 {
                self.call(op::STAT_PRACTICE, &[slot, 4, 1, 0]);
                self.ut_ge_put(g, 6, 1);
                self.call(op::QUOTE_EVENT, &[me as i32, 0xFD, 4, NONE, 1]);
            }
        } else if !locked && (flag == 0 || flag >= 0xFE) {
            self.call(op::QUOTE_EVENT, &[me as i32, 0xFC, 4, NONE, 0]);
        }
        // 10.
        finish(practise);
    }
}

impl Ai {
    /// combat.md "Burying a mine, exact steps" (op 75).
    fn bury_mine(&self, tile: i32) {
        let slot = self.slot();
        let me = self.rd8(0x17A);
        let mut tile = tile;
        // 1.
        let away = tile != s16(self.rd16(0x114) as i32);
        if away {
            let d = self.direction_to(tile as u16) & 0xFF;
            let o = self.tb(tile, 1);
            let blocked = if (0x37..=0x82).contains(&o) {
                let i = (o as i64 - 0x37) * 9 + d as i64;
                get(self.ut_w().obj_block, OBJ_BLOCK_LEN, i) as i8 == 0xB
            } else {
                // Other o: the original's read lands in other data; 0xB only at o = 0xC9, d = 4.
                o == 0xC9 && d == 4
            };
            if blocked {
                tile = super::search::step(tile, self.dir_step(d));
            }
        }
        let put_at = |b6: u8, exact: u8, note: u8, near: u8| {
            let p = unsafe { (*self.w).soldiers.add(self.s * 0x200) };
            crate::sector::put_item_at(self.w, self.h, tile, p, b6, exact, note, near)
        };
        let put = |b6: u8| {
            put_at(b6, 1, 0, 1);
        };
        if self.rd8(0) == 0x11 {
            // 2.
            if self.s8(2) < 10 {
                put(1);
            } else {
                self.wr8(1, 1);
                put(0xFF);
                let s = self.ut_w().cur_sector as usize;
                if s < SECTOR_ROWS {
                    let i = (s * SECTOR_ROW + 0xAF) as i64;
                    let p = self.ut_w().sectors;
                    put_b(p, i);
                }
                self.call(op::MESSAGE, &[MINE_BURIED]);
                self.call(op::STAT_PRACTICE, &[slot, 6, 5, 0]);
            }
        } else if crate::fanfix::on(49) {
            // F49 (1.15m BuryMine 0x3CBB3..0x3CC36): put first, practise, then the blast at the tile PutItem chose.
            self.wr8(2, 0xFF);
            let low = self.rd8(0) < 0x11;
            let at = put_at(if low { 0 } else { 1 }, away as u8, if low { 0xFF } else { 0 }, if low { 0 } else { 1 });
            self.call(op::STAT_PRACTICE, &[slot, 6, 5, 0]);
            self.wr8(0x1C9, super::aim::blast_kind(self.rd8(0)));
            let combat = self.ut_w().combat;
            let c = combat as i8 as i32;
            let k = (combat as i32 * (self.random(2) + 1)) as u8;
            self.call(op::EFFECT, &[s16(at), self.rd8(0x1C9) as i32, k as i32, 0x2134 - 6000 * c, 0x42, 1]);
        } else {
            // 3.
            self.wr8(2, 0xFF);
            self.wr8(0x1C9, super::aim::blast_kind(self.rd8(0))); // SetBlastKind? 0x5ABCC
            let combat = self.ut_w().combat;
            let c = combat as i8 as i32;
            let k = (combat as i32 * (self.random(2) + 1)) as u8;
            self.call(op::EFFECT, &[s16(tile), self.rd8(0x1C9) as i32, k as i32, 0x2134 - 6000 * c, 0x42, 1]);
            put(1);
            self.call(op::STAT_PRACTICE, &[slot, 6, 5, 0]);
        }
        // 4.
        self.call(op::EFFECT_SLOT, &[me as i32, 0]);
        for b in 0..12 {
            self.wr8(b, 0);
        }
        if me == self.ut_w().selected {
            self.call(op::ITEM_PANEL, &[slot, 0, 1]);
        }
    }
}

impl Ai {
    /// combat.md "A detected mine gets a blue flag, exact steps" (op 118).
    fn mine_flag(&self, tile: i32) {
        let slot = self.slot();
        // 1.
        self.ut_tile_put(tile, 1, 0x86);
        let w = self.ut_w();
        // SPEC GAP: a tile off the map or a null tiles / t0_class / t1_class / move_codes buffer skips the rebuild.
        if (0..TILES as i32).contains(&tile)
            && !(w.tiles.is_null() || w.t0_class.is_null() || w.t1_class.is_null() || w.move_codes.is_null())
        {
            unsafe {
                let (gc, oc) = (&*(w.t0_class as *const [u8; 256]), &*(w.t1_class as *const [u8; 256]));
                let i = tile as usize;
                let prev = u16::from_le_bytes([*w.move_codes.add(2 * i), *w.move_codes.add(2 * i + 1)]);
                let code = crate::move_code(self.tb(tile, 0), self.tb(tile, 1), self.tb(tile, 3), prev, gc, oc);
                let [lo, hi] = code.to_le_bytes();
                *w.move_codes.add(2 * i) = lo;
                *w.move_codes.add(2 * i + 1) = hi;
            }
        }
        // 2.
        let found = self.call(op::FIND_PATH, &[slot, self.s16_at(0x124), 1]);
        // 3.
        if found == 0 && self.rd8(0x17A) < 8 {
            self.clear_task();
            let t = self.s16_at(0x114);
            self.wr16(0x126, super::search::px(t) as u16);
            self.wr16(0x128, super::search::py(t) as u16);
            self.call(op::STOP_ANIM, &[slot, 0]);
            self.call(op::ANIM_STEP, &[slot]);
            self.call(op::STOP_WALK, &[slot, 0]);
        }
        // 4.
        // SPEC GAP: a null paths buffer reads 0.
        let p = self.ut_w().paths;
        let me = self.rd8(0x17A) as usize;
        let p0 = if p.is_null() { 0 } else { unsafe { p.add(0x50 * me).read() } };
        self.wr8(0x1A2, p0.wrapping_add(1));
    }

    /// combat.md "Using the lock kit on the mission safe, exact steps" (op 69).
    fn use_special(&self) {
        let slot = self.slot();
        // 1.
        if self.rd8(0) != 0x4C {
            return;
        }
        // 2.
        self.call(op::FOCUS, &[self.rd8(0x17A) as i32, 0]);
        // 3.
        let b = |o: usize| self.rd8(o) as i8 as i32;
        let q = (b(0x18C).wrapping_mul(b(0x190)).wrapping_mul(b(0x18D)) / 10000) as u8;
        let skill = q.wrapping_add(self.rd8(0x192).wrapping_mul(5));
        let skill = ((skill as i32 * b(2)) / 100) as u8;
        let need = match self.ut_w().difficulty as i8 {
            0 => 80,
            1 => 90,
            // SPEC GAP: a difficulty outside 0..2 reads past the table; the port takes the hard value.
            _ => 100,
        };
        // 4.
        if skill as i32 >= need {
            self.call(op::CRY, &[self.rd8(0x179) as i32, -1, 1, 1, 1]);
            self.call(op::VOICE, &[0x2B11, 0x32, 1]);
            self.call(op::DELAY, &[1000]);
            self.ut_w().safe_opened = 1;
            for st in [7, 4, 3] {
                self.call(op::STAT_PRACTICE, &[slot, st, 100, 0]);
            }
            self.call(op::USE_ON_TILE, &[slot, 0x1C9, self.rd8(0) as i32]);
            self.ut_w().mission_step = 0;
            self.ut_w().mission_slot = 0x40;
        } else {
            // 5.
            self.call(op::CRY, &[self.rd8(0x179) as i32, -1, 0, 1, 1]);
            self.call(op::VOICE, &[0x2B11, 0x32, 1]);
            let c = self.rd8(2);
            if c as i8 > 5 {
                self.wr8(2, c.wrapping_sub((self.random(5) + 1) as u8));
            }
        }
    }
}

impl Ai {
    /// combat.md "A thrown item lands, exact steps" (op 176).
    /// Returns the tile PutItem put the item on (1.15m reads it, fan fix F52; 1.13b leaves EAX unused).
    fn land_item(&self, tile: i32, item: u8, cond: i32, x: i32, y: i32) -> i32 {
        let slot = self.slot();
        // 1.
        let mut e = [0u8; 12];
        e[0] = item;
        e[1] = 1;
        e[2] = cond as u8;
        // 2.
        let g = self.tb(tile, 6);
        if g < 200 {
            self.call(op::TRIGGER_ITEM, &[slot, tile, g as i32, 0]);
        }
        // 3.
        let landed = crate::sector::put_item_at(self.w, self.h, tile, e.as_ptr(), 1, 0, 0, 0);
        // 4.
        let (t0, t1) = (self.tb(tile, 0), self.tb(tile, 1));
        let (r, sample, kind);
        if t0 >= 0x6C && t1 != 0x1C && t1 != 0x1F {
            let fx = self.call(op::PUFF, &[tile, x, y, 0, 5, 0, 1]) as u8;
            unsafe {
                (*self.w).fx_id = fx;
                (*self.w).anim_signal = fx as i8 as i32;
            }
            r = (self.random(4) + 4) as u8;
            sample = 0x1F;
            kind = 8;
        } else {
            r = (self.random(4) + 4) as u8;
            sample = 0x10;
            kind = if item == 0x5D { 8 } else { 9 };
        }
        // 5.
        let vol = r.wrapping_mul(10).wrapping_add(20) as i8 as i32;
        self.call(op::SFX, &[sample, 0x2B11, vol, 1]);
        // 6.
        let cls = self.t0_class(self.tb(tile, 0));
        self.call(op::MAKE_NOISE, &[self.rd8(0x17A) as i32, tile, cls as i32, r as i32, kind]);
        landed
    }
}

impl Ai {
    /// combat.md "Picking up at the own tile, exact steps" (op 73).
    fn pickup_here(&self) {
        let slot = self.slot();
        let me = self.rd8(0x17A);
        let t = s16(self.rd16(0x114) as i32);
        // 1.
        let mut g = self.ut_ground(t);
        if g < 0xFF && matches!(self.ut_ge(g, 0), 0x44 | 0x52 | 0x70) {
            g = 0xFF;
        }
        if g < 0xFF && matches!(self.ut_ge(g, 0), 0x29 | 0x3F | 0x71) {
            g = 0xFF;
        }
        // 2.
        if g == 0xFF {
            self.call(op::QUOTE_EVENT, &[me as i32, 0xFC, 1, NONE, 0]);
        }
        // 3.
        let m = self.call(MINE_SPOTTED, &[slot, t, g as i32]) as u8 as i8;
        // 4. F56: 1.15m PickUpHere? 0x3CEC0 drops the FxHere? call (MineSpotted? makes it) and keeps MineSpotted?'s
        //    signed answer: > 0 a mine (count down), < 0 an effect on the own tile.
        let f56 = crate::fanfix::on(56);
        let c = if f56 { m != 0 } else { self.effect_here(t) };
        let down = !f56 || m > 0;
        // 5.
        self.wr8(0x1D8, 0x40);
        self.wr8(0x1C2, 0x40);
        self.wr8(0x1BE, 0);
        self.wr16(0xD0, NONE as u16);
        self.wr16(0xDC, NONE as u16);
        // 6.
        if g < 0xFF {
            let r = self.call(DISARM, &[slot, t, g as i32, 0, 1]) as i32;
            if r > 0 {
                self.call(op::PICK_UP, &[slot, t, 1]);
                self.free_effect(t);
                if c && down {
                    self.mines_down();
                }
            } else if c {
                let j = self.ut_ground(t);
                if j < 0xFF {
                    self.ut_ge_put(j, 2, self.ut_ge(j, 2).wrapping_neg());
                } else {
                    if down {
                        self.mines_down();
                    }
                    self.free_effect(t);
                }
            }
        }
        // 7.
        self.wr8(0x1A6, 0);
        self.wr16(0x134, NONE as u16);
        self.wr8(0x1A7, 0);
        self.wr8(0x95, 0);
        self.wr16(0x96, NONE as u16);
    }

    /// Effect here (combat.md "Picking up at the own tile", step 4).
    fn effect_here(&self, t: i32) -> bool {
        let p = self.ut_w().timed_fx;
        let on = (0..5usize).any(|f| {
            let b = (13 * f) as i64;
            let ft = get(p, FX_LEN, b + 6) as u16 | (get(p, FX_LEN, b + 7) as u16) << 8;
            ft == t as u16 && (get(p, FX_LEN, b + 3) as i8) >= 0
        });
        if !on {
            return false;
        }
        if self.random(8) < self.s8(0x192) + self.s8(0x18F) / 3 && !self.blast_listed(t) {
            self.blast_list_add(t);
        }
        let g = self.tb(t, 6);
        self.ut_ge_put(g, 2, self.ut_ge(g, 2).wrapping_neg());
        true
    }

    fn blast_word(&self, k: i64) -> i32 {
        let p = self.ut_w().blast_list;
        (get(p, BLAST_LIST_LEN, 2 * k) as u16 | (get(p, BLAST_LIST_LEN, 2 * k + 1) as u16) << 8) as i16 as i32
    }
    fn blast_listed(&self, t: i32) -> bool {
        (0..30i64).any(|k| self.blast_word(k) == t)
    }
    /// BlastListRemove? 0x3B718: the first word equal to `t` -> 0xC41.
    fn blast_list_remove(&self, t: i32) {
        let p = self.ut_w().blast_list;
        if let Some(k) = (0..30i64).find(|&k| self.blast_word(k) == t) {
            put(p, BLAST_LIST_LEN, 2 * k, NONE as u8);
            put(p, BLAST_LIST_LEN, 2 * k + 1, (NONE >> 8) as u8);
        }
    }
    /// Blast list add (combat.md).
    fn blast_list_add(&self, t: i32) {
        let p = self.ut_w().blast_list;
        for k in 0..30i64 {
            let v = self.blast_word(k);
            if v == t {
                return;
            }
            if v == NONE {
                put(p, BLAST_LIST_LEN, 2 * k, t as u8);
                put(p, BLAST_LIST_LEN, 2 * k + 1, (t >> 8) as u8);
                return;
            }
        }
    }
}

impl Ai {
    /// combat.md "Picking up a ground item, exact steps" (op 343).
    fn pick_up(&self, tile: i32, cursor: u8) {
        let slot = self.slot();
        let me = self.rd8(0x17A);
        // 1.
        crate::sector::ja1_take_ground_item(self.w, self.h, tile, 1);
        // 2.
        let held = || get(self.ut_w().held, HELD_LEN, 0);
        let typ = held();
        // 3.
        if cursor != 0 {
            self.call(CURSOR_TO_SOLDIER, &[slot]);
        }
        // 4.
        let Some(&(_, quote, flag)) = STORY_PICKUPS.iter().find(|p| p.0 == typ) else {
            return;
        };
        // 5.
        if held() != typ && super::storyday::squad_search(self.w, self, typ, false, false) == -1 {
            return;
        }
        // 6.
        let story = self.ut_w().story;
        if get(story, STORY_LEN, flag) != 0 {
            return;
        }
        self.call(op::QUOTE_EVENT, &[me as i32, quote, 0, NONE, 1]);
        put(story, STORY_LEN, flag, 1);
    }
}

impl Ai {
    /// combat.md "Mine spotted, exact steps" (op 247).
    fn mine_spotted(&self, tile: i32, g: u8) -> u8 {
        if crate::fanfix::on(56) {
            // F56 (1.15l changelog: a trapped explosive taken through some item routes skipped the defuse check;
            // found by DarzaR): 1.15m MineSpotted? 0x3C328 -> cave 0x42770 returns 0 for ground entry 0xFF;
            // 0x3C346 -> cave 0x1B6D6: types 0x16/0x18/0x1A/0x1C/0x1D return -FxHere?(soldier) (own tile).
            if g == 0xFF {
                return 0;
            }
            let ty = self.ut_ge(g, 0);
            if ty != 0x11 {
                return if matches!(ty, 0x16 | 0x18 | 0x1A | 0x1C | 0x1D) {
                    (self.effect_here(s16(self.rd16(0x114) as i32)) as u8).wrapping_neg()
                } else {
                    0
                };
            }
        }
        // 1.
        if self.ut_ge(g, 0) != 0x11 || self.ut_ge(g, 6) as i8 != -1 || self.rd8(0x17A) >= 8 || self.ut_ge(g, 2) as i8 <= 0 {
            return 0;
        }
        // 2.
        if !self.blast_listed(tile) {
            self.blast_list_add(tile);
        }
        // 3.
        self.ut_ge_put(g, 2, self.ut_ge(g, 2).wrapping_neg());
        1
    }

    /// Tile byte 1 = 0 and all move codes rebuilt (a blue flag removed).
    fn remove_flag(&self, tile: i32) {
        self.ut_tile_put(tile, 1, 0);
        if crate::flag_cleared_codes(self.w, tile) {
            return;
        }
        let w = self.ut_w();
        // SPEC GAP: a null tiles, t0_class, t1_class or move_codes buffer skips the rebuild.
        if w.tiles.is_null() || w.t0_class.is_null() || w.t1_class.is_null() || w.move_codes.is_null() {
            return;
        }
        unsafe {
            let tiles = std::slice::from_raw_parts(w.tiles as *const u8, TILES * 8);
            let gc = &*(w.t0_class as *const [u8; 256]);
            let oc = &*(w.t1_class as *const [u8; 256]);
            let mut out = vec![0u16; TILES];
            crate::rebuild(tiles, gc, oc, &mut out);
            for (i, c) in out.iter().enumerate() {
                let [lo, hi] = c.to_le_bytes();
                *w.move_codes.add(2 * i) = lo;
                *w.move_codes.add(2 * i + 1) = hi;
            }
        }
    }

    /// combat.md "Disarming an explosive, exact steps" (op 248).
    fn disarm(&self, tile: i32, g: u8, armed: u8, menu: u8) -> i32 {
        let slot = self.slot();
        let c = self.ut_ge(g, 2) as i8 as i32;
        let merc = self.rd8(0x17A) < 8;
        let ask = |kind: i32| self.call(DISARM_ASK, &[slot, kind]) & 0xFF != 0;
        let refresh = || {
            if menu != 0 {
                self.call(MENU_REDRAW, &[]);
            }
        };
        let cancel = || {
            self.wr16(0x134, NONE as u16);
            self.wr16(0x96, NONE as u16);
            self.wr8(0x1A6, 0);
            self.wr8(0x1A7, 0);
            self.wr8(0x95, 0);
        };
        let set_off = || {
            self.call(op::TRIGGER_ITEM, &[slot, tile, g as i32, 1]);
            -1
        };
        // 1.
        if !(c < -100 && armed != 0 || c < 0 && armed == 0) {
            return 1;
        }
        // 2.
        let known = !merc || self.blast_listed(tile);
        let mut spotted = known;
        // 3.
        if known && merc {
            if !ask(0) {
                cancel();
                if self.tb(tile, 1) == 0x86 && ask(1) {
                    self.remove_flag(tile);
                }
                return 0;
            }
            refresh();
        }
        // 4.
        if !known {
            let mut v = c.abs();
            if v > 100 {
                v -= 100;
            }
            if self.tb(tile, 1) == 0x86 {
                spotted = true;
            }
            if self.s8(0x18F) / 30 + self.s8(0x192) >= v {
                self.blast_list_add(tile);
                spotted = true;
                self.call(op::STAT_PRACTICE, &[slot, 4, v, 0]);
                self.call(op::STAT_PRACTICE, &[slot, 6, v, 0]);
                self.call(op::MERC_MESSAGE, &[slot, 0x13, 0x40, 0, menu as i8 as i32]);
                refresh();
                if !ask(0) {
                    cancel();
                    return 0;
                }
                refresh();
            }
        }
        // 5.
        if !spotted || !merc {
            return set_off();
        }
        // 6.
        let chance = self.s8(0x18F) * 70 / 100 + self.s8(0x18C) * 20 / 100 + self.s8(0x192) - (100 - self.s8(0x18D)) / 5;
        if self.random(100) >= chance {
            self.call(op::STAT_PRACTICE, &[slot, 6, 5, 0]);
            return set_off();
        }
        // 7.
        self.call(op::STAT_PRACTICE, &[slot, 6, 10, 0]);
        match self.ut_ge(g, 0) {
            0x11 => {
                let r = self.random(9);
                self.ut_ge_put(g, 6, 1);
                self.ut_ge_put(g, 2, (r + 1) as u8);
                self.call(GROUND_ITEM_RESET, &[g as i32]);
            }
            0x14 | 0x16 | 0x18 | 0x1A | 0x1C | 0x1D => self.ut_ge_put(g, 2, self.ut_ge(g, 2).wrapping_neg()),
            t @ (0x13 | 0x2C | 0x72 | 0x29 | 0x3F | 0x71) => {
                self.ut_ge_put(g, 0, if matches!(t, 0x13 | 0x2C | 0x72) { 0x40 } else { 0x21 });
                let r = self.random(10);
                self.ut_ge_put(g, 2, (r + 1) as u8);
                self.ut_ge_put(g, 1, 1);
                self.ut_ge_put(g, 6, 1);
                self.call(GROUND_ITEM_RESET, &[g as i32]);
                if self.tb(tile, 1) == 0x86 {
                    self.remove_flag(tile);
                }
            }
            _ => {
                let r = self.random(0x1F);
                self.ut_ge_put(g, 2, (r + 0x46) as u8);
            }
        }
        // 8.
        let typ = self.ut_ge(g, 0) as i64;
        if self.ut_ge(g, 1) as i8 > 1 && get(self.ut_w().item_types, ITEM_TYPES_LEN, 12 * typ + 6) > 1 {
            let base = 12 * g as usize;
            let mut j = 0usize;
            while (j as i64) < self.ut_ge(g, 1) as i8 as i64 - 1 {
                let r = self.random(10);
                let x = self.ut_ge(g, 2).wrapping_add(r as u8).wrapping_sub(5);
                self.ge_byte_put(base + 8 + j, if x as i8 > 0 { x } else { 1 });
                if self.ge_byte(base + 8 + j) as i8 > 100 {
                    self.ge_byte_put(base + 8 + j, 100);
                }
                j += 1;
            }
        }
        // 9.
        1
    }
}

/// Op 247 mine_spotted for `Ai::dispatch`.
pub(super) fn mine_spotted(w: *mut Ja1World, h: *const Ja1Host, slot: i32, tile: i32, g: u8) -> u8 {
    match Ai::new(w, h, slot) {
        Some(a) => std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| a.mine_spotted(tile, g))).unwrap_or(0),
        None => 0,
    }
}

#[no_mangle]
pub extern "C" fn ja1_mine_spotted(w: *mut Ja1World, h: *const Ja1Host, slot: i32, tile: i32, g: i32) -> i32 {
    mine_spotted(w, h, slot, tile, g as u8) as i32
}

/// Op 248 disarm for `Ai::dispatch`.
pub(super) fn disarm(w: *mut Ja1World, h: *const Ja1Host, slot: i32, tile: i32, g: u8, armed: u8, menu: u8) -> i32 {
    match Ai::new(w, h, slot) {
        Some(a) => std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| a.disarm(tile, g, armed, menu))).unwrap_or(0),
        None => 0,
    }
}

#[no_mangle]
pub extern "C" fn ja1_disarm(w: *mut Ja1World, h: *const Ja1Host, slot: i32, tile: i32, g: i32, armed: i32, menu: i32) -> i32 {
    disarm(w, h, slot, tile, g as u8, armed as u8, menu as u8)
}

/// Op 346 ground_item_reset for `Ai::dispatch`: sector_start.md "Item reset" (ground entry g, 0, 0).
pub(crate) fn ground_item_reset(w: *mut Ja1World, h: *const Ja1Host, g: u8) {
    // SPEC GAP: g of 200 or more, or no ground items: nothing.
    let p = unsafe { (*w).ground_items };
    if p.is_null() || g as usize >= 200 {
        return;
    }
    crate::sector::ja1_item_init(w, h, unsafe { p.add(12 * g as usize) }, 0, 0);
}

/// Op 343 pick_up for `Ai::dispatch`.
pub(super) fn pick_up(w: *mut Ja1World, h: *const Ja1Host, slot: i32, tile: i32, cursor: u8) {
    if let Some(a) = Ai::new(w, h, slot) {
        let _ = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| a.pick_up(tile, cursor)));
    }
}

#[no_mangle]
pub extern "C" fn ja1_pick_up(w: *mut Ja1World, h: *const Ja1Host, slot: i32, tile: i32, cursor: i32) {
    pick_up(w, h, slot, tile, cursor as u8)
}

/// Op 73 pickup_here for `Ai::dispatch`.
pub(super) fn pickup_here(w: *mut Ja1World, h: *const Ja1Host, slot: i32) {
    if let Some(a) = Ai::new(w, h, slot) {
        let _ = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| a.pickup_here()));
    }
}

#[no_mangle]
pub extern "C" fn ja1_pickup_here(w: *mut Ja1World, h: *const Ja1Host, slot: i32) {
    pickup_here(w, h, slot)
}

/// Sector byte i + 1 (8-bit).
fn put_b(p: *mut u8, i: i64) {
    put(p, SECTOR_ROWS * SECTOR_ROW, i, get(p, SECTOR_ROWS * SECTOR_ROW, i).wrapping_add(1));
}

/// Op 75 bury_mine for `Ai::dispatch`.
pub(super) fn bury_mine(w: *mut Ja1World, h: *const Ja1Host, slot: i32, tile: i32) {
    if let Some(a) = Ai::new(w, h, slot) {
        let _ = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| a.bury_mine(tile)));
    }
}

#[no_mangle]
pub extern "C" fn ja1_bury_mine(w: *mut Ja1World, h: *const Ja1Host, slot: i32, tile: i32) {
    bury_mine(w, h, slot, tile)
}

/// Op 74 use_on_tile for `Ai::dispatch`.
pub(super) fn use_on_tile(w: *mut Ja1World, h: *const Ja1Host, slot: i32, tile: i32, flag: u8) {
    if let Some(a) = Ai::new(w, h, slot) {
        let _ = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| a.use_on_tile(tile, flag)));
    }
}

#[no_mangle]
pub extern "C" fn ja1_use_on_tile(w: *mut Ja1World, h: *const Ja1Host, slot: i32, tile: i32, flag: i32) {
    use_on_tile(w, h, slot, tile, flag as u8)
}

/// Op 176 land_item for `Ai::dispatch`.
pub(super) fn land_item(w: *mut Ja1World, h: *const Ja1Host, slot: i32, tile: i32, item: u8, cond: i32, x: i32, y: i32) -> i32 {
    if let Some(a) = Ai::new(w, h, slot) {
        return std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| a.land_item(tile, item, cond, x, y))).unwrap_or(tile);
    }
    tile
}

#[no_mangle]
pub extern "C" fn ja1_land_item(w: *mut Ja1World, h: *const Ja1Host, slot: i32, tile: i32, item: i32, cond: i32, x: i32, y: i32) -> i32 {
    land_item(w, h, slot, tile, item as u8, cond, x, y)
}

/// Op 118 mine_flag for `Ai::dispatch`.
pub(super) fn mine_flag(w: *mut Ja1World, h: *const Ja1Host, slot: i32, tile: i32) {
    if let Some(a) = Ai::new(w, h, slot) {
        let _ = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| a.mine_flag(tile)));
    }
}

#[no_mangle]
pub extern "C" fn ja1_mine_flag(w: *mut Ja1World, h: *const Ja1Host, slot: i32, tile: i32) {
    mine_flag(w, h, slot, tile)
}

const WALTER_SAID_LEN: usize = 12;
const WALTER_NEXT_LEN: usize = 8;
const CRATES_LEN: usize = 16;

impl Ai {
    fn wal_w(&self, k: i64) -> i32 {
        get(self.ut_w().walter_said, WALTER_SAID_LEN, k) as i8 as i32
    }
    fn wal_c(&self, k: i64) -> i32 {
        get(self.ut_w().walter_next, WALTER_NEXT_LEN, k) as i8 as i32
    }
    fn wal_cur(&self) -> i32 {
        self.ut_w().cur_sector as i32
    }
    /// W[b] = cur_sector and walter_sector = cur_sector.
    fn wal_said(&self, b: i64) {
        let cur = self.ut_w().cur_sector;
        put(self.ut_w().walter_said, WALTER_SAID_LEN, b, cur);
        self.ut_w().walter_sector = cur;
    }
    fn wal_here(&self) -> bool {
        self.ut_w().walter_sector == self.ut_w().cur_sector
    }
    /// Story (a, b, last); `p` = None for "last".
    fn wal_story(&self, q: &mut i32, a: usize, b: i64, p: Option<i64>) {
        if *q != -1 || self.wal_here() || self.story(5 + a) as i8 != 1 || self.wal_w(b) != 99 {
            return;
        }
        if let Some(p) = p {
            let wp = self.wal_w(p);
            if wp == 99 || self.wal_cur() == wp {
                return;
            }
        }
        *q = b as i32 + 0x6E;
        self.wal_said(b);
    }
    /// q = C[k]; C[k] + 1 (8 bits); C[k] above `top` (signed): C[k] = `back`.
    fn wal_count(&self, k: i64, top: i32, back: u8) -> i32 {
        let p = self.ut_w().walter_next;
        let q = self.wal_c(k);
        put(p, WALTER_NEXT_LEN, k, (q as u8).wrapping_add(1));
        if self.wal_c(k) > top {
            put(p, WALTER_NEXT_LEN, k, back);
        }
        q
    }

    /// combat.md "The listening device at a door, exact steps" (op 67).
    fn walter(&self, tile: i32) {
        let slot = self.slot();
        // 1.
        if self.rd8(0) != 0x45 {
            return;
        }
        let mut q: i32 = -1;
        'pick: {
            // 2.
            if self.s8(2) <= 10 {
                q = 0;
                break 'pick;
            }
            // 3.
            if self.jam_check() == 0 {
                return;
            }
            // 4.
            let c = self.t1_class(self.tb(tile, 1));
            if c < 7 {
                return;
            }
            // 5.
            if c == 9 {
                let g = self.ut_ground(tile);
                q = if g != 0xFF && (self.ut_ge(g, 2) as i8) < 0 { 1 } else { 2 };
                break 'pick;
            }
            // 6.
            let mut count: i32 = 0;
            for k in 40..48usize {
                if self.rd8_of(k, 0x182) != 0
                    && self.rd8_of(k, 0x183) as i8 >= 15
                    && self.t0_class(self.rd8_of(k, 0x1B2)) == 3
                    && self.rd8_of(k, 0x9C) as i8 <= 0
                {
                    let t = (self.rd8_of(k, 0x114) as u16 | (self.rd8_of(k, 0x115) as u16) << 8) as i16 as i32;
                    let d = self.call(op::TILE_DIST, &[t, tile]) as i32;
                    if d < 3 {
                        count += 1;
                    } else if d < 6 {
                        count += 100;
                    }
                }
            }
            let (near, far) = ((count % 100) as u8 as i32, (count / 100) as u8 as i32);
            let alarm = self.ut_w().alarm != 0;
            // 7.
            if near >= 2 {
                self.wal_story(&mut q, 0, 7, None);
                self.wal_story(&mut q, 7, 8, None);
                self.wal_story(&mut q, 1, 9, None);
                self.wal_story(&mut q, 2, 9, None);
                // e.
                if q == -1 && !self.wal_here() && self.story(0xB) as i8 == 1 {
                    let cur = self.wal_cur();
                    let said = self.ut_w().walter_said;
                    if self.wal_w(0) == 99 {
                        q = 0x6E;
                        put(said, WALTER_SAID_LEN, 0, cur as u8);
                    } else if self.wal_w(1) == 99 {
                        if cur != self.wal_w(0) {
                            q = 0x6F;
                            put(said, WALTER_SAID_LEN, 1, cur as u8);
                        }
                    } else if self.wal_w(2) == 99 && cur != self.wal_w(1) {
                        q = 0x70;
                        put(said, WALTER_SAID_LEN, 2, cur as u8);
                    }
                    if q != -1 {
                        self.ut_w().walter_sector = cur as u8;
                    }
                }
                // f.
                if q == -1 && self.story(0xA) as i8 == 1 {
                    self.wal_story(&mut q, 5, 3, None);
                    self.wal_story(&mut q, 5, 4, Some(3));
                }
                // g.
                if q == -1 && !self.wal_here() {
                    let day = self.ut_w().day;
                    let cur = self.wal_cur();
                    for k in 0..3i64 {
                        let d = get(self.ut_w().crates, CRATES_LEN, 4 * k + 1) as i8 as i32;
                        if d > day && d - 5 <= day {
                            if self.wal_w(0xA) == 99 {
                                q = 0x78;
                                self.wal_said(0xA);
                            } else if self.wal_w(0xA) != cur && self.wal_w(0xB) == 99 {
                                q = 0x79;
                                self.wal_said(0xB);
                            }
                            break;
                        }
                    }
                }
                // h.
                if q == -1 && self.story(0xE) as i8 == 1 && self.ut_w().day > 10 {
                    self.wal_story(&mut q, 9, 5, None);
                    self.wal_story(&mut q, 9, 6, Some(5));
                }
                // i.
                if q == -1 {
                    q = if alarm { self.wal_count(2, 0x3B, 0x28) } else { self.wal_count(3, 0x27, 0x14) };
                }
            } else if near + far >= 2 {
                q = if alarm { self.wal_count(0, 0x6D, 0x5A) } else { self.wal_count(1, 0x59, 0x46) };
            } else if near == 1 {
                q = if alarm { self.wal_count(6, 0x13, 0xA) } else { self.wal_count(7, 0x40, 0x3C) };
            } else if far == 1 {
                q = if alarm { self.wal_count(0, 0x45, 0x41) } else { self.wal_count(1, 0x40, 0x3C) };
            } else {
                q = 2;
            }
        }
        // 8.
        self.call(op::WALTER_SAY, &[q]);
        // 9.
        if q == 1 {
            self.blast_list_add(tile);
            self.call(op::MERC_MESSAGE, &[slot, 0x13, 0x40, 0, 1]);
            self.call(op::STAT_PRACTICE, &[slot, 4, 10, 0]);
        }
        if q >= 0x6E {
            self.call(op::STAT_PRACTICE, &[slot, 4, 5, 0]);
        }
    }
}

/// Op 69 use_special for `Ai::dispatch`.
pub(super) fn use_special(w: *mut Ja1World, h: *const Ja1Host, slot: i32) {
    if let Some(a) = Ai::new(w, h, slot) {
        let _ = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| a.use_special()));
    }
}

#[no_mangle]
pub extern "C" fn ja1_use_special(w: *mut Ja1World, h: *const Ja1Host, slot: i32) {
    use_special(w, h, slot)
}

/// Op 67 walter for `Ai::dispatch`.
pub(super) fn walter(w: *mut Ja1World, h: *const Ja1Host, slot: i32, tile: i32) {
    if let Some(a) = Ai::new(w, h, slot) {
        let _ = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| a.walter(tile)));
    }
}

#[no_mangle]
pub extern "C" fn ja1_walter(w: *mut Ja1World, h: *const Ja1Host, slot: i32, tile: i32) {
    walter(w, h, slot, tile)
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

#[cfg(test)]
mod guard_world_tests {
    use super::*;

    #[test]
    fn guard_ut_tile_put_range() {
        // Port rule (SPEC GAP): a tile write off the map is dropped.
        let mut tiles = vec![0u8; (TILES + 1) * 8 + 8];
        let mut w: Box<Ja1World> = Box::new(unsafe { std::mem::zeroed() });
        w.tiles = unsafe { tiles.as_mut_ptr().add(8) };
        let h = Ja1Host { ctx: std::ptr::null_mut::<std::os::raw::c_void>(), random: None, call: None, host_ops: 0 };
        let ai = Ai { w: &mut *w, h: &h, s: 0 };
        ai.ut_tile_put(TILES as i32, 2, 9);
        ai.ut_tile_put(-1, 2, 9);
        assert!(tiles.iter().all(|&b| b == 0));
        ai.ut_tile_put(0, 2, 9);
        assert_eq!(tiles[10], 9);
    }

    /// Ledger row ja1logic:19167: PlayWalterQuote 0x3C574 counts C[2] (byte 0x22A0C5) up and puts it back to 0x28 only
    /// when the new value is above 0x3B (asm 0x3C80B..0x3C821: `inc al`, store, `cmp eax,0x3b; jle`, `mov 0x28`).
    #[test]
    fn walter_counter_wraps_above_top() {
        let mut next = [0u8; 8];
        next[2] = 0x3A;
        let mut w: Box<Ja1World> = Box::new(unsafe { std::mem::zeroed() });
        w.walter_next = next.as_mut_ptr();
        let h = Ja1Host { ctx: std::ptr::null_mut::<std::os::raw::c_void>(), random: None, call: None, host_ops: 0 };
        let ai = Ai { w: &mut *w, h: &h, s: 0 };
        assert_eq!(ai.wal_count(2, 0x3B, 0x28), 0x3A);
        assert_eq!(ai.wal_c(2), 0x3B, "0x3B is not above the top: kept");
        assert_eq!(ai.wal_count(2, 0x3B, 0x28), 0x3B);
        assert_eq!(ai.wal_c(2), 0x28);
    }

    /// Ledger row ja1logic:18838: MineFlagReroute 0x19D74 puts object 0x86 and calls MoveCode 0x2BE54, which reads the
    /// old code as the word g_moveCode[tile] (asm 0x2BE6E `mov si, word ptr [edx + 0x108258]`); a door code 25300 is
    /// never lowered. Object 0x86 (t1 class 9 in JAGGED.DAT) under roof 0x73 gives 1201, so a door tile keeps 25300.
    #[test]
    fn mine_flag_keeps_a_door_code() {
        let mut tiles = vec![0u8; 3136 * 8];
        tiles[100 * 8 + 3] = 0x73;
        let (mut t0, mut t1) = ([0u8; 256], [0u8; 256]);
        t1[0x86] = 9;
        let mut codes = vec![0u8; 3136 * 2];
        codes[200..202].copy_from_slice(&25300u16.to_le_bytes());
        codes[198..200].copy_from_slice(&1000u16.to_le_bytes());
        codes[202..204].copy_from_slice(&1000u16.to_le_bytes());
        let mut w: Box<Ja1World> = Box::new(unsafe { std::mem::zeroed() });
        (w.tiles, w.t0_class, w.t1_class, w.move_codes) =
            (tiles.as_mut_ptr(), t0.as_mut_ptr(), t1.as_mut_ptr(), codes.as_mut_ptr());
        let h = Ja1Host { ctx: std::ptr::null_mut::<std::os::raw::c_void>(), random: None, call: None, host_ops: 0 };
        let ai = Ai { w: &mut *w, h: &h, s: 0 };
        ai.mine_flag(100);
        assert_eq!(tiles[100 * 8 + 1], 0x86);
        assert_eq!(u16::from_le_bytes([codes[200], codes[201]]), 25300);
        codes[200..202].copy_from_slice(&1000u16.to_le_bytes());
        let ai = Ai { w: &mut *w, h: &h, s: 0 };
        ai.mine_flag(100);
        assert_eq!(u16::from_le_bytes([codes[200], codes[201]]), 1201);
    }
}
