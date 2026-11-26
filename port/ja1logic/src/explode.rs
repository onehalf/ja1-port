//! Craters and blast leftovers (docs/spec/combat.md "Craters and blast leftovers"; C interface in
//! docs/spec/port_api.md): `ja1_explode_dat` (library op 336 explode_art). A child module of `ai`; the file, take /
//! death / story calls are ops through `Ai::call`, Forget soldier is the library's own procedure.
use super::{op, Ai, Ja1Host, Ja1World, TILES};

const CELLS: [i32; 9] = [-57, -56, -55, -1, 0, 1, 55, 56, 57];
const FILL: [u8; 9] = [0, 0, 0, 0, 0, 7, 0, 8, 9];
/// (offset, object, byte 2) per 1000+ piece.
const PIECE: [(i32, u8, u8); 10] = [
    (-1, 0x71, 0x50),
    (-1, 0x52, 0x47),
    (-56, 0x72, 0),
    (-56, 0x53, 0),
    (1, 0x0C, 0x48),
    (1, 0x0E, 0x51),
    (0x37, 0x52, 0x47),
    (0x37, 0x71, 0x50),
    (0x38, 0x0B, 0),
    (0x38, 0x0D, 0),
];
/// (tile, difficulty): sector 0's tiles that stay whole.
const CACHE: [(i32, i8); 12] = [
    (0x15B, 0),
    (0x192, 0),
    (0x1C8, 0),
    (0x1C6, 0),
    (0x1FC, 0),
    (0x194, 1),
    (0x202, 2),
    (0x274, 0),
    (0x203, 2),
    (0x2AA, 1),
    (0x15D, 0),
    (0x159, 0),
];
const GROUND: usize = 200;
const SECTOR_STATE_LEN: usize = 80 * 9;
const STORY_LEN: usize = 35;
/// Story byte 0x1E (0x22A05E in the original): set when a 0x6E item becomes 0x7C.
const STORY_5E: usize = 0x1E;
const DOOR: u16 = 0x62D4;
const OPEN_DOOR: u16 = 0x6338;

// SPEC GAP: an index outside a world table (or a tile off the map) reads 0 and a write there is dropped.
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

/// Object class (o).
fn class(o: u8) -> u8 {
    match o {
        0 => 0,
        0x27..=0x36 => 6,
        1..=0x55 | 0x7F | 0x80 => 7,
        0x56..=0x7E | 0x81 | 0x82 => 8,
        _ => 9,
    }
}

struct X {
    w: *mut Ja1World,
    a: Ai,
}

impl X {
    fn w(&self) -> &mut Ja1World {
        unsafe { &mut *self.w }
    }
    fn tb(&self, x: i32, i: usize) -> u8 {
        get(self.w().tiles, TILES * 8, 8 * x as i64 + i as i64)
    }
    fn set(&self, x: i32, i: usize, v: u8) {
        if (0..TILES as i32).contains(&x) {
            put(self.w().tiles, TILES * 8, 8 * x as i64 + i as i64, v);
        }
    }
    fn t1(&self, x: i32) -> u8 {
        self.tb(x, 1)
    }
    fn t2(&self, x: i32) -> u8 {
        self.tb(x, 2)
    }
    fn c1(&self, x: i32, v: u8) {
        self.set(x, 1, v)
    }
    fn c2(&self, x: i32, v: u8) {
        self.set(x, 2, v)
    }
    /// "a/b := v1/v2".
    fn ab(&self, a: i32, b: i32, v1: u8, v2: u8) {
        self.c1(a, v1);
        self.c2(b, v2);
    }
    fn mark(&self, x: i32, v: u8) {
        self.set(x, 4, (self.tb(x, 4) & 0x80) | v);
    }
    fn q(&self, x: i32) -> u8 {
        (self.tb(x, 4) != 0) as u8
    }
    fn gi(&self, g: usize, i: usize) -> u8 {
        get(self.w().ground_items, GROUND * 12, (12 * g + i) as i64)
    }
    fn gi_put(&self, g: usize, i: usize, v: u8) {
        put(self.w().ground_items, GROUND * 12, (12 * g + i) as i64, v)
    }
    fn table(&self, p: *const u8, i: u8) -> u8 {
        get(p, 256, i as i64)
    }
    fn move_code(&self, t: i32) -> u16 {
        get(self.w().move_codes, TILES * 2, 2 * t as i64) as u16
            | (get(self.w().move_codes, TILES * 2, 2 * t as i64 + 1) as u16) << 8
    }
    fn move_code_put(&self, t: i32, v: u16) {
        put(self.w().move_codes, TILES * 2, 2 * t as i64, v as u8);
        put(self.w().move_codes, TILES * 2, 2 * t as i64 + 1, (v >> 8) as u8);
    }
    /// Op 87 read (2) as a signed 16-bit word.
    fn word(&self) -> i32 {
        let mut b = [0u8; 2];
        // SPEC GAP: a short read leaves the missing bytes 0.
        self.a.call_out(op::READ, &[2], &mut b);
        i16::from_le_bytes(b) as i32
    }

    // ---- Marks ----
    fn marks(&self, x: i32, c: i32) {
        // 1.
        let o = self.t1(x);
        let (mut at, mut d) = (x, 0u8);
        match o {
            6 => {
                at = x + 56;
                if self.q(x + 56) == 0x50 {
                    d = 7;
                } else if self.q(x + 57) == 0x50 {
                    self.mark(x + 57, 0x4D);
                } else if self.t1(x - 1) != 0x52 {
                    d = 0x1E;
                }
                if self.q(x + 1) == 0x51 {
                    self.mark(x + 1, 0x4E);
                }
                if matches!(self.q(x + 57), 0x49 | 0x4E | 0x46 | 0x4C) {
                    self.mark(x + 57, 0x63);
                }
                if matches!(self.q(x + 1), 0x4F | 0x4B) {
                    self.mark(x + 1, 0x63);
                }
                if self.q(x + 57) == 0x51 {
                    self.mark(x + 57, 0x4F);
                }
            }
            0x0B => {
                at = x + 1;
                d = if self.q(x + 1) == 0x4E { 0x0C } else { 0x0A };
            }
            0x0C => {
                at = x + 56;
                d = if self.q(x + 56) == 0x4C { 0x0B } else { 8 };
            }
            0x0F => {
                if c as u8 == 6 {
                    self.mark(x + 56, 0x63);
                } else {
                    for y in [x, x - 1, x - 56, x - 57] {
                        self.mark(y, 0x63);
                    }
                }
            }
            0x22 | 0x25 => d = 0x1E,
            0x52 => {
                at = x + 57;
                if self.q(x + 57) != 0x50 {
                    d = 7;
                }
            }
            0x53 => {
                at = x + 57;
                d = 9;
            }
            _ => {}
        }
        // 2.
        if d != 0 {
            self.mark(at, d + 0x45);
        }
    }

    // ---- Explode dat ----
    fn explode_dat(&self, tile: i32, big: i32) {
        let t = tile as i16 as i32;
        let mut big = big;
        let w = self.w();
        // 1.
        if w.cur_sector == 0 && (w.santino_spoke as i8) < 2 {
            if (0x122..=0x124).contains(&t) {
                return;
            }
            if CACHE.iter().any(|&(ct, d)| ct == t && d <= w.difficulty as i8) {
                return;
            }
        }
        // 2.
        self.a.call(op::OPEN_PATCH, &[5, 0, 0]);
        // 3.
        let o0 = self.t1(t);
        let g = self.table(w.t0_class, self.tb(t, 0));
        if g == 3 {
            self.set(t, 0, (self.a.random(3) + 0x1D) as u8);
        }
        // 4.
        let i = self.tb(t, 6) as usize;
        if i < GROUND {
            match self.gi(i, 0) {
                0x5F => {
                    self.gi_put(i, 0, 0x7B);
                    self.gi_put(i, 1, 0x63);
                    self.gi_put(i, 6, 0);
                }
                0x6E => {
                    self.gi_put(i, 0, 0x7C);
                    self.gi_put(i, 1, 0x63);
                    put(w.story, STORY_LEN, STORY_5E as i64, 1);
                    self.gi_put(i, 6, 0);
                }
                ty => {
                    if ty == 0x3F && w.cur_sector == 0 && t == 0x15B && w.santino_spoke != 0 {
                        if let Some(s) = Ai::new(self.w, self.a.h, 0x28) {
                            s.forget_soldier();
                        }
                        let k = 9 * w.cur_sector as i64 + 1;
                        put(w.sector_state, SECTOR_STATE_LEN, k, get(w.sector_state, SECTOR_STATE_LEN, k).wrapping_add(1));
                        self.a.call(op::MERC_DIES, &[0x28, 0]);
                        self.a.call(op::STAGE_FIVE, &[]);
                    }
                    self.a.call(op::TAKE_GROUND_ITEM, &[t, 0]);
                }
            }
        }
        // 5.
        let mut c: i32 = 0;
        if big != 0 {
            c = class(o0) as i32;
            if c == 9 {
                big = 0;
            }
        }
        let big = big != 0;
        // 6.
        if o0 < 0x1B {
            if o0 == 0 && (1..=7).contains(&g) {
                let v = self.a.random(2) + if g == 3 { 3 } else { 1 };
                self.c1(t, v as u8);
            }
        } else {
            // 7.
            let oo = o0;
            for k in 0..9usize {
                let a = t + CELLS[k];
                let b = a - 56;
                // a.
                let o = self.t1(a);
                self.a.call(op::SEEK, &[4 * k as i32 + 36 * o as i32]);
                let mut v = self.word();
                // b.
                if v < 0xFF {
                    c = class(o) as i32;
                    if !(6..=8).contains(&c) || big {
                        self.c1(a, v as u8);
                    }
                    self.marks(a, c);
                    v = self.word();
                    if v != 0xFF {
                        c = self.table(w.t2_class, self.t2(b)) as i32;
                        if !matches!(c, 0x10 | 0x11) || big {
                            self.c2(b, v as u8);
                        }
                    }
                }
                // c.
                if (k == 0 || k == 3) && o < 7 && self.t2(b - 1) == 0x4A {
                    self.c2(b, 0x50);
                }
                // d.
                if (1..=0x82).contains(&o) && big {
                    self.big_fixes(t, oo, o, k, a, b, c);
                }
                // e.
                if (0x1B..=0x1D).contains(&o) {
                    self.c2(t, 0);
                }
                if (0x1E..=0x20).contains(&o) {
                    self.c2(t - 1, 0);
                }
                // f.
                if v >= 1000 && big {
                    self.piece(v - 1000, a, b);
                }
                // g.
                c = self.table(w.t1_class, oo) as i32;
                if (c == 7 || c == 8) && self.t1(a) < 10 {
                    if k == 5 || k == 7 {
                        self.c1(a, FILL[k]);
                    } else if k == 8 && self.t1(a - 56) < 15 && self.t1(a - 1) < 15 {
                        self.c1(a, FILL[8]);
                    }
                }
            }
        }
        // 8.
        self.a.call(op::CLOSE, &[]);
        self.move_code_at(t, o0);
        // #70 F34 (row 26): 1.15m 0x351A7 jumps to cave 0x18EC4, which runs the same step for the 8 tiles around
        // the centre (directions 8 down to 1, StepTile; object = the tile's object now), so a breach blown into a
        // wall next to the centre can be walked through at once.
        if crate::fanfix::on(34) {
            for d in (1..=8).rev() {
                let n = crate::ai::step_tile(t, self.a.dir_step(d));
                self.move_code_at(n, self.t1(n));
            }
        }
    }

    /// Step 8 for tile `t` with object `o`: a door class (2, 3) gets the door code, else the move code is built
    /// again unless it is a door code.
    fn move_code_at(&self, t: i32, o: u8) {
        let w = self.w();
        if matches!(self.table(w.t1_class, o), 2 | 3) {
            self.move_code_put(t, DOOR);
        } else {
            let prev = self.move_code(t);
            if prev != DOOR && prev != OPEN_DOOR && !w.t0_class.is_null() && !w.t1_class.is_null() {
                // One tile's move code by the rules of Movement (crate::move_code with the old code).
                let (gc, oc) = unsafe { (&*(w.t0_class as *const [u8; 256]), &*(w.t1_class as *const [u8; 256])) };
                let code = crate::move_code(self.tb(t, 0), self.tb(t, 1), self.tb(t, 3), prev, gc, oc);
                self.move_code_put(t, code);
            }
        }
    }

    /// Step 7d: the big-blast object fixes of cell k (old object o, centre object oo).
    #[allow(clippy::too_many_arguments)]
    fn big_fixes(&self, t: i32, oo: u8, o: u8, k: usize, a: i32, b: i32, c: i32) {
        // 1.
        if oo == 0x56 || oo == 0x66 {
            if k == 2 && o == 0x67 {
                self.ab(a, b, 0x72, 0);
            } else if k == 5 {
                if o == 0x57 || (0x75..=0x79).contains(&o) {
                    self.ab(a, b, 0x0E, 0x51);
                } else if o == 0x71 {
                    self.ab(a, b, 6, 0);
                } else if o == 0x5B {
                    self.ab(a, b, 6, 0);
                    self.ab(a + 1, b + 1, 0x0E, 0x51);
                }
            } else if k == 7 {
                if o == 0x6D {
                    self.ab(a - 56, b - 56, 0x0D, 0);
                } else if o == 0x61 {
                    self.ab(a, b, 6, 0);
                    self.ab(a + 56, b + 56, 0x0D, 0);
                }
            }
            if k == 7 && (o == 0x58 || (0x7A..=0x7E).contains(&o)) {
                self.ab(a, b, 0x0D, 0);
            }
        }
        // 2.
        if oo == 0x37 {
            if k == 7 && o == 0x39 {
                self.ab(a, b, 0x0B, 0);
            } else if k == 5 {
                match o {
                    0x38 => self.ab(a, b, 0x0C, 0),
                    0x52 => self.ab(a, b, 6, 0),
                    0x3C => {
                        self.ab(a, b, 6, 0);
                        self.ab(a + 1, b + 1, 0x0C, 0x48);
                    }
                    _ => {}
                }
            }
        }
        // 3.
        if oo == 0x57 && o == 0x67 && k == 6 {
            self.ab(a, b, 0x71, 0x50);
        }
        // 4.
        if oo == 0x67 {
            if o == 0x65 && k == 7 && self.t1(a - 1) == 6 {
                self.ab(a, b, 6, 0);
            } else if o == 0x57 && k == 6 && self.t1(a + 1) != 0x65 {
                self.ab(a, b, 0x71, 0x50);
            }
        }
        // 5.
        if (oo == 0x38 || oo == 0x52) && k == 3 && o == 0x37 && self.t1(a - 1) == 0 {
            self.ab(a, b, 6, 0);
            if self.t1(a - 1) == 0 && self.t2(b - 1) == 0x4A {
                self.c2(b - 1, 0x50);
            }
        }
        // 6.
        if oo == 0x4B && k == 3 && o == 0x37 {
            self.ab(a, b, 0x0B, 0);
        }
        // 7.
        if o == 0x56 && k == 1 {
            match self.t1(a - 56) {
                0x4E => self.ab(a - 56, b - 56, 0x53, 0),
                0x50 => self.ab(a - 56, b - 56, 0x52, 0x47),
                _ => {}
            }
        }
        // 8.
        if (o == 0x56 && k == 3 && self.t1(t - 2) == 0)
            || (oo == 0x56 && k == 5 && self.t1(t + 1) == 0x71)
            || (o == 0x72 && k == 7 && oo == 0x56)
            || (oo == 0x39 && k == 3 && o == 0x71)
        {
            self.ab(a, b, 6, 0);
        }
        // 9.
        if oo == 0x39 && (k == 3 || k == 6) && o == 0x57 {
            self.ab(a, b, 0x71, 0x50);
            self.c1(t, 6);
            self.c2(t - 56, 0);
            self.marks(t, c);
            self.c1(t + 56, if self.t1(t + 56) == 0x53 { 6 } else { 0x0B });
            self.c2(t, 0);
            self.marks(t + 56, c);
        }
        // 10.
        if oo == 0x4A && k == 5 {
            match o {
                0x38 => self.ab(a, b, 0x0C, 0x48),
                0x52 => self.ab(a, b, 6, 0),
                _ => {}
            }
            self.marks(a, c);
        }
        // 11.
        if k == 3 && ((o == 0x5D && self.t1(t - 2) == 0x5B) || (o == 0x5A && self.t1(t - 2) == 0x59)) {
            self.c1(t - 2, 0x71);
            self.c2(t - 58, 0x50);
        }
        // 12.
        if k == 3 && (((o == 0x3D || o == 0x3E) && self.t1(t - 2) == 0x3C) || (o == 0x3B && self.t1(t - 2) == 0x3A)) {
            self.c1(t - 2, 0x52);
            self.c2(t - 58, 0x47);
            self.marks(t - 2, c);
        }
        // 13.
        if k == 1 && (o == 0x64 || o == 0x62) && self.t1(t - 112) == 0x63 {
            self.c1(t - 112, 0x72);
        }
        // 14.
        if k == 1 && ((o == 0x41 && self.t1(t - 112) == 0x42) || ((o == 0x43 || o == 0x45) && self.t1(t - 112) == 0x44)) {
            self.c1(t - 112, 0x53);
            self.marks(t - 112, c);
        }
        // 15.
        if k == 0 && o == 0x57 && self.t1(t - 112) == 0x39 {
            self.c1(t - 112, 0x53);
            self.marks(t - 112, c);
            self.c1(t - 56, 6);
            self.c2(t - 112, 0);
            self.marks(t - 56, c);
        }
        // 16.
        if matches!(oo, 0x58 | 0x72 | 0x39) && matches!(self.t1(t - 112), 0x58 | 0x6D) && self.t1(t - 56) == 6 {
            self.c1(t - 112, 0x72);
        }
        // 17.
        if oo == 0x58 && self.t1(t - 112) == 0x0D && self.t1(t - 56) == 6 {
            self.c1(t - 112, 6);
        }
        // 18.
        if (oo == 0x58 || oo == 0x72) && self.t1(t - 112) == 0x4E {
            self.c1(t - 112, 0x53);
        }
        // 19.
        if oo == 0x49 && k == 7 {
            match o {
                0x39 => self.c1(a, 0x0B),
                0x53 => self.c1(a, 6),
                _ => {}
            }
            self.marks(a, c);
        }
        // 20.
        if k == 1 && (o == 0x66 || o == 0x47) && self.t1(a - 56) == 0 {
            self.c1(a, 6);
        }
    }

    /// Step 7f: the 1000+ piece v (0 and up) of cell a (byte 2 at b).
    fn piece(&self, v: i32, a: i32, b: i32) {
        let x = self.t1(a - 1);
        let y = self.t1(a - 56);
        match v {
            0..=9 => {
                self.ab(a, b, 6, 0);
                let (d, p, q) = PIECE[v as usize];
                self.ab(a + d, b + d, p, q);
            }
            10 => {
                if matches!(x, 0x57 | 0x0E | 0x5C | 0x5D | 0x75..=0x79) {
                    self.ab(a, b, 0x71, 0x50);
                } else if x == 0x0C {
                    self.ab(a - 1, b - 1, 6, 0);
                    self.ab(a, b, 6, 0);
                } else {
                    self.ab(a, b, 6, 0);
                }
            }
            11 => self.ab(a, b, if y == 0x6D { 0x72 } else { 6 }, 0),
            12 => {
                if y == 6 {
                    self.ab(a, b, 6, 0);
                    self.ab(a + 56, b + 56, 0x0D, 0);
                }
            }
            13 => {
                if matches!(x, 0x56..=0x58 | 0x75..=0x79) {
                    self.ab(a - 1, b - 1, 0x71, 0x50);
                } else {
                    self.ab(a - 2, b - 2, 0x71, 0x50);
                    self.ab(a - 1, b - 1, 6, 0);
                }
                self.ab(a, b, 0x53, 0);
            }
            14 => {
                if matches!(x, 0x37..=0x39 | 0x7F) {
                    self.ab(a, b, 0x52, 0x47);
                } else {
                    self.ab(a, b, 0x71, 0x50);
                }
            }
            15 => self.ab(a, b, if x == 0 { 6 } else { 0x53 }, 0),
            16 => {
                if y == 0x67 {
                    self.ab(a - 56, b - 56, 0x72, 0);
                }
                self.ab(a, b, 0x0E, 0x51);
            }
            17 => {
                if y == 0x72 {
                    self.ab(a, b, 6, 0);
                } else {
                    self.ab(a, b, 0x0E, 0x51);
                }
            }
            18 => {
                if self.t1(a + 56) != 0x72 {
                    self.ab(a, b, 6, 0);
                } else {
                    self.ab(a, b, 0x0D, 0);
                }
            }
            _ => {}
        }
    }
}

/// Op 336 explode_art for `Ai::dispatch`.
pub(super) fn explode_dat(w: *mut Ja1World, h: *const Ja1Host, tile: i32, big: i32) {
    // SPEC GAP: a null world or host does nothing.
    if let Some(a) = Ai::new(w, h, 0) {
        X { w, a }.explode_dat(tile, big);
    }
}

#[no_mangle]
pub extern "C" fn ja1_explode_dat(w: *mut Ja1World, h: *const Ja1Host, tile: i32, big: i32) {
    explode_dat(w, h, tile, big)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Port safety rule of `get` / `put` (their SPEC GAP; ledger row 19331): an index outside the table reads 0
    /// and a write there is dropped. The original has no range test: it reads the globals beside its tables.
    #[test]
    fn off_table_read_is_zero_and_write_is_dropped() {
        let mut b = [1u8, 2, 3, 4];
        assert_eq!(get(b.as_ptr(), 2, 1), 2);
        assert_eq!(get(b.as_ptr(), 2, 2), 0);
        assert_eq!(get(b[1..].as_ptr(), 2, -1), 0);
        put(b.as_mut_ptr(), 2, 2, 9);
        put(b.as_mut_ptr(), 2, 1, 8);
        assert_eq!(b, [1, 8, 3, 4]);
        assert_eq!(get(std::ptr::null(), 2, 0), 0);
    }
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
