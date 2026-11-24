//! Minimal ISO 9660 reader: primary volume descriptor + directory records, find a file by path.
use std::fs::File;
use std::io::{self, Read, Seek, SeekFrom};

/// Whole-file counterpart to ReadWholeFile 0x52768 and the OpenCDFile calls (port_api.md op 86 / op 307): the
/// game-folder file first (the folder of `exe`, so the official 1.13b patch files DAT\XRA.DAT and DAT\QUESTS.DAT win),
/// the CD file only when it is absent.
pub fn game_file(exe: Option<&str>, iso: &mut Iso, name: &str) -> io::Result<Vec<u8>> {
    if let Some(dir) = exe.and_then(|e| std::path::Path::new(e).parent()) {
        if let Some(b) = read_any_case(dir, name) {
            return Ok(b);
        }
    }
    iso.read(name)
}

/// The file `name` (parts split by a backslash) under `dir`: DOS finds a name in any case, so each part that is not there
/// as written matches a folder entry with no regard to case.
fn read_any_case(dir: &std::path::Path, name: &str) -> Option<Vec<u8>> {
    let mut at = dir.to_path_buf();
    for part in name.split(['\\', '/']) {
        let exact = at.join(part);
        at = if exact.exists() {
            exact
        } else {
            std::fs::read_dir(&at).ok()?.flatten().map(|e| e.path())
                .find(|p| p.file_name().is_some_and(|f| f.to_string_lossy().eq_ignore_ascii_case(part)))?
        };
    }
    std::fs::read(at).ok()
}

pub struct Iso {
    file: File,
    block: u64,
    root_lba: u32,
    root_len: u32,
}

#[derive(Clone, Copy, Debug)]
struct Entry {
    lba: u32,
    len: u32,
    dir: bool,
}

fn bad(msg: &str) -> io::Error {
    io::Error::new(io::ErrorKind::InvalidData, msg.to_string())
}

/// Name as stored, without the ";n" version suffix and a trailing '.', upper-case.
fn clean_name(raw: &[u8]) -> String {
    let s: String = raw.iter().map(|&b| b as char).collect();
    let s = match s.find(';') {
        Some(i) => &s[..i],
        None => &s[..],
    };
    s.trim_end_matches('.').to_ascii_uppercase()
}

impl Iso {
    pub fn open(path: &str) -> io::Result<Iso> {
        let mut file = File::open(path)?;
        // Volume descriptors start at sector 16 (2048-byte sectors); scan for type 1 (primary).
        for i in 16..64u64 {
            let mut vd = [0u8; 2048];
            file.seek(SeekFrom::Start(i * 2048))?;
            file.read_exact(&mut vd)?;
            if &vd[1..6] != b"CD001" {
                return Err(bad("no ISO 9660 volume descriptor"));
            }
            match vd[0] {
                1 => {
                    let block = u16::from_le_bytes([vd[128], vd[129]]) as u64;
                    let rec = &vd[156..190];
                    let root_lba = u32::from_le_bytes([rec[2], rec[3], rec[4], rec[5]]);
                    let root_len = u32::from_le_bytes([rec[10], rec[11], rec[12], rec[13]]);
                    if block == 0 {
                        return Err(bad("block size 0"));
                    }
                    return Ok(Iso { file, block, root_lba, root_len });
                }
                255 => break,
                _ => {}
            }
        }
        Err(bad("no primary volume descriptor"))
    }

    fn read_extent(&mut self, lba: u32, len: u32) -> io::Result<Vec<u8>> {
        let mut v = vec![0u8; len as usize];
        self.file.seek(SeekFrom::Start(lba as u64 * self.block))?;
        self.file.read_exact(&mut v)?;
        Ok(v)
    }

    fn list(&mut self, lba: u32, len: u32) -> io::Result<Vec<(String, Entry)>> {
        let data = self.read_extent(lba, len)?;
        let bs = self.block as usize;
        let mut out = Vec::new();
        let mut p = 0usize;
        while p < data.len() {
            let rl = data[p] as usize;
            if rl == 0 {
                // Records never cross a block boundary: skip to the next block.
                p = (p / bs + 1) * bs;
                continue;
            }
            if p + rl > data.len() || rl < 34 {
                break;
            }
            let r = &data[p..p + rl];
            let nl = r[32] as usize;
            if 33 + nl <= rl {
                let name = &r[33..33 + nl];
                // Skip "." and ".." (single bytes 0 and 1).
                if !(nl == 1 && (name[0] == 0 || name[0] == 1)) {
                    let e = Entry {
                        lba: u32::from_le_bytes([r[2], r[3], r[4], r[5]]),
                        len: u32::from_le_bytes([r[10], r[11], r[12], r[13]]),
                        dir: r[25] & 2 != 0,
                    };
                    out.push((clean_name(name), e));
                }
            }
            p += rl;
        }
        Ok(out)
    }

    fn find(&mut self, path: &str) -> io::Result<Entry> {
        let mut cur = Entry { lba: self.root_lba, len: self.root_len, dir: true };
        let parts: Vec<&str> = path.split(['/', '\\']).filter(|s| !s.is_empty()).collect();
        for part in parts {
            if !cur.dir {
                return Err(io::Error::new(io::ErrorKind::NotFound, path.to_string()));
            }
            let want = clean_name(part.as_bytes());
            let list = self.list(cur.lba, cur.len)?;
            cur = list
                .into_iter()
                .find(|(n, _)| *n == want)
                .map(|(_, e)| e)
                .ok_or_else(|| io::Error::new(io::ErrorKind::NotFound, path.to_string()))?;
        }
        Ok(cur)
    }

    /// Whole file at `path` (e.g. "DAT/TERRAIN.DAT").
    pub fn read(&mut self, path: &str) -> io::Result<Vec<u8>> {
        let e = self.find(path)?;
        if e.dir {
            return Err(bad("is a directory"));
        }
        self.read_extent(e.lba, e.len)
    }
}

#[cfg(test)]
mod tests {
    use super::{clean_name, game_file, Iso};

    /// Ledger row c2aea50:ja1front:14118 (ISO 9660): the descriptor set ends at a type 255 descriptor; a primary
    /// descriptor after it is not read.
    #[test]
    fn the_terminator_ends_the_descriptor_scan() {
        let p = std::env::temp_dir().join(format!("ja1_isoterm_{}.iso", std::process::id()));
        let mut img = vec![0u8; 18 * 2048];
        for (s, t) in [(16usize, 255u8), (17, 1)] {
            img[s * 2048] = t;
            img[s * 2048 + 1..s * 2048 + 6].copy_from_slice(b"CD001");
        }
        img[17 * 2048 + 129] = 8;
        std::fs::write(&p, &img).unwrap();
        let r = Iso::open(p.to_str().unwrap());
        img.swap(16 * 2048, 17 * 2048);
        img[16 * 2048 + 129] = 8;
        std::fs::write(&p, &img).unwrap();
        let ok = Iso::open(p.to_str().unwrap()).is_ok();
        std::fs::remove_file(&p).ok();
        assert!(r.is_err());
        assert!(ok);
    }
    #[test]
    fn names() {
        assert_eq!(clean_name(b"TERRAIN.DAT;1"), "TERRAIN.DAT");
        assert_eq!(clean_name(b"dat"), "DAT");
        assert_eq!(clean_name(b"README.;1"), "README");
    }

    /// DOS finds a game-folder file in any case; a missing file stays missing (the CD file is the next try).
    #[test]
    fn game_folder_names_in_any_case() {
        let dir = std::env::temp_dir().join(format!("ja1_case_{}", std::process::id()));
        std::fs::create_dir_all(dir.join("dat")).unwrap();
        std::fs::write(dir.join("dat").join("Quests.dat"), b"q").unwrap();
        std::fs::write(dir.join("XRA.DAT"), b"x").unwrap();
        assert_eq!(super::read_any_case(&dir, "DAT\\QUESTS.DAT").as_deref(), Some(&b"q"[..]));
        assert_eq!(super::read_any_case(&dir, "XRA.DAT").as_deref(), Some(&b"x"[..]));
        assert_eq!(super::read_any_case(&dir, "DAT\\XRA.DAT"), None);
        std::fs::remove_dir_all(&dir).unwrap();
    }

    /// OpenCDFile order (#24): the 1.13b game folder's QUESTS.DAT / XRA.DAT win over the CD; a file only on the CD
    /// (DAT\TILES0.DAT) and a missing game folder give the CD bytes.
    #[test]
    fn game_file_order() {
        let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
        let (iso, exe) = (root.join("rom/JA1.gog"), root.join("build/game113b/JA.EXE"));
        if !iso.exists() || !exe.exists() {
            return;
        }
        let mut cd = Iso::open(iso.to_str().unwrap()).unwrap();
        let exe = exe.to_str().unwrap();
        for (f, n) in [("DAT\\QUESTS.DAT", 204), ("DAT\\XRA.DAT", 4)] {
            let a = game_file(Some(exe), &mut cd, f).unwrap();
            let b = cd.read(f).unwrap();
            assert_eq!(a.len(), b.len());
            assert_eq!(a.iter().zip(&b).filter(|(x, y)| x != y).count(), n, "{}", f);
            assert_eq!(game_file(None, &mut cd, f).unwrap(), b);
        }
        let t = cd.read("DAT\\TILES0.DAT").unwrap();
        assert_eq!(game_file(Some(exe), &mut cd, "DAT\\TILES0.DAT").unwrap(), t);
    }
}
