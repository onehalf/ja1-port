//! Soldiers (tactical_map.md "Soldiers"): SH.DAT bank and the 12-step draw of one soldier record.
use crate::iso::Iso;
use crate::render::{HEIGHT, PITCH};
use std::io;

pub const SH_COUNT: usize = 1456;
pub const REC: usize = 512;

/// DAT/SH.DAT: u32 count, then the offsets relative to the end of the offset table.
pub struct SoldierBank {
    pub bank: Vec<u8>,
    /// Absolute byte offset of every bitmap.
    pub offsets: Vec<usize>,
}

impl SoldierBank {
    pub fn load(iso: &mut Iso) -> io::Result<SoldierBank> {
        let file = iso.read("DAT/SH.DAT")?;
        Self::from_file(&file, crate::offsetbank::SH_MAX)
    }
    pub fn from_bytes(file: Vec<u8>) -> io::Result<SoldierBank> {
        let max = file.len() as i32;
        Self::from_file(&file, max)
    }
    fn from_file(file: &[u8], max: i32) -> io::Result<SoldierBank> {
        let bad = |m: &str| io::Error::new(io::ErrorKind::InvalidData, m.to_string());
        let b = crate::offsetbank::load(Some(file), max).map_err(|_| bad("SH.DAT does not load"))?;
        if b.entries.len() < SH_COUNT {
            return Err(bad("SH.DAT has too few entries"));
        }
        Ok(SoldierBank { bank: b.image, offsets: b.entries })
    }
}

/// Game values and tables for drawing soldiers.
pub struct SoldierEnv<'a> {
    pub bank: &'a SoldierBank,
    /// Remap tables 0..33 of JAGGED.DAT.
    pub remap: &'a [[u8; 256]],
    pub seen: &'a [u8],
    pub show_all: bool,
    pub viewer: u8,
    pub selected: u8,
    pub shade: u8,
}

fn u16_at(r: &[u8], o: usize) -> u16 {
    u16::from_le_bytes([r[o], r[o + 1]])
}

/// Draw one soldier. `rec` is its 512-byte record (anchor fields are written), `rest` the bytes that follow it in
/// memory (the anim read may run past the record). `buf` is the 384 x 854 back buffer; the view top-left tile is
/// (c0, r0), the window is cols x rows tiles. Returns the drawn place (x, y after step 8, the width) when step 12
/// was reached (tactical_map.md "Soldier texts" use it).
#[allow(clippy::too_many_arguments)]
/// The table 0x381C0 fills at 0xA5588 (8 rows of 182 numbers) as the original's DrawSoldier? 0x165E8 reads it: row f,
/// frame `frame` of the bank SH.DAT.
pub fn bitmap_number(f: u16, frame: u16) -> u16 {
    f.wrapping_mul(182).wrapping_add(frame)
}

pub fn draw_soldier(
    buf: &mut [u8],
    env: &SoldierEnv,
    rec: &mut [u8],
    rest: &[u8],
    script_word: u16,
    c0: i32,
    r0: i32,
    cols: i32,
    rows: i32,
) -> Option<(i32, i32, i32)> {
    // 1.
    if rec[0x188] == 1 {
        return None;
    }
    // 2.
    let f = (rec[0x1A1] as i8 as i16).wrapping_sub(1) as u16;
    let mut frame = script_word;
    // 3.
    if (rec[0x9C] as i8) <= 0 && !env.show_all {
        return None;
    }
    // 4.
    let slot = rec[0x17A];
    if slot >= 0x28 && env.viewer < 0x40 {
        // SPEC GAP: a sight table byte outside the given table counts as not seen.
        let s = env.seen.get(env.viewer as usize * 512 + slot as usize).copied().unwrap_or(0);
        if s != 1 {
            return None;
        }
    }
    // 5.
    let over = u16_at(rec, 0x136);
    if over != 0 {
        frame = over;
    }
    // 6.
    let (w, h, ax, ay): (i16, i16, u16, u16) = if frame >= 0x43 {
        if frame >= 0xB6 {
            return None;
        }
        (32, 28, 15, 16)
    } else {
        (20, 20, 9, 12)
    };
    rec[0x11A..0x11C].copy_from_slice(&ax.to_le_bytes());
    rec[0x11C..0x11E].copy_from_slice(&ay.to_le_bytes());
    // 7.
    let mut x: i32 = (u16_at(rec, 0x116) as i16 as i32) - ax as i32 - 16 * c0 + 32;
    let mut y: i32 = (u16_at(rec, 0x118) as i16 as i32) - ay as i32 - 14 * r0 + 28;
    // 8. SPEC GAP: anim is read as a u16 at 0x1E8 + 2 * animIdx; bytes past the record come from the next record,
    // and 0 past the end of all records.
    let ao = 2 * rec[0x1F3] as usize;
    let at = |i: usize| -> u8 {
        let p = 0x1E8 + ao + i;
        if p < REC {
            rec[p]
        } else {
            rest.get(p - REC).copied().unwrap_or(0)
        }
    };
    let code = u16::from_le_bytes([at(0), at(1)]);
    if matches!(code, 1..=3 | 0xD | 0x2B) && frame > 0x30 {
        x += 1;
        y += 4;
    }
    // 9.
    let (xi, yi) = (x, y);
    if xi < 32 - w as i32 || xi > 16 * cols + 32 || yi < 28 - h as i32 || yi > 14 * rows + 28 {
        return None;
    }
    // 10. SPEC GAP: only remap tables 0..33 exist; a table index outside that (including a negative signed byte)
    // draws nothing.
    let k: u8 = if rec[0x1AD] != 0 {
        0x1B
    } else if slot < 8 && slot == env.selected {
        rec[0x17B].wrapping_add(8)
    } else {
        rec[0x17B]
    };
    let k = k as i8;
    // 11.
    if frame == 0x48 || frame == 0x50 {
        frame = frame.wrapping_add(u16_at(rec, 0x10A));
    }
    // 12. SPEC GAP: the bitmap number f * 182 + frame is computed in 16 bits; a number past the bank is not drawn.
    let n = bitmap_number(f, frame) as usize;
    let Some(&src) = env.bank.offsets.get(n) else { return Some((xi, yi, w as i32)) };
    let ktab = if k >= 0 { env.remap.get(k as usize) } else { None };
    // SPEC GAP: a shade table number outside 0..33 leaves darkened pixels unchanged.
    let stab = env.remap.get(env.shade as usize);
    for row in 0..h as i64 {
        for col in 0..w as i64 {
            // SPEC GAP: a bitmap running past the end of SH.DAT: the missing pixels are not drawn.
            // The tables do the original RemapBlit 0x52425's job: skip 0, map key through tblB, else tblA.
            let Some(&v) = env.bank.bank.get(src + (row * w as i64 + col) as usize) else { continue };
            if v == 0x18 {
                continue;
            }
            let off = (yi as i64 + row) * PITCH as i64 + xi as i64 + col;
            if off < 0 || off >= (PITCH * HEIGHT) as i64 {
                continue;
            }
            let d = buf[off as usize];
            if v == 0x1C {
                if let Some(t) = stab {
                    buf[off as usize] = t[d as usize];
                }
            } else if let Some(t) = ktab {
                buf[off as usize] = t[v as usize];
            }
        }
    }
    Some((xi, yi, w as i32))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn draws_small_frame_and_writes_anchor() {
        // Bank: 1456 entries, entry n at offset n * 400 (20 x 20 bitmaps); bitmap 0 = 0x05 except two special pixels.
        let n = SH_COUNT;
        let mut b = Vec::new();
        b.extend_from_slice(&(n as u32).to_le_bytes());
        for i in 0..n {
            b.extend_from_slice(&((i * 400) as u32).to_le_bytes());
        }
        let mut data = vec![0x05u8; n * 400];
        data[1] = 0x18; // pixel (1, 0): leave
        data[2] = 0x1C; // pixel (2, 0): darken
        b.extend_from_slice(&data);
        let bank = SoldierBank::from_bytes(b).unwrap();
        let mut remap = vec![[0u8; 256]; 34];
        for (k, t) in remap.iter_mut().enumerate() {
            for v in 0..256 {
                t[v] = (v as u8).wrapping_add(k as u8);
            }
        }
        let env = SoldierEnv { bank: &bank, remap: &remap, seen: &[], show_all: true, viewer: 0xFF, selected: 0xFF, shade: 2 };
        let mut rec = vec![0u8; REC];
        rec[0x1A1] = 1; // facing 1 -> bitmap 0 + frame
        rec[0x17B] = 3; // colour
        rec[0x116..0x118].copy_from_slice(&100u16.to_le_bytes()); // x2
        rec[0x118..0x11A].copy_from_slice(&80u16.to_le_bytes()); // y2
        let mut buf = vec![7u8; (PITCH * HEIGHT) as usize];
        draw_soldier(&mut buf, &env, &mut rec, &[], 0, 0, 0, 14, 12);
        assert_eq!(u16_at(&rec, 0x11A), 9);
        assert_eq!(u16_at(&rec, 0x11C), 12);
        let (x, y) = (100 - 9 + 32, 80 - 12 + 28);
        let at = |c: i32, r: i32| buf[((y + r) * PITCH + x + c) as usize];
        assert_eq!(at(0, 0), 0x05 + 3);
        assert_eq!(at(1, 0), 7);
        assert_eq!(at(2, 0), 7 + 2);
        // dead soldier: nothing more drawn
        rec[0x188] = 1;
        let mut buf2 = vec![7u8; (PITCH * HEIGHT) as usize];
        draw_soldier(&mut buf2, &env, &mut rec, &[], 0, 0, 0, 14, 12);
        assert!(buf2.iter().all(|&v| v == 7));
        // step 9: a soldier more than one frame height above the top edge (yi = 6 < 28 - 20) is not drawn
        rec[0x188] = 0;
        rec[0x118..0x11A].copy_from_slice(&(-10i16 as u16).to_le_bytes());
        let mut buf3 = vec![7u8; (PITCH * HEIGHT) as usize];
        draw_soldier(&mut buf3, &env, &mut rec, &[], 0, 0, 0, 14, 12);
        assert!(buf3.iter().all(|&v| v == 7));
    }

    fn bank_of(n: usize) -> Vec<u8> {
        let mut b = Vec::new();
        b.extend_from_slice(&(n as u32).to_le_bytes());
        for i in 0..n {
            b.extend_from_slice(&((i * 4) as u32).to_le_bytes());
        }
        b.extend_from_slice(&vec![0u8; n * 4]);
        b
    }

    /// SH.DAT needs at least SH_COUNT entries; more are fine.
    #[test]
    fn bank_entry_count() {
        assert!(SoldierBank::from_bytes(bank_of(SH_COUNT - 1)).is_err());
        assert!(SoldierBank::from_bytes(bank_of(SH_COUNT)).is_ok());
        assert!(SoldierBank::from_bytes(bank_of(SH_COUNT + 1)).is_ok());
    }
}
