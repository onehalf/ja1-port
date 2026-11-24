//! The picture bank loader: LoadOffsetBank 0x52E00 and the tile bank name + size rule of OpenTilesDat 0x36EA0.
//! File: u32 n, n u32 offsets relative to the end of the table, data. The original reads exactly `max` bytes into its
//! buffer, moves them down over the count and turns the offsets into pointers. Here `image` is the buffer after the
//! move and `entries` are the pointers as indexes into `image`.
//! Not copied: the file picked (iso::game_file has the order), the buffer of a failed load (no caller here reads it),
//! a file under 4 bytes or with a count past its end (the original reads old buffer bytes or writes past the buffer;
//! both give -1 here, no game file does this), the last 4 buffer bytes (the original moves in the 4 bytes after its buffer).

/// LoadOffsetBank: the file ended before `max` bytes, or it did not open.
pub const E_READ: i32 = -1;
/// LoadOffsetBank: the file is longer than `max`.
pub const E_BIG: i32 = -2;
/// The `max` of the three fixed banks (0x2AD8A, 0x2ADA8, 0x381AA) = their exact file sizes.
pub const SH_MAX: i32 = 0xFF044;
pub const SINV_MAX: i32 = 0x1E98;
pub const INV_MAX: i32 = 0xD73C;
/// OpenTilesDat: a larger tile bank fails.
pub const TILES_LIMIT: usize = 0x40000;

pub struct OffsetBank {
    pub image: Vec<u8>,
    pub entries: Vec<usize>,
}

/// LoadOffsetBank 0x52E00 on the file's bytes (None = no file). A file shorter than `max` fails: the read asks for
/// exactly `max` bytes. A count of 0 or below (signed) keeps the table bytes as they are and gives no entries.
pub fn load(file: Option<&[u8]>, max: i32) -> Result<OffsetBank, i32> {
    let Some(f) = file else { return Err(E_READ) };
    if f.len() as i64 > max as i64 {
        return Err(E_BIG);
    }
    if (f.len() as i64) < max as i64 || f.len() < 4 {
        return Err(E_READ);
    }
    let image = f[4..].to_vec();
    let n = i32::from_le_bytes([f[0], f[1], f[2], f[3]]);
    let mut entries = Vec::new();
    if n > 0 {
        let table = (n as u32).wrapping_mul(4);
        if table as usize > image.len() {
            return Err(E_READ);
        }
        entries.reserve(n as usize);
        for t in image[..table as usize].chunks_exact(4) {
            entries.push(u32::from_le_bytes([t[0], t[1], t[2], t[3]]).wrapping_add(table) as usize);
        }
    }
    Ok(OffsetBank { image, entries })
}

/// OpenTilesDat 0x36EA0: only the last decimal digit of the tile set goes into the name.
pub fn tiles_name(tileset: u8) -> String {
    format!("DAT/TILES{}.DAT", tileset % 10)
}

/// OpenTilesDat 0x36EA0: the size of the file itself is the `max`; above 256 KB or missing (size -1, unsigned) it fails.
/// The caller sees only -1, also for a failed load.
pub fn open_tiles(file: Option<&[u8]>) -> Result<OffsetBank, i32> {
    match file {
        Some(f) if f.len() <= TILES_LIMIT => load(file, f.len() as i32).map_err(|_| E_READ),
        _ => Err(E_READ),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn exact_size_only() {
        let f = [1u8, 0, 0, 0, 2, 0, 0, 0, 9, 8];
        let b = load(Some(&f), 10).unwrap();
        assert_eq!(b.entries, [6]);
        assert_eq!(b.image, [2, 0, 0, 0, 9, 8]);
        assert_eq!(load(Some(&f), 11).err(), Some(E_READ));
        assert_eq!(load(Some(&f), 9).err(), Some(E_BIG));
        assert_eq!(load(None, 9).err(), Some(E_READ));
        assert_eq!(tiles_name(13), "DAT/TILES3.DAT");
    }

    /// Ledger rows ja1front:21664 and c2aea50:ja1front:22592 (port rules): a table that fills the image exactly loads,
    /// one entry more fails; a 4-byte file is a count with an empty image, a file of under 4 bytes fails.
    #[test]
    fn table_and_count_bounds() {
        let f = [1u8, 0, 0, 0, 0, 0, 0, 0];
        assert_eq!(load(Some(&f), 8).unwrap().entries, [4]);
        let f = [2u8, 0, 0, 0, 0, 0, 0, 0];
        assert_eq!(load(Some(&f), 8).err(), Some(E_READ));
        let b = load(Some(&[0u8, 0, 0, 0]), 4).unwrap();
        assert!(b.entries.is_empty() && b.image.is_empty());
        assert_eq!(load(Some(&[1u8, 0, 0]), 3).err(), Some(E_READ));
    }
}
