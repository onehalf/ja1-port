//! A merc joins the team (docs/spec/strategic.md "A merc joins the team"; C interface `ja1_merc_join`). A child
//! module of `ai`: the host random goes through `Ai`, the item reset is that of `sector` (sector_start.md).
use super::damage::{get16, get8, put16, put8};
use super::{Ai, Ja1Host, Ja1World, REC};

const SOLDIERS_LEN: usize = 48 * REC;
const PROFILE: i64 = 0x5A;
const PROFILES_LEN: usize = 64 * 0x5A;
const PROGRESS_LEN: usize = 8 * 0x12;
const ITEM_TYPES_LEN: usize = 136 * 12;

const IN_CUR_SECTOR: usize = 0x182;
const HP_MAX: usize = 0x184;
const LEVEL: usize = 0x192;
const STATUS: usize = 0x19B;

/// Profile byte -> soldier byte (step 5).
const COPIES: [(usize, usize); 13] = [
    (0x18A, 0x2B), (0x18B, 0x2C), (0x18C, 0x42), (0x18D, 0x2E), (0x18E, 0x40), (0x18F, 0x43), (0x190, 0x41),
    (0x191, 0x2D), (0x192, 0x44), (0x19B, 0x4A), (0x19C, 0x2F), (0x17C, 0x49), (0x17B, 0x48),
];
/// The stat fields of the practice start (step 7).
const STATS: [usize; 8] = [HP_MAX, 0x18B, 0x18C, 0x18D, 0x18E, 0x18F, 0x190, 0x191];

struct Join {
    w: *mut Ja1World,
    h: *const Ja1Host,
    a: Ai,
    k: i64,
}

impl Join {
    fn sb(&self, off: usize) -> u8 {
        get8(unsafe { (*self.w).soldiers }, SOLDIERS_LEN, self.k * REC as i64 + off as i64)
    }
    fn sb_put(&self, off: usize, v: u8) {
        put8(unsafe { (*self.w).soldiers }, SOLDIERS_LEN, self.k * REC as i64 + off as i64, v)
    }
    fn prog(&self, i: i64) -> u16 {
        get16(unsafe { (*self.w).stat_progress }, PROGRESS_LEN, self.k * 0x12 + 2 * i)
    }
    fn prog_put(&self, i: i64, v: u16) {
        put16(unsafe { (*self.w).stat_progress }, PROGRESS_LEN, self.k * 0x12 + 2 * i, v)
    }
    fn item_type(&self, t: u8, off: i64) -> u8 {
        get8(unsafe { (*self.w).item_types }, ITEM_TYPES_LEN, t as i64 * 12 + off)
    }

    fn practice_start(&self) {
        self.prog_put(8, 0);
        for (i, &f) in STATS.iter().enumerate() {
            let i = i as i64;
            match self.sb(STATUS) {
                0 => self.prog_put(i, 0),
                1 => {
                    if self.sb(f) >= 100 {
                        self.prog_put(i, 0);
                    } else {
                        let r = self.a.random(100) as u16;
                        self.prog_put(i, r);
                        if (self.sb(LEVEL) as i8) < 10 {
                            self.prog_put(8, self.prog(8).wrapping_add(r));
                        }
                    }
                }
                2 => {
                    if self.sb(f) == 0 {
                        self.prog_put(i, 0);
                    } else {
                        let r = self.a.random(100) as u16;
                        self.prog_put(i, r.wrapping_neg());
                        if (self.sb(LEVEL) as i8) > 1 {
                            self.prog_put(8, self.prog(8).wrapping_sub(r));
                        }
                    }
                }
                _ => return,
            }
        }
    }

    fn join(&self, p: i32) {
        let pr = unsafe { (*self.w).profiles };
        let pb = |off: usize| get8(pr, PROFILES_LEN, p as i64 * PROFILE + off as i64);
        for j in 0..0x5A {
            let c = pb(j);
            self.sb_put(0x16E + j, c);
            if c == 0 {
                break;
            }
        }
        self.sb_put(0x17A, self.k as u8);
        self.sb_put(0x179, p as u8);
        for o in [0x16A, 0x16B, 0x16C, 0x194] {
            self.sb_put(o, 0);
        }
        self.sb_put(IN_CUR_SECTOR, 1);
        let salary = get16(pr, PROFILES_LEN, p as i64 * PROFILE + 0x28);
        let s = unsafe { (*self.w).soldiers };
        put16(s, SOLDIERS_LEN, self.k * REC as i64 + 0x112, salary);
        put16(s, SOLDIERS_LEN, self.k * REC as i64 + 0x110, salary);
        self.sb_put(0x183, pb(0x2A));
        self.sb_put(HP_MAX, self.sb(0x183));
        for (so, po) in COPIES {
            self.sb_put(so, pb(po));
        }
        self.sb_put(0x1DA, 0x40);
        for o in [0x1D9, 0x1B1, 0x17E, 0x185, 0x188, 0x19A] {
            self.sb_put(o, 0);
        }
        put16(s, SOLDIERS_LEN, self.k * REC as i64 + 0x10C, 0);
        self.sb_put(0x18A, 100);
        self.practice_start();
        for i in 0..12usize {
            let e = 12 * i;
            let t = if i < 10 { pb(0x50 + i) } else { 0 };
            self.sb_put(e, t);
            if t == 0 {
                self.sb_put(e + 1, 0);
                self.sb_put(e + 2, 0);
            } else {
                let qty = if self.item_type(t, 6) == 1 { self.item_type(t, 5) } else { 1 };
                self.sb_put(e + 1, qty);
                self.sb_put(e + 2, if t == 0x6F { 20 } else { 100 });
            }
            let item = unsafe { s.add(self.k as usize * REC + e) };
            crate::sector::ja1_item_init(self.w, self.h, item, 0, 0);
        }
        if self.sb(0) == 0 && self.sb(12) != 0 {
            for o in 0..6 {
                self.sb_put(o, self.sb(12 + o));
                self.sb_put(12 + o, 0);
            }
        }
    }
}

/// The entry point: returns the team slot k of the new merc, or −1 (a full team, a profile outside 0..63, missing
/// buffers: nothing changes).
#[no_mangle]
pub extern "C" fn ja1_merc_join(w: *mut Ja1World, h: *const Ja1Host, profile: i32) -> i32 {
    // SPEC GAP: a null world or a null soldier, profile, stat progress or item type buffer does nothing.
    if w.is_null() || !(0..64).contains(&profile) {
        return -1;
    }
    unsafe {
        if (*w).soldiers.is_null() || (*w).profiles.is_null() || (*w).stat_progress.is_null() || (*w).item_types.is_null()
        {
            return -1;
        }
    }
    let s = unsafe { (*w).soldiers };
    let k = (0..8).take_while(|&k| get8(s, SOLDIERS_LEN, k * REC as i64 + IN_CUR_SECTOR as i64) != 0).count() as i64;
    if k >= 8 {
        return -1;
    }
    let Some(a) = Ai::new(w, h, 0) else { return -1 };
    let j = Join { w, h, a, k };
    match std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| j.join(profile))) {
        Ok(()) => k as i32,
        Err(_) => -1,
    }
}
