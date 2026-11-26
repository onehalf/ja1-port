//! Doors (docs/spec/combat.md "Doors"; C interface in docs/spec/port_api.md). A child module of `ai`: the door
//! value, opening or breaking an object, closing a door, using a key and the door effect. Host calls go through
//! `Ai::call` (the host_ops rule); world data is read again after every host call.
use super::{op, Ai, Ja1Host, Ja1World, TILES};

const SECTOR_ROWS: usize = 60;
const SECTOR_ROW: usize = 0xB1;
const DOOR_PAIRS: usize = 12;
const CLOSED_DOOR: u16 = 1500;
const OPEN_DOOR: u16 = 25400;
const KEY: u8 = 0x57;
const BIG_KEY: u8 = 0x58;
const LOCK_PICK: u8 = 0x4C;
/// Item entry indexes in pocket order (pocket number i = position in this list).
const POCKETS: [usize; 7] = [0, 7, 8, 9, 10, 11, 1];
const ENTRY: usize = 12;

// SPEC GAP: a tile outside 0..3135 (or a null tile table) reads as 0 and is not written.
fn tile_get(w: *mut Ja1World, t: i32, i: usize) -> u8 {
    let p = unsafe { (*w).tiles };
    if p.is_null() || !(0..TILES as i32).contains(&t) {
        return 0;
    }
    unsafe { p.add(t as usize * 8 + i).read() }
}
fn tile_put(w: *mut Ja1World, t: i32, i: usize, v: u8) {
    let p = unsafe { (*w).tiles };
    if p.is_null() || !(0..TILES as i32).contains(&t) {
        return;
    }
    unsafe { p.add(t as usize * 8 + i).write(v) }
}
fn move_put(w: *mut Ja1World, t: i32, v: u16) {
    let p = unsafe { (*w).move_codes };
    if p.is_null() || !(0..TILES as i32).contains(&t) {
        return;
    }
    let [lo, hi] = v.to_le_bytes();
    unsafe {
        *p.add(2 * t as usize) = lo;
        *p.add(2 * t as usize + 1) = hi;
    }
}
fn obj_add(w: *mut Ja1World, t: i32, d: i32) {
    tile_put(w, t, 1, tile_get(w, t, 1).wrapping_add(d as u8));
}

/// Door value of a tile (library op 179): the value of the first pair of the current sector's door table whose
/// tile word equals the whole tile argument; none: 0.
pub(super) fn door_lookup(w: *mut Ja1World, tile: i32) -> i32 {
    // SPEC GAP: a current sector of 60 or more (or a null sectors table) has no doors (value 0).
    let p = unsafe { (*w).sectors };
    let s = unsafe { (*w).cur_sector } as usize;
    if p.is_null() || s >= SECTOR_ROWS {
        return 0;
    }
    let word = |o: usize| unsafe { i16::from_le_bytes([p.add(s * SECTOR_ROW + o).read(), p.add(s * SECTOR_ROW + o + 1).read()]) };
    for k in 0..DOOR_PAIRS {
        if word(4 * k) as i32 == tile {
            return word(4 * k + 2) as i32;
        }
    }
    0
}

fn sound(w: *mut Ja1World, h: *const Ja1Host, snd: i32) {
    if snd as u8 != 0 {
        if let Some(a) = Ai::new(w, h, 0) {
            a.call(op::SFX, &[0x18, 0x2B11, 0x32, 1]);
        }
    }
}

/// Opening or breaking an object (library op 107).
pub(super) fn break_object(w: *mut Ja1World, h: *const Ja1Host, tile: i32, object: u8, snd: i32) {
    match object {
        0x3E | 0x5D | 0x3D | 0x5C => {
            obj_add(w, tile, if matches!(object, 0x3E | 0x5D) { -3 } else { -2 });
            move_put(w, tile, OPEN_DOOR);
            obj_add(w, tile - 1, -2);
        }
        0x45 | 0x64 | 0x43 | 0x62 => {
            obj_add(w, tile, if matches!(object, 0x45 | 0x64) { -4 } else { -2 });
            let n = tile - 56;
            let o = tile_get(w, n, 1).wrapping_sub(2);
            tile_put(w, n, 1, o);
            tile_put(w, n, 2, if o == 0x42 { 0x45 } else { 0x4E });
            move_put(w, tile, OPEN_DOOR);
        }
        _ => return,
    }
    sound(w, h, snd);
}

/// Toggling doors by value (library op 342, combat.md): mode 1 opens the unstandable doors of the value without
/// sound, any other mode closes the empty ones with sound.
pub(super) fn toggle_doors(w: *mut Ja1World, h: *const Ja1Host, mode: u8, value: u8) {
    for t in 0..TILES as i32 {
        let obj = tile_get(w, t, 1);
        if !(0x3B..=0x64).contains(&obj) {
            continue;
        }
        if mode == 1 {
            if super::shared::tile_standable(w, h, t) < 0 && door_lookup(w, t) == value as i32 {
                if let Some(a) = super::Ai::new(w, h, 0) {
                    a.call(op::BREAK_OBJECT, &[t, obj as i32, 0]);
                }
            }
        } else if door_lookup(w, t) == value as i32 && tile_get(w, t, 5) == 0x40 {
            if let Some(a) = super::Ai::new(w, h, 0) {
                a.call(op::CLOSE_DOOR, &[t, obj as i32, 1]);
            }
        }
    }
}

/// Closing a door (library op 104).
pub(super) fn close_door(w: *mut Ja1World, h: *const Ja1Host, tile: i32, object: u8, snd: i32) {
    if tile_get(w, tile, 5) & 0x7F < 0x30 {
        return;
    }
    match object {
        0x3B | 0x5A => {
            obj_add(w, tile, 3);
            move_put(w, tile, CLOSED_DOOR);
            obj_add(w, tile - 1, 2);
        }
        0x41 | 0x60 => {
            obj_add(w, tile - 56, 2);
            tile_put(w, tile - 56, 2, 0);
            obj_add(w, tile, 2);
            move_put(w, tile, CLOSED_DOOR);
        }
        _ => return,
    }
    sound(w, h, snd);
}

impl Ai {
    fn panel_step(&self, i: usize) {
        if unsafe { (*self.w).selected } as i32 == self.slot() {
            self.call(op::ITEM_COND, &[i as i32, self.slot(), 0, 1]);
        }
    }
    /// Using a key: 1 when a key with id v fitted (and was used up), else 0.
    fn use_key(&self, big: bool, v: i32) -> i32 {
        let key = if big { BIG_KEY } else { KEY };
        for (i, &idx) in POCKETS.iter().enumerate() {
            let e = ENTRY * idx;
            if self.rd8(e) != key {
                continue;
            }
            let mut k = 0usize;
            while (k as i32) < self.s8(e + 1) {
                let count = self.s8(e + 1);
                if k == 0 && self.s8(e + 2) == v {
                    if count > 1 {
                        let last = e + (count + 6) as usize;
                        self.wr8(e + 2, self.rd8(last));
                        self.wr8(last, 0);
                        self.wr8(e + 1, (count - 1) as u8);
                    } else {
                        for b in 0..ENTRY {
                            self.wr8(e + b, 0);
                        }
                    }
                    self.panel_step(i);
                    return 1;
                }
                if k >= 1 && self.s8(e + k + 7) == v {
                    if k < 4 {
                        for m in k - 1..=2 {
                            self.wr8(e + 8 + m, self.rd8(e + 9 + m));
                        }
                        // The original game's mistake (bug list row 9): the record's byte 0x0B, not the entry's.
                        // Port fix P01 (#71, level all): the ring entry's own byte 0x0B.
                        self.wr8(if crate::fanfix::port_on(1) { e + 0x0B } else { 0x0B }, 0);
                    } else {
                        self.wr8(e + k + 7, 0);
                    }
                    self.wr8(e + 1, (count - 1) as u8);
                    self.panel_step(i);
                    return 1;
                }
                k += 1;
            }
        }
        0
    }

    /// The door film's screen place (PlayDoorFx 0x3B03C, before the door value): x = TileScreenX 0x1FB28 - 32,
    /// y = TileScreenY 0x1FB50 - 14 (truncating division), kept in the view window: x < left -> left, else
    /// x + 79 > right -> right - 79; y < top -> top, else y + 49 > bottom -> bottom - 49. The window of
    /// SetViewWindow 0x24880: left 48 when the view is narrower than 320, else 0; top 0 in mode 3, else 14;
    /// right = left + view_w - 1, bottom = top + view_h - 1.
    pub(super) fn film_place(&self, tile: i32) -> (i32, i32) {
        let w = unsafe { &*self.w };
        let left = if w.view_w < 320 { 48 } else { 0 };
        let top = if w.opt_onscreen == 3 { 0 } else { 14 };
        let (right, bottom) = (left + w.view_w - 1, top + w.view_h - 1);
        let mut x = 16 * (tile % 56) - w.view_x + left - 32;
        let mut y = 14 * (tile / 56) - w.view_y + top - 14;
        if x < left {
            x = left;
        } else if x + 79 > right {
            x = right - 79;
        }
        if y < top {
            y = top;
        } else if y + 49 > bottom {
            y = bottom - 49;
        }
        (x, y)
    }

    /// The door effect (library op 70).
    pub(super) fn door_fx(&self, tile: i32, task: u8) {
        let tile = super::search::s16(tile);
        let slot = self.slot();
        // 1.
        let obj = tile_get(self.w, tile, 1);
        self.call(op::DOOR_SND, &[0]);
        // 2.
        let mut v = door_lookup(self.w, tile) as i16 as i32;
        let big = v > 100;
        if big {
            v -= 100;
        }
        // 3.
        let r = if v == 0 {
            11
        } else {
            let it = if task == 0xFF { 0 } else { self.rd8(0) };
            if it == LOCK_PICK {
                let skill = (self.s8(0x18D) * self.s8(0x190)) / 100 * self.s8(0x18C) / 100 + self.s8(0x192) * 5;
                if (self.s8(2) * skill) / 100 >= v {
                    self.call(op::STAT_PRACTICE, &[slot, 7, 2, 0]);
                    self.call(op::STAT_PRACTICE, &[slot, 4, 2, 0]);
                    self.call(op::STAT_PRACTICE, &[slot, 3, 1, 0]);
                    10
                } else {
                    if self.random(100) < v && self.rd8(2) != 0 {
                        self.wr8(2, self.rd8(2).wrapping_sub(1));
                    }
                    7
                }
            } else if self.use_key(big, v) == 1 {
                9
            } else if it == KEY || it == BIG_KEY {
                6
            } else {
                8
            }
        };
        // 4.
        let (fx, fy) = self.film_place(tile);
        self.call(op::DOOR_ANIM, &[big as i32, r, 0, fx, fy]);
        // 5.
        if r == 8 && big {
            self.call(op::DOOR_SND, &[1]);
            return;
        }
        // 6.
        if r >= 9 {
            self.call(op::BREAK_OBJECT, &[tile, obj as i32, 1]);
            let l = self.random(3);
            self.call(op::MAKE_NOISE, &[slot, tile, self.rd8(0x1B3) as i32, l, 2]);
            self.wr8(0x9D, 1);
        }
        // 7.
        self.call(op::DOOR_ANIM, &[big as i32, r, 1, fx, fy]);
        // 8.
        self.panel_step(0);
        // 9.
        self.call(op::DOOR_SND, &[1]);
    }
}

#[no_mangle]
pub extern "C" fn ja1_door_lookup(w: *mut Ja1World, _h: *const Ja1Host, tile: i32) -> i32 {
    if w.is_null() {
        return 0;
    }
    door_lookup(w, tile)
}
#[no_mangle]
pub extern "C" fn ja1_break_object(w: *mut Ja1World, h: *const Ja1Host, tile: i32, object: i32, snd: i32) {
    if w.is_null() || h.is_null() {
        return;
    }
    break_object(w, h, tile, object as u8, snd)
}
#[no_mangle]
pub extern "C" fn ja1_close_door(w: *mut Ja1World, h: *const Ja1Host, tile: i32, object: i32, snd: i32) {
    if w.is_null() || h.is_null() {
        return;
    }
    close_door(w, h, tile, object as u8, snd)
}
#[no_mangle]
pub extern "C" fn ja1_door_fx(w: *mut Ja1World, h: *const Ja1Host, slot: i32, tile: i32, task: i32) {
    super::run(w, h, slot, (), |a| a.door_fx(tile, task as u8))
}

// ================= Reload (combat.md "Reload", library op 190) =================
const WEAPONS_LEN: usize = 256 * 6;
const ITEM_TYPES_LEN: usize = 136 * 12;
// SPEC GAP: an index outside the table (or a null table) reads 0.
fn table(p: *const u8, len: usize, i: usize) -> u8 {
    if p.is_null() || i >= len {
        0
    } else {
        unsafe { p.add(i).read() }
    }
}

impl Ai {
    /// Reload (library op 190): 0 = the gun in hand was reloaded, 1 = no clip.
    pub(super) fn reload(&self) -> i32 {
        let t = self.rd8(0);
        let a = table(unsafe { (*self.w).weapons }, WEAPONS_LEN, 6 * t as usize + 1);
        let Some((p, idx)) = POCKETS.iter().enumerate().find(|&(_, &idx)| self.rd8(ENTRY * idx) == a) else {
            return 1;
        };
        let clip = table(unsafe { (*self.w).item_types }, ITEM_TYPES_LEN, 12 * t as usize + 5);
        self.wr8(1, self.rd8(1).wrapping_add(clip));
        if self.s8(0x1D5) > 0 {
            self.call(op::STAT_PRACTICE, &[self.slot(), 3, 1, 0]);
        }
        self.call(op::SFX, &[if t < 8 { 0x0F } else { 0x0E }, 0x2B11, 0x32, 1]);
        let e = ENTRY * idx;
        let c = self.s8(e + 1);
        // Original 0x56090 clears the last round's sidecar byte before decrementing the clip stack.
        if c > 1 {
            self.wr8(e + (c + 6) as usize, 0);
        }
        let n = self.rd8(e + 1).wrapping_sub(1);
        self.wr8(e + 1, n);
        if n == 0 {
            for k in 0..ENTRY {
                self.wr8(e + k, 0);
            }
        }
        if unsafe { (*self.w).selected } as i32 == self.slot() {
            self.call(op::ITEM_COND, &[0, self.slot(), 0, 1]);
            self.call(op::ITEM_COND, &[p as i32, self.slot(), 0, 1]);
        }
        if unsafe { (*self.w).opt_step_text } != 0 {
            self.call(op::MERC_TEXT, &[self.slot(), 2, 800]);
        }
        0
    }
}
#[no_mangle]
pub extern "C" fn ja1_reload(w: *mut Ja1World, h: *const Ja1Host, slot: i32) -> i32 {
    super::run(w, h, slot, 1, |a| a.reload())
}

#[cfg(test)]
mod guard_tests {
    use super::*;

    #[test]
    fn guard_table_range() {
        // Port rule (SPEC GAP): an index outside the table or a null table reads 0; the byte past the end is 9.
        let buf = [7u8, 1, 2, 3, 4, 9];
        let p = buf[1..].as_ptr();
        assert_eq!(table(p, 4, 3), 4);
        assert_eq!(table(p, 4, 4), 0);
        assert_eq!(table(std::ptr::null(), 4, 4), 0);
    }
}

#[cfg(test)]
mod guard_world_tests {
    use super::*;

    #[test]
    fn guard_tile_get_move_put_range() {
        // Port rule (SPEC GAP): a tile off the map (below 0, 3136 and up) reads 0 and a move-code write is dropped.
        let mut tiles = vec![0u8; (TILES + 1) * 8 + 8];
        tiles[8 + (TILES - 1) * 8 + 2] = 3;
        tiles[8 + TILES * 8 + 2] = 5;
        tiles[2] = 5;
        let mut mc = vec![0u8; (TILES + 2) * 2];
        let mut w: Box<Ja1World> = Box::new(unsafe { std::mem::zeroed() });
        w.tiles = unsafe { tiles.as_mut_ptr().add(8) };
        w.move_codes = unsafe { mc.as_mut_ptr().add(2) };
        let wp: *mut Ja1World = &mut *w;
        assert_eq!(tile_get(wp, TILES as i32 - 1, 2), 3);
        assert_eq!(tile_get(wp, TILES as i32, 2), 0);
        assert_eq!(tile_get(wp, -1, 2), 0);
        move_put(wp, TILES as i32, 0x0101);
        move_put(wp, -1, 0x0101);
        assert!(mc.iter().all(|&b| b == 0));
    }
}
