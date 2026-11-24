//! Compiled sprites: translate the x86 code of one entry into a pixel list (no emulator).
//!
//! Forms (tactical_map.md, TILES*n*.DAT): `mov byte/word/dword [edi + k], imm`, `add edi, ebx`, `ret`.
//! "All operand sizes": byte (C6 /0 ib), word (66 C7 /0 iw), dword (C7 /0 id); the address [edi], [edi + disp8]
//! or [edi + disp32]. `add edi, ebx` has two encodings: 01 DF and 03 FB.

/// One opaque pixel: `row` = number of `add edi, ebx` before it, `col` = k plus the byte index of the store.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Px {
    pub row: i32,
    pub col: i32,
    pub val: u8,
}

#[derive(Debug, PartialEq, Eq)]
pub enum DecodeError {
    /// Byte offset in the entry, and the opcode byte found there.
    Unknown(usize, u8),
    /// The code ran past the end of the entry without `ret`.
    NoRet,
}

pub fn decode(code: &[u8]) -> Result<Vec<Px>, DecodeError> {
    let mut px = Vec::new();
    let mut row = 0i32;
    let mut p = 0usize;
    let get = |i: usize| code.get(i).copied().ok_or(DecodeError::NoRet);
    loop {
        let op = get(p)?;
        match op {
            0xC3 => return Ok(px),
            0x01 | 0x03 => {
                // 01 DF = add edi, ebx (r/m = edi, reg = ebx); 03 FB = add edi, ebx (reg = edi, r/m = ebx).
                let m = get(p + 1)?;
                if (op == 0x01 && m == 0xDF) || (op == 0x03 && m == 0xFB) {
                    row += 1;
                    p += 2;
                } else {
                    return Err(DecodeError::Unknown(p, op));
                }
            }
            0x66 | 0xC6 | 0xC7 => {
                let (size, q) = match op {
                    0xC6 => (1usize, p + 1),
                    0xC7 => (4, p + 1),
                    _ => {
                        if get(p + 1)? != 0xC7 {
                            return Err(DecodeError::Unknown(p, op));
                        }
                        (2, p + 2)
                    }
                };
                let modrm = get(q)?;
                // reg field must be 0 (/0 = mov), r/m = 7 (edi).
                if modrm & 0x38 != 0 || modrm & 7 != 7 {
                    return Err(DecodeError::Unknown(p, op));
                }
                let (k, imm_at) = match modrm >> 6 {
                    0 => (0i32, q + 1),
                    1 => (get(q + 1)? as i8 as i32, q + 2),
                    2 => {
                        let b = [get(q + 1)?, get(q + 2)?, get(q + 3)?, get(q + 4)?];
                        (i32::from_le_bytes(b), q + 5)
                    }
                    _ => return Err(DecodeError::Unknown(p, op)),
                };
                for i in 0..size {
                    px.push(Px { row, col: k + i as i32, val: get(imm_at + i)? });
                }
                p = imm_at + size;
            }
            _ => return Err(DecodeError::Unknown(p, op)),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Ledger row c2aea50:ja1front:22531: 01 DF and 03 FB are the row step; 01 FB is no known form.
    #[test]
    fn only_01df_and_03fb_step_a_row() {
        assert!(decode(&[0x01, 0xDF, 0x03, 0xFB, 0xC3]).is_ok());
        assert!(matches!(decode(&[0x01, 0xFB, 0xC3]), Err(DecodeError::Unknown(0, 0x01))));
    }
    #[test]
    fn all_forms() {
        let code = [
            0xC6, 0x07, 0x11, // mov byte [edi], 0x11
            0xC6, 0x47, 0xFF, 0x12, // mov byte [edi-1], 0x12
            0x01, 0xDF, // add edi, ebx
            0x66, 0xC7, 0x47, 0x02, 0x21, 0x22, // mov word [edi+2], 0x2221
            0x03, 0xFB, // add edi, ebx
            0xC7, 0x87, 0x00, 0x01, 0x00, 0x00, 1, 2, 3, 4, // mov dword [edi+0x100], 0x04030201
            0x66, 0xC7, 0x07, 5, 6, // mov word [edi], 0x0605
            0xC6, 0x87, 0x10, 0, 0, 0, 7, // mov byte [edi+0x10] (disp32), 7
            0xC7, 0x47, 4, 8, 9, 10, 11, // mov dword [edi+4], ...
            0xC3, 0x90, 16, 0, 14, 0, // ret + trailer
        ];
        let v = decode(&code).unwrap();
        let t: Vec<(i32, i32, u8)> = v.iter().map(|p| (p.row, p.col, p.val)).collect();
        assert_eq!(
            t,
            vec![
                (0, 0, 0x11),
                (0, -1, 0x12),
                (1, 2, 0x21),
                (1, 3, 0x22),
                (2, 256, 1),
                (2, 257, 2),
                (2, 258, 3),
                (2, 259, 4),
                (2, 0, 5),
                (2, 1, 6),
                (2, 16, 7),
                (2, 4, 8),
                (2, 5, 9),
                (2, 6, 10),
                (2, 7, 11),
            ]
        );
    }
    #[test]
    fn rejects() {
        assert_eq!(decode(&[0x90, 0xC3]), Err(DecodeError::Unknown(0, 0x90)));
        assert_eq!(decode(&[0xC6, 0x07]), Err(DecodeError::NoRet));
        assert_eq!(decode(&[0x01, 0xD8, 0xC3]), Err(DecodeError::Unknown(0, 0x01)));
    }
}
