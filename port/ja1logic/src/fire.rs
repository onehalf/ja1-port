//! Slice 2c part 3: Line of fire (docs/spec/ai.md "Line of fire", "Exact procedure"; library op 42 fire_line).
//!
//! A child module of `ai`. Inner calls are host calls: direction (6), hit_test (48), sight_line (49), div_zero
//! (47) and the host random.
use super::search::{px, py, s16, step};
use super::{f, op, Ai};

const ITEM_B1B3: usize = 0x1B3;

/// vt(x, y) = cdiv(x, 16) + cdiv(y, 14) × 56: the tile under a pixel.
pub(super) fn vt(x: i32, y: i32) -> i32 {
    x / 16 + (y / 14) * 56
}

/// SPEC GAP: "Repeat" has no end other than reaching T or leaving the map; a path that never does (not possible
/// for a T on the map) stops after this many pixels with result 0.
const MAX_PIXELS: i32 = 1 << 20;

impl Ai {
    /// Line of fire of this soldier to tile t (s16) with the thrown flag: the chance in %.
    pub(super) fn line_of_fire(&self, t: i32, thrown: u8) -> i32 {
        let me = self.slot();
        let t = s16(t);
        // 1.
        let my = s16(self.rd16(f::TILE) as i32);
        if my == t {
            return 100;
        }
        // 2.
        let (tx, ty) = (px(t) + 2, py(t) + 4);
        let mut x = s16(self.rd16(f::X2) as i32) + 2;
        let mut y = s16(self.rd16(f::Y2) as i32) + 4;
        // 3.
        let d = (self.call(op::DIRECTION, &[x, y, tx, ty]) as u8) as i32;
        // 4.
        let dist = (x - tx).abs().max((y - ty).abs());
        // 5.
        let mut skip: u8 = 0;
        if self.call(op::SIGHT_LINE, &[me, x, y, t, tx, ty, 0, 0x0E, 0x40]) != 0 {
            skip = dist as u8;
        }
        // 6.
        let item = if thrown != 0 { 1 } else { self.rd8(f::ITEMS) };
        let throw = self.item_kind(item) == super::KIND_THROW;
        // 7.
        let (cap, down, mut h, mut slope);
        if throw {
            cap = if self.rd8(ITEM_B1B3) == 3 { 9000 } else { 22000 };
            h = 8000;
            let half = dist / 2;
            slope = (self.cdiv((cap - 8000) * 2, half) + 1) / 2;
            down = (self.cdiv(-cap * 2, half) - 1) / 2;
        } else {
            cap = 0;
            down = 0;
            h = self.random(1001) + 4500;
            let e = 4000 - h;
            slope = (self.cdiv(e * 2, dist) + if e >= 0 { 1 } else { -1 }) / 2;
        }
        // 8.
        let (mut ax, mut ay) = (tx - x, ty - y);
        let sx = if ax < 0 { -1 } else { 1 };
        let sy = if ay < 0 { -1 } else { 1 };
        ax = ax.abs();
        ay = ay.abs();
        let (major, minor, mx, myy) = if ax >= ay { (ax, ay, sx, 0) } else { (ay, ax, 0, sy) };
        let mut err = 2 * minor - major;
        // 9.
        let mut prev = vt(x, y);
        let mut flown = 0;
        let mut pct = 100;
        // 10.
        while flown < MAX_PIXELS {
            // a.
            if err < 0 {
                x += mx;
                y += myy;
                err += 2 * minor;
            } else {
                x += sx;
                y += sy;
                err += 2 * minor - 2 * major;
            }
            // b.
            let tt = vt(x, y);
            flown += 1;
            h = (h + slope).max(0);
            if cap != 0 && h > cap {
                h = cap;
                slope = down;
            }
            // c.
            if tt != prev && tt != my {
                if step(prev, tt.wrapping_sub(prev)) != tt {
                    return 0;
                }
                prev = tt;
            }
            // d.
            if tt != my {
                // The out record starts as skip and chance 100 (unchanged unless hit_test writes them).
                let mut out = [skip, 0, 0, 0, 100, 0, 0, 0];
                let res = self.call_out(op::HIT_TEST, &[h, x, y, flown, d, skip as i32, cap, t, 1], &mut out);
                skip = out[0];
                let chance = i32::from_le_bytes([out[4], out[5], out[6], out[7]]);
                if res != 0 {
                    return if tt == t { pct } else { 0 };
                }
                pct = pct.wrapping_mul(chance) / 100;
            }
            // e.
            if tt == t {
                return pct;
            }
        }
        0
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn tile_under_pixel() {
        assert_eq!(vt(0, 0), 0);
        assert_eq!(vt(16, 14), 57);
        assert_eq!(vt(-1, 0), 0);
    }
}
