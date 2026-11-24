//! Loading the tactical data from the CD image and JA.EXE (tactical_map.md "Files", "Where the files are").
use crate::iso::Iso;
use crate::sprite::{decode, Px};
use std::io;

pub const RECORD: usize = 15681;
pub const SECTORS: usize = 60;
pub const TILES: usize = 3136;
pub const BANK_COUNT: usize = 1327;
pub const COMPILED: usize = 1293; // entries 0..1292
pub const RAW_FIRST: usize = 1293; // entries 1293..1326: 16 x 14 bitmaps
pub const SHADOW_IDS: usize = 99;
const REMAP_AT: usize = 13424; // 0x3470
const REMAP_COUNT: usize = 34;

fn bad(msg: String) -> io::Error {
    io::Error::new(io::ErrorKind::InvalidData, msg)
}

/// Data shared by all tilesets of one CD image.
pub struct Common {
    pub terrain: Vec<u8>,
    /// The 34 colour remap tables as in JAGGED.DAT, table 29 with its fixed overrides.
    pub remap: Vec<[u8; 256]>,
    pub t1class: [u8; 256],
    pub t2class: [u8; 256],
}

impl Common {
    pub fn load(iso: &mut Iso) -> io::Result<Common> {
        let terrain = iso.read("DAT/TERRAIN.DAT")?;
        let jag = iso.read("DAT/JAGGED.DAT")?;
        Common::from_files(terrain, &jag)
    }

    /// `load` on the bytes of TERRAIN.DAT and JAGGED.DAT.
    pub(crate) fn from_files(terrain: Vec<u8>, jag: &[u8]) -> io::Result<Common> {
        if terrain.len() < RECORD * SECTORS {
            return Err(bad(format!("TERRAIN.DAT is {} bytes", terrain.len())));
        }
        let need = REMAP_AT + REMAP_COUNT * 256 + 157 + 256 + 155;
        if jag.len() < need {
            return Err(bad(format!("JAGGED.DAT is {} bytes", jag.len())));
        }
        let mut remap = Vec::with_capacity(REMAP_COUNT);
        for i in 0..REMAP_COUNT {
            let mut t = [0u8; 256];
            t.copy_from_slice(&jag[REMAP_AT + i * 256..REMAP_AT + (i + 1) * 256]);
            remap.push(t);
        }
        // "The overrides of remap table 29 apply after loading, whatever the tileset."
        remap[29][0x27] = 0x2B;
        remap[29][0xF1] = 0xF2;
        remap[29][0x1B] = 0xC6;
        let mut p = REMAP_AT + REMAP_COUNT * 256 + 157; // skip the t0 class table (not used for drawing)
        let mut t1class = [0u8; 256];
        t1class.copy_from_slice(&jag[p..p + 256]);
        p += 256;
        let mut t2class = [0u8; 256]; // values 155..255 = 0
        t2class[..155].copy_from_slice(&jag[p..p + 155]);
        Ok(Common { terrain, remap, t1class, t2class })
    }

    /// Bytes t0..t4 of tile (col, row) of `sector` (1..60). Records hold the tiles from the last to the first.
    pub fn tile(&self, sector: usize, col: usize, row: usize) -> [u8; 5] {
        let k = row * 56 + col;
        let at = (sector - 1) * RECORD + (TILES - 1 - k) * 5;
        let mut t = [0u8; 5];
        t.copy_from_slice(&self.terrain[at..at + 5]);
        t
    }

    pub fn tileset(&self, sector: usize) -> u8 {
        self.terrain[(sector - 1) * RECORD + TILES * 5]
    }

    /// LoadTerrainTileset 0x2A9FC step 1: the 3136 x 8 tile map of `sector` (1..60), from its record.
    pub fn fill_map(&self, sector: usize, out: &mut [u8]) {
        fill_map_record(&self.terrain[(sector - 1) * RECORD..sector * RECORD], out)
    }
}

/// One DAT/TERRAIN.DAT sector record (15,680 tile bytes, tiles from the last to the first, 5 each) into the
/// 3136 x 8 map: t0..t4, then byte 5 = 0x40, byte 6 = 0xFF, byte 7 = 0.
pub fn fill_map_record(record: &[u8], out: &mut [u8]) {
    for k in 0..TILES {
        let at = (TILES - 1 - k) * 5;
        out[k * 8..k * 8 + 5].copy_from_slice(&record[at..at + 5]);
        out[k * 8 + 5] = 0x40;
        out[k * 8 + 6] = 0xFF;
        out[k * 8 + 7] = 0;
    }
}

/// The per-value tables read from JA.EXE v1.13 at fixed file offsets.
pub struct ExeTables {
    pub gv_t1: [u8; 256],
    pub gv_t3: [u8; 256],
    /// Per t1 from 0x27: [above-left, above, left] overlay numbers.
    pub composite: Vec<[u8; 3]>,
    /// Per shadow id: (raw bitmap number, dx, dy).
    pub shadow: Vec<(u8, i8, i8)>,
    /// cursor.md "Values": the cursor table (182 pairs).
    pub cursor: Vec<u8>,
    /// tactical_map.md "Ground items": byte 4 of each of the 136 item type entries (the SINV.DAT sprite).
    pub item_sprite: Vec<u8>,
}

impl ExeTables {
    pub fn load(path: &str) -> io::Result<ExeTables> {
        let exe = std::fs::read(path)?;
        // SPEC GAP: the 2 x 2 composite table's length is not given; read t1 = 0x27..0xFF (217 entries).
        let comp_n = 256 - 0x27;
        if exe.len() < 0xDCF08 + SHADOW_IDS * 3 || exe.len() < 0xDAE04 + comp_n * 3 {
            return Err(bad(format!("JA.EXE is {} bytes", exe.len())));
        }
        let mut gv_t1 = [0u8; 256];
        gv_t1.copy_from_slice(&exe[0xDABF0..0xDABF0 + 256]);
        let mut gv_t3 = [0u8; 256];
        gv_t3.copy_from_slice(&exe[0xDACF0..0xDACF0 + 256]);
        let composite = (0..comp_n)
            .map(|i| {
                let a = 0xDAE04 + i * 3;
                [exe[a], exe[a + 1], exe[a + 2]]
            })
            .collect();
        let shadow = (0..SHADOW_IDS)
            .map(|i| {
                let a = 0xDCF08 + i * 3;
                (exe[a], exe[a + 1] as i8, exe[a + 2] as i8)
            })
            .collect();
        let ct = crate::cursor::TABLE_FILE_OFF;
        let cursor = exe.get(ct..ct + crate::cursor::TABLE_LEN).ok_or_else(|| bad("JA.EXE too short".into()))?.to_vec();
        let it = crate::invdraw::ITEM_TYPES_AT;
        let item_sprite = (0..136).map(|t| exe.get(it + 12 * t + 4).copied().unwrap_or(0)).collect();
        Ok(ExeTables { gv_t1, gv_t3, composite, shadow, cursor, item_sprite })
    }
}

/// 6-bit VGA value to 8 bits: v << 2 | v >> 4.
pub fn vga8(v: u8) -> u8 {
    let v = (v & 63) as u32;
    (v * 4 + v / 16).min(255) as u8
}

/// Remap table 28 for a tileset from its LST file (tactical_map.md MISC\TILESET*n*.LST).
pub fn table28_from_lst(base: &[u8; 256], lst: &[u8], tileset: u8) -> [u8; 256] {
    let mut t = *base;
    for i in 0..256 {
        let Some(&v) = lst.get(i) else { break };
        if v == 255 {
            continue; // keeps the table's own byte, no overrides
        }
        // Overrides chosen by the entry number i (position in the file), in the spec's order.
        let mut w = v;
        if i == 0xA2 || i == 0xA3 {
            w = 0xA3;
        }
        if tileset == 7 && i == 0xB9 {
            w = 0xBD;
        }
        if tileset == 9 {
            if i == 0xF9 {
                w = 0xFA;
            }
            if i == 0xB9 {
                w = 0xBD;
            }
        }
        if i == 0x2E {
            w = 0x2F;
        }
        if i == 0xF5 || i == 0xF7 || i == 0xF9 {
            w = 0xF9;
        }
        t[i] = w;
    }
    t
}

/// One tileset: decoded sprites, raw bank (for the shadow bitmaps), palette and the two remap tables shadows use.
pub struct Tileset {
    pub sprites: Vec<Vec<Px>>,
    /// tactical_map.md "Ground items": the decoded DAT/SINV.DAT sprites (empty when the file is missing).
    pub items: Vec<Vec<Px>>,
    pub bank: Vec<u8>,
    /// Absolute byte offset in `bank` of every entry.
    pub offsets: Vec<usize>,
    pub palette: [u8; 768],
    pub table0: [u8; 256],
    pub table28: [u8; 256],
    /// Number of compiled entries whose code had a form the decoder does not know (kept partially empty).
    pub decode_errors: usize,
}

/// LoadTilePalette 0x2A990: MISC\TILE<n>.PAL (768 6-bit bytes), here scaled to 8 bits.
pub fn load_palette(iso: &mut Iso, tileset: u8) -> io::Result<[u8; 768]> {
    palette_from(&iso.read(&format!("MISC/TILE{}.PAL", tileset))?, tileset)
}

/// `load_palette` on the bytes of the file.
fn palette_from(pal: &[u8], tileset: u8) -> io::Result<[u8; 768]> {
    if pal.len() < 768 {
        return Err(bad(format!("TILE{}.PAL is {} bytes", tileset, pal.len())));
    }
    let mut out = [0u8; 768];
    for i in 0..768 {
        out[i] = vga8(pal[i]);
    }
    Ok(out)
}

/// The compiled bank of a TILES file and its entry offsets (OpenTilesDat 0x36EA0, LoadOffsetBank 0x52E00). Port rule:
/// fewer than BANK_COUNT entries or an entry past the end of the bank fails.
fn bank_from(file: &[u8], tileset: u8) -> io::Result<(Vec<u8>, Vec<usize>)> {
    let loaded = crate::offsetbank::open_tiles(Some(file)).map_err(|_| bad(format!("TILES{}.DAT does not load", tileset)))?;
    let (bank, offsets) = (loaded.image, loaded.entries);
    if offsets.len() < BANK_COUNT {
        return Err(bad(format!("TILES{}.DAT has {} entries", tileset, offsets.len())));
    }
    if let Some(i) = offsets.iter().position(|&o| o > bank.len()) {
        return Err(bad(format!("entry {} past the end", i)));
    }
    Ok((bank, offsets))
}

impl Tileset {
    pub fn load(iso: &mut Iso, common: &Common, tileset: u8) -> io::Result<Tileset> {
        let file = iso.read(&crate::offsetbank::tiles_name(tileset))?;
        let (bank, offsets) = bank_from(&file, tileset)?;
        let mut sprites = Vec::with_capacity(COMPILED);
        let mut decode_errors = 0;
        for i in 0..COMPILED {
            // The code runs to `ret`; the slice to the end of the bank is fine since decode stops there.
            match decode(&bank[offsets[i]..]) {
                Ok(v) => sprites.push(v),
                Err(_) => {
                    decode_errors += 1;
                    sprites.push(Vec::new());
                }
            }
        }
        let palette = load_palette(iso, tileset)?;
        let lst = iso.read(&format!("MISC/TILESET{}.LST", tileset))?;
        let table28 = table28_from_lst(&common.remap[28], &lst, tileset);
        let items = iso.read("DAT/SINV.DAT").map(|b| sinv(&b)).unwrap_or_default();
        Ok(Tileset { sprites, items, bank, offsets, palette, table0: common.remap[0], table28, decode_errors })
    }
}

/// The sprites of DAT/SINV.DAT (offsetbank::load); a bad entry or a bank that does not load stays empty.
fn sinv(file: &[u8]) -> Vec<Vec<Px>> {
    let Ok(b) = crate::offsetbank::load(Some(file), crate::offsetbank::SINV_MAX) else { return Vec::new() };
    b.entries.iter().map(|&o| b.image.get(o..).and_then(|c| decode(c).ok()).unwrap_or_default()).collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Ledger rows ja1front:7617 and c2aea50:ja1front:8109 (port rule): a JA.EXE one byte short of the shadow table
    /// end is refused with an error (no read past the file); at the full length the tables load.
    #[test]
    fn exe_tables_need_the_shadow_table() {
        let end = 0xDCF08 + SHADOW_IDS * 3;
        let p = std::env::temp_dir().join(format!("ja1front_exetables_{}.bin", std::process::id()));
        let p = p.to_str().unwrap().to_string();
        std::fs::write(&p, vec![0u8; end - 1]).unwrap();
        let short = ExeTables::load(&p);
        std::fs::write(&p, vec![0u8; end]).unwrap();
        let full = ExeTables::load(&p);
        let _ = std::fs::remove_file(&p);
        assert!(short.is_err());
        assert_eq!(full.unwrap().shadow.len(), SHADOW_IDS);
    }

    /// Ledger rows ja1front:7509, c2aea50:ja1front:8016, ja1front:7687, ja1front:7691, c2aea50:ja1front:8191 (port
    /// rules): a TERRAIN.DAT, JAGGED.DAT or TILE<n>.PAL one byte short fails; a TILES bank needs BANK_COUNT entries,
    /// and an entry may end exactly at the end of the bank.
    #[test]
    fn short_files_fail() {
        let need = REMAP_AT + REMAP_COUNT * 256 + 157 + 256 + 155;
        assert!(Common::from_files(vec![0; RECORD * SECTORS - 1], &vec![0; need]).is_err());
        assert!(Common::from_files(vec![0; RECORD * SECTORS], &vec![0; need - 1]).is_err());
        assert!(Common::from_files(vec![0; RECORD * SECTORS], &vec![0; need]).is_ok());
        assert!(palette_from(&[0; 767], 1).is_err());
        assert!(palette_from(&[0; 768], 1).is_ok());
        let bank = |n: usize, last: u32| {
            let mut f = (n as u32).to_le_bytes().to_vec();
            for k in 0..n {
                let o: u32 = if k == n - 1 { last } else { 0 };
                f.extend_from_slice(&o.to_le_bytes());
            }
            f.extend_from_slice(&[0xC3; 4]);
            f
        };
        assert!(bank_from(&bank(BANK_COUNT - 1, 0), 1).is_err());
        assert!(bank_from(&bank(BANK_COUNT, 4), 1).is_ok(), "an entry at the end of the bank");
        assert!(bank_from(&bank(BANK_COUNT, 5), 1).is_err());
    }

    #[test]
    fn palette_values() {
        assert_eq!(vga8(0), 0);
        assert_eq!(vga8(63), 255);
        assert_eq!(vga8(32), 130);
        assert_eq!(vga8(15), 60);
    }
    #[test]
    fn lst() {
        let mut base = [0u8; 256];
        for i in 0..256 {
            base[i] = i as u8;
        }
        let mut lst = [0x10u8; 256];
        lst[0x30] = 255;
        lst[0xA2] = 255; // 255 keeps the base byte, no override
        let t = table28_from_lst(&base, &lst, 9);
        assert_eq!(t[0x30], 0x30);
        assert_eq!(t[0xA2], 0xA2);
        assert_eq!(t[0xA3], 0xA3);
        assert_eq!(t[0x2E], 0x2F);
        assert_eq!(t[0xB9], 0xBD);
        assert_eq!((t[0xF5], t[0xF7], t[0xF9]), (0xF9, 0xF9, 0xF9));
        assert_eq!(t[0x31], 0x10);
        let t = table28_from_lst(&base, &lst, 3);
        assert_eq!(t[0xB9], 0x10);
        let t = table28_from_lst(&base, &lst, 7);
        assert_eq!(t[0xB9], 0xBD);
    }
}

/// The version texts in JA.EXE (1.13 and 1.13b have the same layout; file offsets): key V on the tactical screen
/// (TacticalKeyHandler, 0x90334), A.I.M. (AimScreen 0x202CA, 0x903F4: 1.13b did not patch this one, it still says
/// v1.13) and the office (OfficeScreen 0x4F50A, 0x90E04).
pub const VERSION_TACTICAL: usize = 0xD73D8;
pub const VERSION_AIM: usize = 0xD7498;
pub const VERSION_OFFICE: usize = 0xD7EA8;

/// The NUL-ended text at file offset `off` of `exe` (read once per file and offset); "" when it cannot be read.
pub fn exe_text(exe: &str, off: usize) -> &'static str {
    use std::collections::HashMap;
    use std::sync::{Mutex, OnceLock};
    static CACHE: OnceLock<Mutex<HashMap<(String, usize), &'static str>>> = OnceLock::new();
    let mut c = CACHE.get_or_init(Default::default).lock().unwrap_or_else(|e| e.into_inner());
    c.entry((exe.to_string(), off))
        .or_insert_with(|| {
            let t = std::fs::read(exe)
                .ok()
                .and_then(|d| d.get(off..).map(|r| r.iter().take_while(|&&b| b != 0).take(80).copied().collect::<Vec<u8>>()))
                .unwrap_or_default();
            Box::leak(String::from_utf8_lossy(&t).into_owned().into_boxed_str())
        })
}
