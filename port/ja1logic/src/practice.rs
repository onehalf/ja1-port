//! Stat practice (docs/spec/combat.md "Stat practice", exact steps; C interface in docs/spec/port_api.md):
//! `ja1_stat_practice`, the work of host op 105 stat_practice. A child module of `ai`; it reads the host's Random.
use super::damage::{get16, get8, put16};
use super::{Ja1Host, Ja1World, REC};

const SOLDIERS_LEN: usize = 48 * REC;
const PROGRESS_LEN: usize = 8 * 0x12;
// soldier record offsets (defs/types.toml "soldier")
const SLOT: i64 = 0x17A;
const WISDOM: i64 = 0x18D;
const MODE: i64 = 0x19B;
const STATE: i64 = 0x19C;
/// The stat bytes of stats 1..9.
const FIELDS: [i64; 9] = [0x184, 0x18B, 0x18C, 0x18D, 0x18E, 0x18F, 0x190, 0x191, 0x192];

fn random(h: *const Ja1Host, n: i32) -> i32 {
    unsafe {
        match (*h).random {
            Some(r) => r((*h).ctx, n),
            None => 0,
        }
    }
}

/// C division, cut toward zero; a zero divisor gives 0 (the spec's level 0 case).
fn cdiv(a: i32, b: i32) -> i32 {
    if b == 0 {
        0
    } else {
        a.wrapping_div(b)
    }
}

fn stat_practice(w: *mut Ja1World, h: *const Ja1Host, slot: i32, stat: u8, n: i16, flag: u8) {
    let sol = unsafe { (*w).soldiers };
    let prog = unsafe { (*w).stat_progress };
    let base = slot as i64 * REC as i64;
    let sb = |o: i64| get8(sol, SOLDIERS_LEN, base + o);
    // 1.
    let row = sb(SLOT);
    let mode = sb(MODE);
    if row > 7 || mode == 0 || (flag != 0 && mode == 2) {
        return;
    }
    // 2.
    let mut n = n;
    if flag == 0 && stat != 1 {
        n = n.wrapping_mul(3);
    }
    if !(1..=9).contains(&stat) {
        return;
    }
    // 3.
    let at = |k: i64| row as i64 * 0x12 + 2 * k;
    let v = sb(FIELDS[stat as usize - 1]) as i32;
    let p = get16(prog, PROGRESS_LEN, at(stat as i64 - 1)) as i16 as i32;
    let wp = get16(prog, PROGRESS_LEN, at(3)) as i16 as i32;
    let b = cdiv(wp, 100) + sb(WISDOM) as i8 as i32 - 50;
    let mut delta: i32 = 0;
    // 4.
    for _ in 0..n.max(0) {
        if mode == 1 {
            let c = if stat == 9 { 100 - (cdiv(p, v * 1000) + v) * 10 } else { 100 - (cdiv(p, 100) + v) };
            let mut c = c as i16 as i32;
            if c > 0 {
                c = c.wrapping_add(cdiv(b.wrapping_mul(c), 100)) as i16 as i32;
            }
            if stat == 8 && sb(STATE) == 6 {
                c = cdiv(c, 5);
            }
            if c > 99 {
                c = 99;
            }
            if random(h, 100) < c {
                delta += 1;
            }
        } else {
            let c = match stat {
                9 => (cdiv(p, v * 1000) + v - 1) * 10,
                1..=4 => cdiv(p, 100) + v - 1,
                _ => cdiv(p, 100) + v,
            };
            let mut c = c as i16 as i32;
            if c > 0 {
                c = c.wrapping_sub(cdiv(c.wrapping_mul(b), 100)) as i16 as i32;
                if c < 1 {
                    c = 1;
                }
            }
            if random(h, 100) < c {
                delta -= 1;
            }
        }
    }
    // 5.
    let k = at(stat as i64 - 1);
    put16(prog, PROGRESS_LEN, k, (get16(prog, PROGRESS_LEN, k) as i32).wrapping_add(delta) as u16);
    if stat != 9 && flag == 0 {
        let l = at(8);
        put16(prog, PROGRESS_LEN, l, (get16(prog, PROGRESS_LEN, l) as i32).wrapping_add(delta) as u16);
    }
}

// SPEC GAP: a null world or host, or a slot outside 0..47, does nothing.
#[no_mangle]
pub extern "C" fn ja1_stat_practice(w: *mut Ja1World, h: *const Ja1Host, slot: i32, stat: i32, n: i32, flag: i32) {
    if w.is_null() || h.is_null() || !(0..48).contains(&slot) {
        return;
    }
    let _ = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        stat_practice(w, h, slot, stat as u8, n as i16, flag as u8)
    }));
}
