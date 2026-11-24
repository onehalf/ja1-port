//! The overview picture of view mode 0 (panel.md "View modes", "The overview picture").
use crate::data::{Common, ExeTables, Tileset};
use crate::render::{FxSheets, PITCH};
use crate::sprite::Px;

/// Mini tile groups of the tile bank: ground, object, overlay.
const MINI_GROUND: usize = 669;
const MINI_OBJECT: usize = 826;
const MINI_OVERLAY: usize = 1082;
/// Size of the picture in the back buffer at (32, 28).
pub const OV_W: i32 = 224;
pub const OV_H: i32 = 168;

/// The world values the picture reads.
pub struct OverviewIn<'a> {
    /// 3136 tiles of 8 bytes.
    pub tiles: &'a [u8],
    /// 200 ground item records of 12 bytes.
    pub items: &'a [u8],
    /// 48 soldier records of 512 bytes.
    pub soldiers: &'a [u8],
    /// The effect records (16 x 18 bytes; record 0 is not drawn).
    pub fx: &'a [u8],
    pub sees_slot: u8,
    pub filter: bool,
    pub roofs: bool,
    pub show_all: bool,
}

/// The values of the overview cursor (panel.md "The overview picture" step 4).
#[repr(C)]
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct OvCursor {
    /// The picture value (the overview colour) and the given point (px = 0: the mouse).
    pub p: i32,
    pub px: i32,
    pub py: i32,
    pub mouse_x: i32,
    pub mouse_y: i32,
    /// The window rectangle (edges inside).
    pub left: i32,
    pub top: i32,
    pub right: i32,
    pub bottom: i32,
    /// The overview cost and "over" value of the overview mouse step.
    pub cost: i32,
    pub over: i32,
    pub combat: i32,
    pub tactical_on: i32,
    pub overlay_up: i32,
    /// Step 3: the clock shows (pause.rs, clock switch and blink byte on), with the time on the clock.
    pub clock: i32,
    pub clock_hour: i32,
    pub clock_minute: i32,
}

/// Step 4: the overview cursor (the AP number or sprite 413 + p) into the back buffer.
pub fn cursor(bb: &mut [u8], ts: &Tileset, font0: &crate::panel::Font, table0: &mut [u8; 256], c: &OvCursor) {
    // Step 3: the clock at (32, 28).
    if c.clock != 0 {
        crate::pause::clock(bb, ts, font0, table0, 32, 28, c.clock_hour as i8, c.clock_minute as i8);
    }
    let inside = (c.left..=c.right).contains(&c.mouse_x) && (c.top..=c.bottom).contains(&c.mouse_y);
    if c.px == 0 && !inside {
        return;
    }
    let (x, y) = if c.px != 0 {
        let x = c.px.wrapping_sub(16);
        let x = if x < c.left { c.left } else if x > c.right { c.right } else { x };
        let y = c.py.wrapping_add(14);
        let y = if y < c.top { c.top } else if y > c.bottom { c.bottom } else { y };
        (x, y)
    } else {
        (c.mouse_x.wrapping_sub(16), c.mouse_y.wrapping_add(14))
    };
    if c.cost != 0 && c.combat != 0 {
        let mut s = c.cost.to_string().into_bytes();
        while s.len() < 2 {
            s.insert(0, b'^');
        }
        let fg = c.over as u8;
        crate::cursor::text(bb, font0, table0, &s, x - 5, y - 3, false, fg, fg.wrapping_sub(1), true);
    } else if c.tactical_on != 0 && c.overlay_up == 0 {
        Bb(bb).sprite(ts, 413 + (c.p as u8) as usize, x - 4, y - 4);
    }
}

/// Sheet x, y of the 2 x 2 marks.
fn mark(e: i32) -> Option<(i32, i32)> {
    match e {
        0x6A => Some((162, 138)),
        0x6B => Some((182, 138)),
        0x6C => Some((184, 138)),
        0x6D => Some((186, 138)),
        0x6E => Some((188, 138)),
        0x6F | 0x7A => Some((190, 138)),
        // SPEC GAP: other element numbers in +0x1BD are not described; nothing is drawn.
        _ => None,
    }
}

/// Tile bank sprite `n` at back buffer point (x, y) (DrawCompiledSprite).
pub fn tile_sprite(bb: &mut [u8], ts: &Tileset, n: usize, x: i32, y: i32) {
    Bb(bb).sprite(ts, n, x, y);
}

struct Bb<'a>(&'a mut [u8]);

impl Bb<'_> {
    fn put(&mut self, x: i32, y: i32, v: u8) {
        let off = y as i64 * PITCH as i64 + x as i64;
        if off >= 0 && (off as usize) < self.0.len() {
            self.0[off as usize] = v;
        }
    }
    fn get(&self, x: i32, y: i32) -> u8 {
        let off = y as i64 * PITCH as i64 + x as i64;
        if off >= 0 && (off as usize) < self.0.len() { self.0[off as usize] } else { 0 }
    }
    fn sprite(&mut self, ts: &Tileset, n: usize, x: i32, y: i32) {
        let px: &[Px] = match ts.sprites.get(n) {
            Some(p) => p,
            None => return,
        };
        for p in px {
            self.put(x + p.col, y + p.row, p.val);
        }
    }
    fn elem(&mut self, sheet: &[u8], e: i32, x: i32, y: i32) {
        let Some((sx, sy)) = mark(e) else { return };
        for r in 0..2 {
            for c in 0..2 {
                let v = sheet.get(((sy + r) * 320 + sx + c) as usize).copied().unwrap_or(0);
                if v != 0 {
                    self.put(x + c, y + r, v);
                }
            }
        }
    }
}

/// Overview X of a map pixel x: 32 + x / 4 (toward zero).
fn ov_x(x: i16) -> i32 {
    32 + x as i32 / 4
}

/// Overview Y of a map pixel y: ⌊y × 0.2127659574468085 + 28⌋ toward zero (the factor is a little above 10 / 47).
fn ov_y(y: i16) -> i32 {
    let v = y as i32 * 10 + 28 * 47;
    if v >= 0 { v / 47 } else if v % 47 == 0 { v / 47 + 1 } else { v / 47 }
}

fn s16(b: &[u8], o: usize) -> i16 {
    i16::from_le_bytes([b[o], b[o + 1]])
}

/// Draws the overview picture into the back buffer `bb` (pitch 384); `sheet` = the 320-wide panel sheet.
pub fn draw(bb: &mut [u8], ts: &Tileset, exe: &ExeTables, common: &Common, sheet: &[u8], fxs: &FxSheets, w: &OverviewIn) {
    let mut b = Bb(bb);
    // 1. Tiles, and the marks of the tile two rows up.
    for i in 0..3248usize {
        let (x, y) = (32 + 4 * (i % 56) as i32, 28 + 3 * (i / 56) as i32);
        if i < 3136 {
            let t = &w.tiles[i * 8..i * 8 + 8];
            let (t0, t1, t2, t3, t4) = (t[0], t[1], t[2], t[3], t[4]);
            if w.filter && t4 & 0x80 == 0 {
                b.sprite(ts, MINI_GROUND, x, y);
            } else {
                let o = if exe.gv_t3[t3 as usize] == 0 && w.roofs { 0 } else { t1 };
                if (exe.gv_t1[o as usize] != 0 && exe.gv_t3[t3 as usize] != 0) || t3 == 0x73 || !w.roofs {
                    b.sprite(ts, MINI_GROUND + t0 as usize, x, y);
                }
                if o != 0 {
                    b.sprite(ts, MINI_OBJECT + o as usize, x, y);
                    if common.t1class[o as usize] == 6 {
                        // SPEC GAP: t1 below 0x27 with class 6 has no composite entry; none is drawn.
                        if let Some(&[al, ab, le]) = (o as usize).checked_sub(0x27).and_then(|k| exe.composite.get(k)) {
                            if y - 3 >= 0 {
                                b.sprite(ts, MINI_OVERLAY + ab as usize, x, y - 3);
                            }
                            if x - 4 >= 0 && y - 3 >= 0 {
                                b.sprite(ts, MINI_OVERLAY + al as usize, x - 4, y - 3);
                            }
                            if x - 4 >= 0 {
                                b.sprite(ts, MINI_OVERLAY + le as usize, x - 4, y);
                            }
                        }
                    }
                }
                if t2 != 0 {
                    b.sprite(ts, MINI_OVERLAY + t2 as usize, x, y);
                }
                if t3 != 0 && t3 != 0x73 && (t3 < 0x50 || w.roofs) {
                    b.sprite(ts, MINI_OVERLAY + t3 as usize, x, y);
                }
            }
        }
        // Places 0..111 have no tile two rows up (the original reads memory outside the map there; not visible).
        if i >= 112 {
            let t = &w.tiles[(i - 112) * 8..(i - 112) * 8 + 8];
            let n = t[6] as usize;
            if n < 200 && w.items.get(n * 12 + 6).map_or(false, |&v| (v as i8) > 0) {
                b.elem(sheet, 0x6F, x + 1, y - 5);
            }
            if t[7] >> 4 != 0 {
                b.elem(sheet, 0x7A, x + 1, y - 5);
            }
        }
    }
    // 2. Soldier marks and the effect records.
    let viewer = w.sees_slot as usize;
    for k in 0..48usize {
        let s = &w.soldiers[k * 512..k * 512 + 512];
        let shown = s[0x182] != 0 && s[0x183] != 0 && if k < 40 {
            (s[0x9C] as i8) > 0 && s[0x138] != 0
        } else {
            ((s[0x9C] as i8) > 0 || w.show_all)
                && (viewer >= 8 || w.soldiers.get(viewer * 512 + 0x112 + k) == Some(&1))
        };
        if shown {
            b.elem(sheet, s[0x1BD] as i8 as i32, ov_x(s16(s, 0x116)), ov_y(s16(s, 0x118)));
        }
        if (1..16).contains(&k) {
            let Some(e) = w.fx.get(k * 0x12..k * 0x12 + 0x12) else { continue };
            let (c, kind) = (e[0] as i8 as i32, e[0xB]);
            // Kind 0 (a frame table sprite) is made by nobody in the library; it is not drawn.
            if c == 0 || kind == 0 {
                continue;
            }
            small_blast(&mut b, fxs, c, kind, ov_x(s16(e, 5)), ov_y(s16(e, 7)));
        }
    }
}

/// The small explosion of an effect record: the frame scaled into the block at (0, 0), then keyed to (x, y).
fn small_blast(b: &mut Bb, fxs: &FxSheets, c: i32, kind: u8, x: i32, y: i32) {
    let (src, pitch, col, sw, sh, dw, dh, key): (&[u8], i64, i64, u32, u32, i32, i32, u8) = match kind {
        3 => (&fxs.explode, 1596, (c as i64 - 1) * 76, 76, 60, 19, 15, 0x7B),
        2 => (&fxs.explode, 1596, (c as i64 - 1) * 76, 76, 60, 9, 7, 0x7B),
        _ => (&fxs.smoke, 420, (c as i64 - 1) * 42, 42, 34, 10, 8, 0xD3),
    };
    let (sx, sy) = ((sw << 22) / dw as u32, (sh << 22) / dh as u32);
    for j in 0..dh {
        let row = (sy / 2).wrapping_add((j as u32).wrapping_mul(sy)) >> 22;
        for i in 0..dw {
            let cl = (sx / 2).wrapping_add((i as u32).wrapping_mul(sx)) >> 22;
            // SPEC GAP: a source point outside the picture is not described; it reads 0 here.
            let o = row as i64 * pitch + col + cl as i64;
            let v = if o >= 0 { src.get(o as usize).copied().unwrap_or(0) } else { 0 };
            b.put(i, j, v);
        }
    }
    for j in 0..dh {
        for i in 0..dw {
            let v = b.get(i, j);
            if v != key {
                b.put(x + i, y + j, v);
            }
        }
    }
}

#[cfg(test)]
mod bb_tests {
    use super::*;

    /// Ledger rows ja1front:21325, c2aea50:ja1front:22270: overview pixels outside the buffer are dropped and read 0.
    #[test]
    fn overview_pixels_outside_the_buffer() {
        let mut v = vec![0u8; PITCH as usize * 2];
        let mut b = Bb(&mut v);
        b.put(-1, 0, 7);
        b.put(0, 2, 7);
        b.put(PITCH - 1, 1, 7);
        assert_eq!((b.get(-1, 0), b.get(0, 2), b.get(PITCH - 1, 1)), (0, 0, 7));
        assert_eq!(v.iter().filter(|&&x| x == 7).count(), 1);
    }
}
