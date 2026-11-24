//! Sight line (docs/spec/ai.md "Sightings and the opponent's position": Sight line, Tile opacity; library op 49).
//!
//! A child module of `ai`. Inner calls go through `Ai::call`: direction (op 6) and fire_line (op 42).
use super::damage::get8;
use super::{op, Ai, Ja1Host, Ja1World};

const WINDOW_LEN: usize = 2056;

/// SPEC GAP: "Repeat" ends only at the target, a blocked line or the limit; a limit that 16-bit steps never reach
/// stops after this many steps with result 0.
const MAX_STEPS: u32 = 1 << 20;

fn s16(v: i32) -> i32 {
    v as i16 as i32
}

impl Ai {
    /// Tile opacity of tile t at point (x, y) with steps count n, direction d, target point (tx, ty): (opacity, brush).
    fn tile_opacity(&self, t: i32, x: i32, y: i32, n: i32, d: i32) -> (i32, bool) {
        let o = self.tile_byte(t as u16, 1) as i32;
        if (0x37..=0x82).contains(&o) {
            let v = get8(unsafe { (*self.w).window }, WINDOW_LEN, 8 * o as i64 + d as i64) as i8 as i32;
            return (v, false);
        }
        if (0x27..=0x36).contains(&o) {
            let r = (n / 4).clamp(2, 8);
            let (tx0, ty0) = ((t % 56) * 16 + 5, (t / 56) * 14 + 5);
            let near = (x - (tx0 + 2)).abs().max((y - (ty0 + 4)).abs()) < r;
            return (if near { ((n / 15) * 2) & 0xFF } else { 0 }, true);
        }
        if o == 0xE2 || o == 0xE3 {
            return (n / 30, false);
        }
        (0, false)
    }

    /// Sight line from this soldier S (ai.md Sight line): 1 = seen, 0 = not seen.
    pub(super) fn sight_line(&self, args: &[i32]) -> i32 {
        let g = |k: usize| args.get(k).copied().unwrap_or(0);
        let me = self.slot();
        let (mut x, mut y, tile, tx, ty) = (s16(g(1)), s16(g(2)), s16(g(3)), s16(g(4)), s16(g(5)));
        let flag = g(6) as u8;
        let mut range = g(7);
        let seen = g(8) as u8;
        // 1.
        if seen < 0x40
            && flag == 0
            && self.rd8_of(seen as usize, 0x1B1) != 0
            && matches!(self.rd8_of(seen as usize, 0x1B3), 1 | 2 | 4 | 5 | 6 | 7)
        {
            range = range.wrapping_sub(range.wrapping_mul(30) / 100);
        }
        let mut lim = s16(range.wrapping_mul(15));
        // 2.
        let d = (self.call(op::DIRECTION, &[x, y, tx, ty]) & 0xFF) as i32;
        // 3.
        let (mut dx, mut dy) = (s16(tx - x), s16(ty - y));
        let sx = if dx < 0 { -1 } else { 1 };
        let sy = if dy < 0 { -1 } else { 1 };
        dx = s16(dx.abs());
        dy = s16(dy.abs());
        let (major, minor, stx, sty) = if dx < dy { (dy, dx, 0, sy) } else { (dx, dy, sx, 0) };
        let mut err = s16(2 * minor - major);
        let diag = s16(err - major);
        // 4.
        let vt = |x: i32, y: i32| x / 16 + (y / 14) * 56;
        let mut prev = vt(x, y);
        let (mut steps, mut sum, mut count, mut opacity, mut brush) = (0i32, 0i32, 0i32, 0i32, false);
        // 5.
        for _ in 0..MAX_STEPS {
            // a.
            if err < 0 {
                x = s16(x + stx);
                y = s16(y + sty);
                err = s16(err + 2 * minor);
            } else {
                x = s16(x + sx);
                y = s16(y + sy);
                err = s16(err + diag);
            }
            let cur = vt(x, y);
            steps = s16(steps + 1);
            if steps > lim {
                return 0;
            }
            // b.
            if cur != prev {
                sum = 0;
                count = 0;
                let (o, b) = self.tile_opacity(cur, x, y, steps, d);
                opacity = o;
                brush = b;
                if opacity >= 0x62 {
                    if steps > 15 {
                        lim = s16(if opacity == 0x62 { steps + 30 } else { steps });
                    } else if opacity == 0x63 {
                        lim = s16(steps + 30);
                    }
                    opacity = 3;
                }
                if brush {
                    count += 1;
                }
            }
            // c.
            if opacity != 0 || brush {
                if brush && count >= 0 {
                    count = s16(count + 1);
                    if count > 4 {
                        lim = s16(lim - 15).max(0);
                        count = -1;
                    }
                }
                sum = s16(sum + if cur == prev && brush { self.tile_opacity(cur, x, y, steps, d).0 } else { opacity });
                // ai.md Sight line step 5c: a sum above 56 ends the walk (F = 0: 0, else by fire_line); else step d.
                if sum > 56 {
                    if flag == 0 {
                        return 0;
                    }
                    return if (self.call(op::FIRE_LINE, &[me, tile, 1]) as i32) >= 25 { 1 } else { 0 };
                }
            }
            // d.
            if x == tx && y == ty {
                return 1;
            }
            prev = cur;
        }
        0
    }
}

// ---- Sight line (ai.md Sight line); library op 49 ----
// SPEC GAP: on a null world / host or a slot outside 0..47 the result is 0.
#[no_mangle]
pub extern "C" fn ja1_sight_line(
    w: *mut Ja1World,
    h: *const Ja1Host,
    slot: i32,
    x: i32,
    y: i32,
    tile: i32,
    tx: i32,
    ty: i32,
    flag: i32,
    range: i32,
    seen: i32,
) -> i32 {
    super::run(w, h, slot, 0, |a| a.sight_line(&[slot, x, y, tile, tx, ty, flag, range, seen]))
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn s16_wraps() {
        assert_eq!(s16(32768), -32768);
        assert_eq!(s16(-1), -1);
    }
}
