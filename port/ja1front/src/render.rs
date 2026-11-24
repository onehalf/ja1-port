//! The static scene (tactical_map.md "Back buffer", "Drawing order").
use crate::data::{Common, ExeTables, Tileset, RAW_FIRST, SHADOW_IDS};
use crate::sprite::Px;
use crate::soldier::{draw_soldier, SoldierEnv, REC};
use crate::cursor::{self, CursorEnv, CursorState};
use crate::panel::Font;
use std::cell::RefCell;

pub const PITCH: i32 = 384;
pub const HEIGHT: i32 = 28 + 14 * 57 + 28; // 854
pub const WIN_X: i32 = 32;
pub const WIN_Y: i32 = 28;
pub const MAP: i32 = 56;
/// Widest window that fits the 384-pixel buffer after the 32-pixel margin.
pub const MAX_COLS: i32 = (PITCH - WIN_X) / 16; // 22

pub struct Scene<'a> {
    pub common: &'a Common,
    pub exe: &'a ExeTables,
    pub ts: &'a Tileset,
    pub sector: usize,
    pub c0: i32,
    pub r0: i32,
    pub cols: i32,
    pub rows: i32,
    /// A given tile map (3136 x 8 bytes, map index = row * 56 + column) instead of TERRAIN.DAT.
    pub tilemap: Option<&'a [u8]>,
    /// Soldier records (48 x 512, anchor fields written), script words and draw environment.
    pub soldiers: Option<SceneSoldiers<'a>>,
    /// The tile cursor (cursor.md "Drawing"), drawn when given.
    pub cursor: Option<SceneCursor<'a>>,
    /// The "sees enemies" line (panel.md "Tactical messages"): font 2, text and centre x, drawn after the cursor.
    /// Texts on the view after the cursor pass (panel.md "Tactical messages"): font, text, x, y, fg; centred.
    pub lines: Vec<(&'a Font, Vec<u8>, i32, i32, u8)>,
    /// The effect records (combat.md "Drawing the effect records"): the pictures and the `fx_anims` bytes.
    pub fx: Option<SceneFx<'a>>,
    /// The Control Panel switch Shadows (save byte 21542): off = no shadow pass (tactical_map.md pass 2).
    pub shadows: bool,
    /// The roofs flag (tactical_map.md "Drawing order"); off = every ground sprite and tall object, no roofs.
    pub roofs: bool,
    /// Time compression on (panel.md "The turn banner and the other view texts"): after the first n `lines` the
    /// window's pixels become table[pixel].
    pub dark: Option<(usize, &'a [u8; 256])>,
    /// View modes 2 and 3: the slot portraits over the map, before the texts (TacticalRedraw 0x17759).
    pub portraits: Option<RefCell<crate::panel::ViewPortraits<'a>>>,
    /// The world's ground items (200 x 12 bytes; tactical_map.md "Ground items"); None = none drawn.
    pub ground: Option<&'a [u8]>,
}

/// The effect pictures of MAIN.DAT (combat.md "Drawing the effect records").
pub struct FxSheets {
    /// EXPLODE.PCX: 1596 x 60, 21 frames of 76 x 60.
    pub explode: Vec<u8>,
    /// SMOKBOMB.PCC: 420 x 34, 10 frames of 42 x 34.
    pub smoke: Vec<u8>,
}

/// Width and height of EXPLODE.PCX and SMOKBOMB.PCC.
pub const EXPLODE_W: usize = 1596;
pub const EXPLODE_H: usize = 60;
pub const SMOKE_W: usize = 420;
pub const SMOKE_H: usize = 34;

impl FxSheets {
    /// The two pictures from MAIN.DAT in the directory of JA.EXE.
    pub fn load(exe_path: &str) -> std::io::Result<FxSheets> {
        let dir = std::path::Path::new(exe_path).parent().map(|p| p.to_path_buf()).unwrap_or_default();
        let main = std::fs::read(dir.join("DAT").join("MAIN.DAT"))?;
        let pic = |name: &str, w: usize, h: usize| -> std::io::Result<Vec<u8>> {
            let (pw, ph, px) = crate::panel::pcx(&crate::panel::archive_member(&main, name)?)?;
            // SPEC GAP: a picture of another size is cut or padded with 0 to the size the original loads.
            let mut v = vec![0u8; w * h];
            for r in 0..h.min(ph) {
                for c in 0..w.min(pw) {
                    v[r * w + c] = px[r * pw + c];
                }
            }
            Ok(v)
        };
        Ok(FxSheets { explode: pic("EXPLODE.PCX", EXPLODE_W, EXPLODE_H)?, smoke: pic("SMOKBOMB.PCC", SMOKE_W, SMOKE_H)? })
    }
}

/// The effect records to draw: the pictures and the `fx_anims` buffer (16 x 0x12 bytes).
pub struct SceneFx<'a> {
    pub sheets: &'a FxSheets,
    pub entries: &'a [u8],
    /// tactical_map.md "Animated tiles": the frame of each tile animation 0..9 (None = count 0).
    pub tile_frames: [Option<u8>; 10],
}

pub struct SceneCursor<'a> {
    pub st: RefCell<&'a mut CursorState>,
    pub font0: &'a Font,
}

pub struct SceneSoldiers<'a> {
    pub env: SoldierEnv<'a>,
    pub records: RefCell<&'a mut [u8]>,
    pub script_words: &'a [u16],
    /// tactical_map.md "Soldier texts" (None = the soldiers only, as the test interface draws them).
    pub texts: Option<RefCell<SoldierTexts<'a>>>,
}

/// tactical_map.md "Soldier texts": fonts 0 and 1, remap table 0, the lit and the named slot (cursor.md "Hover"),
/// the hit numbers (5 bytes per slot).
pub struct SoldierTexts<'a> {
    pub font0: &'a Font,
    pub font1: &'a Font,
    pub table0: [u8; 256],
    pub lit: u8,
    pub named: u8,
    pub dmg_text: &'a [u8],
}

struct Buf(Vec<u8>);

impl Buf {
    #[inline]
    fn put(&mut self, off: i64, v: u8) {
        // A pixel outside 0 .. 384 x 854 - 1 is dropped; x >= 384 simply lands on the next row.
        if off >= 0 && (off as usize) < self.0.len() {
            self.0[off as usize] = v;
        }
    }
    fn sprite(&mut self, px: &[Px], x: i32, y: i32) {
        for p in px {
            let off = (y + p.row) as i64 * PITCH as i64 + (x + p.col) as i64;
            self.put(off, p.val);
        }
    }
}

impl<'a> Scene<'a> {
    fn on_map(&self, c: i32, r: i32) -> bool {
        let (mc, mr) = (self.c0 + c, self.r0 + r);
        (0..MAP).contains(&mc) && (0..MAP).contains(&mr)
    }
    /// Tile bytes of window tile (c, r); the caller checks `on_map`.
    fn tile(&self, c: i32, r: i32) -> [u8; 5] {
        if let Some(m) = self.tilemap {
            let k = ((self.r0 + r) * 56 + self.c0 + c) as usize;
            return [m[k * 8], m[k * 8 + 1], m[k * 8 + 2], m[k * 8 + 3], m[k * 8 + 4]];
        }
        self.common.tile(self.sector, (self.c0 + c) as usize, (self.r0 + r) as usize)
    }
    /// Tile byte 7 (path marks) of window tile (c, r); 0 without a tile map.
    fn mark(&self, c: i32, r: i32) -> u8 {
        match self.tilemap {
            Some(m) => m[((self.r0 + r) * 56 + self.c0 + c) as usize * 8 + 7],
            None => 0,
        }
    }
    /// tactical_map.md "Ground items": the item picture of window tile (c, r), if it shows one.
    fn ground_item(&self, c: i32, r: i32) -> Option<&'a [Px]> {
        let (m, gi) = (self.tilemap?, self.ground?);
        let g = m[((self.r0 + r) * 56 + self.c0 + c) as usize * 8 + 6] as usize;
        if g >= 200 || gi.get(12 * g + 6).map_or(true, |&b| b as i8 <= 0) {
            return None;
        }
        let s = *self.exe.item_sprite.get(gi[12 * g] as usize)? as usize;
        self.ts.items.get(s).map(|v| v.as_slice())
    }
    /// Occupant byte t5 of a map tile by linear index (0 without a tile map; no soldiers there).
    fn occupant_at(&self, k: i32) -> u8 {
        match self.tilemap {
            Some(m) if (0..(MAP * MAP)).contains(&k) => m[k as usize * 8 + 5],
            _ => 0x40,
        }
    }
    fn t1_at(&self, k: i32) -> Option<u8> {
        if !(0..(MAP * MAP)).contains(&k) {
            return None;
        }
        let (col, row) = ((k % MAP) as usize, (k / MAP) as usize);
        Some(match self.tilemap {
            Some(m) => m[k as usize * 8 + 1],
            None => self.common.tile(self.sector, col, row)[1],
        })
    }
    /// combat.md "Drawing the effect records": entries 1..15 with count ≠ 0 and kind ≠ 0.
    /// DrawProjectilePass? 0x17548: the projectiles in flight (the shadow sprite +0x1C5 at (+0xF6, +0x9E) of slots
    /// 0..15 and 40..47 with +0x1C7 set, combat.md "Projectile flight"), then the effect records: kind 0 = the
    /// sprite of its look's frame table at the count (bullet puff / splash), else the explosion frames.
    fn effects(&self, buf: &mut Buf, fx: &SceneFx) {
        if let Some(sd) = &self.soldiers {
            let recs = sd.records.borrow();
            for s in (0..16).chain(40..48) {
                let Some(r) = recs.get(s * REC..(s + 1) * REC) else { continue };
                if r[0x1C7] != 0 {
                    let (x, y) = (i16::from_le_bytes([r[0xF6], r[0xF7]]) as i32, i16::from_le_bytes([r[0x9E], r[0x9F]]) as i32);
                    self.world_sprite(buf, r[0x1C5] as usize, x, y);
                }
            }
        }
        let table0 = &self.ts.table0;
        for k in 1..16 {
            let Some(e) = fx.entries.get(k * 0x12..(k + 1) * 0x12) else { break };
            let (c, kind) = (e[0] as i8 as i32, e[0xB]);
            if c == 0 {
                continue;
            }
            let x = i16::from_le_bytes([e[5], e[6]]) as i32;
            let y = i16::from_le_bytes([e[7], e[8]]) as i32;
            if kind == 0 {
                // The frame tables of host op 178's looks: 0 splash 0x95670, 1 puff 0x9566B (5 bytes each, frame 0
                // unused); the original reads table + count (s8).
                // SPEC GAP: other looks and counts outside 0..4 have no kind-0 table here; nothing is drawn.
                let look = i32::from_le_bytes([e[0xC], e[0xD], e[0xE], e[0xF]]);
                let table: &[u8] = match look {
                    0 => &[0, 0x7C, 0x7D, 0x7E, 0x7F],
                    1 => &[0, 0x78, 0x79, 0x7A, 0x7B],
                    _ => &[],
                };
                if let Some(&n) = usize::try_from(c).ok().and_then(|i| table.get(i)) {
                    self.world_sprite(buf, n as usize, x, y);
                }
                continue;
            }
            draw_effect(&mut buf.0, fx.sheets, table0, c, kind, x - 16 * self.c0 + WIN_X, y - 14 * self.r0 + WIN_Y);
        }
    }
    /// DrawSpriteAtWorld 0x1698C: sprite 413 + n at world pixel (x, y) when 16 < X < view_w + 32 and
    /// 14 < Y < view_h + 28 (X, Y = the back-buffer position).
    fn world_sprite(&self, buf: &mut Buf, n: usize, x: i32, y: i32) {
        let (bx, by) = (x - 16 * self.c0 + WIN_X, y - 14 * self.r0 + WIN_Y);
        if bx > 16 && by > 14 && bx < 16 * self.cols + 32 && by < 14 * self.rows + 28 {
            self.sprite(buf, 413 + n, bx, by);
        }
    }
    fn corner(c: i32, r: i32) -> (i32, i32) {
        (WIN_X + 16 * c, WIN_Y + 14 * r)
    }
    fn sprite(&self, buf: &mut Buf, n: usize, x: i32, y: i32) {
        // SPEC GAP: sprite numbers at or past 1293 (a large t1/t2/t3 or composite value) are raw bitmaps, not code;
        // the spec does not say what the game draws then. They are not drawn here.
        if let Some(px) = self.ts.sprites.get(n) {
            buf.sprite(px, x, y);
        }
    }

    fn shadow(&self, buf: &mut Buf, id: usize, x: i32, y: i32) {
        let Some(&(bmp, dx, dy)) = self.exe.shadow.get(id) else { return };
        let entry = RAW_FIRST + bmp as usize;
        // SPEC GAP: a table bitmap number past entry 1326 is not described; such a shadow is not drawn.
        let Some(&src) = self.ts.offsets.get(entry) else { return };
        let (sx, sy) = (x + dx as i32, y + dy as i32);
        // Quirk: rows above the buffer top are skipped, but 14 rows are still drawn, reading on past this bitmap.
        // SPEC GAP: "reading on into the next bitmap" is taken as the bytes that follow this entry in the bank file.
        let skip = if sy < 0 { -sy } else { 0 };
        for i in 0..14 {
            let srow = (skip + i) as usize;
            let by = sy + skip + i;
            for j in 0..16 {
                let v = self.ts.bank.get(src + srow * 16 + j as usize).copied().unwrap_or(0);
                if v == 0 {
                    continue;
                }
                let off = by as i64 * PITCH as i64 + (sx + j) as i64;
                if off < 0 || off as usize >= buf.0.len() {
                    continue;
                }
                let d = buf.0[off as usize];
                buf.0[off as usize] = if v == 0x17 { self.ts.table28[d as usize] } else { self.ts.table0[v as usize] };
            }
        }
    }

    /// tactical_map.md "Soldier texts": after the soldier's bitmap, at its place (x, y, width w).
    fn soldier_texts(&self, buf: &mut [u8], tx: &mut SoldierTexts, rec: &[u8], (x, y, w): (i32, i32, i32)) {
        let slot = rec[0x17A];
        let s16 = |o: usize| i16::from_le_bytes([rec[o], rec[o + 1]]) as i32;
        if rec[0x1D2] == 0 {
            if slot == tx.lit || slot == tx.named {
                if let Some(sc) = &self.cursor {
                    let mut st = sc.st.borrow_mut();
                    st.label_y = (if s16(0x114) < 0x38 { y + 8 } else { y - 3 }) as i16;
                    st.label_x = (x + w / 2) as i16;
                }
            }
        } else {
            let (hx, hy) = (x + s16(0xD8), y + s16(0xDA));
            if hy < 14 * self.rows + 28 && hx > 32 && hy > 28 {
                let k = 5 * slot as usize;
                let t = tx.dmg_text.get(k..(k + 5).min(tx.dmg_text.len())).unwrap_or(&[]).to_vec();
                cursor::text(buf, tx.font1, &mut tx.table0, &t, hx, hy, false, 0xFF, 1, false);
            }
        }
        if slot < 8 && rec[0x195] != 0 {
            let n: Vec<u8> = rec[0x16E..0x177].iter().copied().take_while(|&b| b != 0).collect();
            cursor::text(buf, tx.font0, &mut tx.table0, &n, x + w / 2, y + 0x12, true, 0xFF, 1, true);
        }
    }

    /// The whole back buffer (384 x 854) after the four steps. The port redraws the panel instead of retaining
    /// the original's per-element screen rectangle (0x46850) for temporary element animations.
    pub fn back_buffer(&self) -> Vec<u8> {
        let mut buf = Buf(vec![0u8; (PITCH * HEIGHT) as usize]);
        // cursor.md "Drawing": the full redraw clears under_soldier and the hover label first.
        let mut cenv = self.cursor.as_ref().map(|sc| {
            let mut st = sc.st.borrow_mut();
            st.under_soldier = 0;
            st.label_x = -1;
            CursorEnv {
                table: &self.exe.cursor,
                font0: sc.font0,
                table0: self.common.remap[0],
                view_x: 16 * self.c0,
                view_y: 14 * self.r0,
                view_w: 16 * self.cols,
                view_h: 14 * self.rows,
            }
        });
        let (gv1, gv3) = (&self.exe.gv_t1, &self.exe.gv_t3);
        let (t1c, t2c) = (&self.common.t1class, &self.common.t2class);
        // For a cols x rows window the spec's 13 / 14 / 11 / 12 are cols-1 / cols / rows-1 / rows.

        // 1. Ground, window tiles, row by row.
        for r in 0..self.rows {
            for c in 0..self.cols {
                let [t0, t1, t2, t3, _] = self.tile(c, r);
                let (x, y) = Self::corner(c, r);
                if (gv1[t1 as usize] != 0 && gv3[t3 as usize] != 0) || t3 == 0x73 || !self.roofs {
                    self.sprite(&mut buf, t0 as usize, x, y);
                }
                if t1 != 0 && t1c[t1 as usize] < 6 && t3 != 0x73 {
                    self.sprite(&mut buf, 157 + t1 as usize, x, y);
                }
                if t2 != 0 && t2c[t2 as usize] < 12 {
                    self.sprite(&mut buf, 413 + t2 as usize, x, y);
                }
                if t1 < 0x75 {
                    if let Some(px) = self.ground_item(c, r) {
                        buf.sprite(px, x, y);
                    }
                }
                // The path marks of tile byte 7 (select.md).
                let b7 = self.mark(c, r);
                let n = (b7 >> 4) as usize;
                if n != 0 {
                    self.sprite(&mut buf, if n < 10 { 644 + n } else { 595 }, x, y);
                }
                if b7 & 0x0F != 0 {
                    self.sprite(&mut buf, 652 + (b7 & 0x0F) as usize, x, y);
                }
                // tactical_map.md "Animated tiles" (the front-end clears t3 itself when the count runs out).
                if (1..=9).contains(&t3) {
                    if let Some(f) = self.fx.as_ref().and_then(|fx| fx.tile_frames[t3 as usize]) {
                        self.sprite(&mut buf, 413 + f as usize, x, y);
                    }
                }
            }
        }

        // 2. Shadows, rows -1..rows, columns -1..cols-1 (only tiles on the map), while the switch Shadows is on.
        for r in (-1..=self.rows).filter(|_| self.shadows) {
            for c in -1..self.cols {
                if !self.on_map(c, r) {
                    continue;
                }
                let id = (self.tile(c, r)[4] & 0x7F) as i32;
                if id >= SHADOW_IDS as i32 {
                    continue;
                }
                let (x, y) = Self::corner(c, r);
                if id < 0x40 {
                    let (px, py) = (x + 2, y - 2);
                    self.shadow(&mut buf, id as usize, px, py);
                    // DrawShadow2x2 0x16588.
                    for (k, (ox, oy)) in [(1, (-16, 0)), (2, (0, -14)), (3, (-16, -14))] {
                        let (qx, qy) = (px + ox, py + oy);
                        // SPEC GAP: "its position" is taken as the placement point before the table dx, dy.
                        // SPEC GAP: ids id-1..id-3 below 0 (id < 3) are not described; they are skipped.
                        if qx < 0 || qy < 0 || id - k < 0 {
                            continue;
                        }
                        self.shadow(&mut buf, (id - k) as usize, qx, qy);
                    }
                } else {
                    self.shadow(&mut buf, id as usize, x, y);
                }
            }
        }

        // Target mark: after the shadows, before soldiers and objects.
        if let (Some(sc), Some(env)) = (&self.cursor, cenv.as_mut()) {
            let mut st = sc.st.borrow_mut();
            let occ = self.occupant_at(st.tile);
            cursor::target_mark(&mut buf.0, &self.ts.sprites, env, &mut st, occ);
        }

        // 3. Soldiers and objects, rows -1..rows, columns -1..cols, only tiles on the map.
        for r in -1..=self.rows {
            for c in -1..=self.cols {
                if !self.on_map(c, r) {
                    continue;
                }
                let [_, t1, _, t3, _] = self.tile(c, r);
                let class = t1c[t1 as usize];
                let (x, y) = Self::corner(c, r);
                let k = (self.r0 + r) * MAP + self.c0 + c;
                let occ = (self.occupant_at(k) & 0x7F) as usize;
                if occ < 0x30 {
                    if let Some(sd) = &self.soldiers {
                        let mut recs = sd.records.borrow_mut();
                        if (occ + 1) * REC <= recs.len() {
                            let (head, tail) = recs.split_at_mut((occ + 1) * REC);
                            let rec = &mut head[occ * REC..];
                            // SPEC GAP: a script word list shorter than the slot number gives script word 0.
                            let sw = sd.script_words.get(occ).copied().unwrap_or(0);
                            let at = draw_soldier(&mut buf.0, &sd.env, rec, tail, sw, self.c0, self.r0, self.cols, self.rows);
                            if let (Some(at), Some(tx)) = (at, &sd.texts) {
                                self.soldier_texts(&mut buf.0, &mut tx.borrow_mut(), rec, at);
                            }
                        }
                    }
                    if c >= 0 && r >= 0 {
                        if let Some(nt1) = self.t1_at(k - 55) {
                            if nt1 == 0x41 || nt1 == 0x60 {
                                self.sprite(&mut buf, 157 + nt1 as usize, x + 16, y - 14);
                            }
                        }
                    }
                }
                let tall_ok = c >= 0 && r >= 0; // window tiles and column cols / row rows, not -1
                if class >= 7 {
                    if tall_ok && (gv3[t3 as usize] != 0 || !self.roofs) {
                        let (ox, oy) = if t1 > 0x82 { (x - 2, y - 1) } else { (x, y) };
                        self.sprite(&mut buf, 157 + t1 as usize, ox, oy);
                        // The ground item on a tall object (t1 >= 0x75) at (-3, -3) from the object.
                        if t1 >= 0x75 {
                            if let Some(px) = self.ground_item(c, r) {
                                buf.sprite(px, ox - 3, oy - 3);
                            }
                        }
                    }
                } else if class == 6 {
                    let (px, py) = (x + 2, y - 2);
                    self.sprite(&mut buf, 157 + t1 as usize, px, py);
                    // SPEC GAP: the composite table starts at t1 = 0x27; a class-6 t1 below that has no entry and
                    // gets only its own sprite.
                    // DrawComposite2x2 0x164BC.
                    if t1 >= 0x27 {
                        let [al, ab, le] = self.exe.composite[(t1 - 0x27) as usize];
                        // SPEC GAP: "overlay numbers" are drawn as sprite 413 + n (like t2), including n = 0.
                        for (n, (ox, oy)) in [(ab, (0, -14)), (al, (-16, -14)), (le, (-16, 0))] {
                            let (qx, qy) = (px + ox, py + oy);
                            if qx < 0 || qy < 0 {
                                continue;
                            }
                            self.sprite(&mut buf, 413 + n as usize, qx, qy);
                        }
                    }
                }
            }
        }

        // 4. Overlays and roofs, window tiles.
        for r in 0..self.rows {
            for c in 0..self.cols {
                let [_, _, t2, t3, _] = self.tile(c, r);
                let (x, y) = Self::corner(c, r);
                if t2 > 0x3F {
                    self.sprite(&mut buf, 413 + t2 as usize, x, y);
                }
                if t3 > 0x50 && t3 != 0x73 && self.roofs {
                    self.sprite(&mut buf, 413 + t3 as usize, x, y);
                }
            }
        }
        // 5. The projectiles in flight and the effect records (combat.md "Drawing the effect records").
        if let Some(fx) = &self.fx {
            self.effects(&mut buf, fx);
        }
        // Cursor pass: the last step.
        if let (Some(sc), Some(env)) = (&self.cursor, cenv.as_mut()) {
            let mut st = sc.st.borrow_mut();
            static NO_MAP: [u8; 3136 * 8] = [0; 3136 * 8];
            let recs = self.soldiers.as_ref().map(|sd| sd.records.borrow());
            let marks = recs.as_ref().filter(|r| r.len() >= 48 * REC).map(|r| (&r[..], self.tilemap.unwrap_or(&NO_MAP)));
            cursor::cursor_pass(&mut buf.0, &self.ts.sprites, env, &mut st, marks);
        }
        if let (Some(v), Some(sd)) = (&self.portraits, &self.soldiers) {
            v.borrow_mut().draw(&mut buf.0, &mut sd.records.borrow_mut());
        }
        self.view_lines(&mut buf.0, (16 * self.cols) as usize, (14 * self.rows) as usize);
        buf.0
    }

    /// The view texts into the back buffer `bb`, with the darkening of `dark` over the w x h window between them.
    fn view_lines(&self, bb: &mut [u8], w: usize, h: usize) {
        for (i, (f, t, x, y, fg)) in self.lines.iter().enumerate() {
            if let Some((_, table)) = self.dark.filter(|d| d.0 == i) {
                dark_rect(bb, table, w, h);
            }
            let mut table0 = self.common.remap[0];
            cursor::text(bb, f, &mut table0, t, *x, *y, true, *fg, 0xFE, false);
        }
        if let Some((_, table)) = self.dark.filter(|d| d.0 >= self.lines.len()) {
            dark_rect(bb, table, w, h);
        }
    }

    /// The 16*cols x 14*rows window, row-major palette indexes.
    pub fn window(&self) -> Vec<u8> {
        self.window_then(|_| {})
    }

    /// `window` with a last drawing step `f` into the back buffer before the window is cut out.
    pub fn window_then(&self, f: impl FnOnce(&mut [u8])) -> Vec<u8> {
        let mut bb = self.back_buffer();
        f(&mut bb);
        let (w, h) = ((16 * self.cols) as usize, (14 * self.rows) as usize);
        let mut out = Vec::with_capacity(w * h);
        for y in 0..h {
            let at = (WIN_Y as usize + y) * PITCH as usize + WIN_X as usize;
            out.extend_from_slice(&bb[at..at + w]);
        }
        out
    }
}

/// The w x h window at (WIN_X, WIN_Y) of the back buffer: each pixel becomes table[pixel].
fn dark_rect(bb: &mut [u8], table: &[u8; 256], w: usize, h: usize) {
    for y in 0..h {
        let at = (WIN_Y as usize + y) * PITCH as usize + WIN_X as usize;
        if let Some(row) = bb.get_mut(at..at + w) {
            for p in row {
                *p = table[*p as usize];
            }
        }
    }
}

impl<'a> Scene<'a> {
    /// The overview picture of view mode 0 (panel.md "The overview picture") with the view's texts, as the
    /// 224 x 168 block at (32, 28) of the back buffer.
    pub fn overview_window(&self, sheet: &[u8], w: &crate::overview::OverviewIn, font0: &crate::panel::Font,
                           cur: Option<&crate::overview::OvCursor>) -> Vec<u8> {
        let mut bb = vec![0u8; (PITCH * HEIGHT) as usize];
        if let Some(fx) = &self.fx {
            crate::overview::draw(&mut bb, self.ts, self.exe, self.common, sheet, fx.sheets, w);
        }
        // Step 4: the overview cursor, before the texts.
        if let Some(c) = cur {
            let mut table0 = self.common.remap[0];
            crate::overview::cursor(&mut bb, self.ts, font0, &mut table0, c);
        }
        self.view_lines(&mut bb, crate::overview::OV_W as usize, crate::overview::OV_H as usize);
        let (ow, oh) = (crate::overview::OV_W as usize, crate::overview::OV_H as usize);
        let mut out = Vec::with_capacity(ow * oh);
        for y in 0..oh {
            let at = (WIN_Y as usize + y) * PITCH as usize + WIN_X as usize;
            out.extend_from_slice(&bb[at..at + ow]);
        }
        out
    }
}

/// combat.md "Drawing the effect records", steps 1..4: frame c − 1 of an entry of `kind` at back buffer point
/// (x, y) (pitch 384).
pub fn draw_effect(bb: &mut [u8], sh: &FxSheets, table0: &[u8; 256], c: i32, kind: u8, x: i32, y: i32) {
    let (mut w, mut h, mw, mh) = match kind {
        3 => (76, 60, 76, 60),
        2 => (76, 60, 38, 30),
        _ => (42, 34, 42, 34),
    };
    let (mut x, mut y, mut sx, mut sy) = (x, y, 0i64, 0i64);
    if x < 0 {
        sx = -x as i64;
        w += x;
        x = 0;
    }
    if y < 0 {
        sy = -y as i64;
        h += y;
        y = 0;
    }
    if w <= 0 || h <= 0 || x >= PITCH - mw || y > 252 - mh {
        return;
    }
    // A source offset outside the picture reads 0 (SPEC GAP: the original reads the memory after it).
    let px = |sheet: &[u8], o: i64| -> u8 { if o >= 0 { sheet.get(o as usize).copied().unwrap_or(0) } else { 0 } };
    let mut put = |r: i32, col: i32, v: u8| {
        let o = (y + r) as usize * PITCH as usize + (x + col) as usize;
        if let Some(d) = bb.get_mut(o) {
            *d = v;
        }
    };
    let base = (c as i64 - 1) * if kind == 2 || kind == 3 { 76 } else { 42 } + sx;
    match kind {
        3 => {
            for r in 0..h {
                for col in 0..w {
                    let v = px(&sh.explode, (sy + r as i64) * EXPLODE_W as i64 + base + col as i64);
                    if v != 0x7B {
                        put(r, col, v);
                    }
                }
            }
        }
        2 => {
            for r in 0..30 {
                for col in 0..38 {
                    let v = px(&sh.explode, 2 * r as i64 * EXPLODE_W as i64 + base + 2 * col as i64);
                    if v != 0x7B {
                        put(r, col, table0[v as usize]);
                    }
                }
            }
        }
        _ => {
            for r in 0..34 {
                for col in 0..42 {
                    let v = px(&sh.smoke, r as i64 * SMOKE_W as i64 + base + col as i64);
                    if v != 0xD3 {
                        put(r, col, v);
                    }
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Ledger rows ja1front:18326/18330: a pixel outside the buffer is dropped; the last byte is written.
    #[test]
    fn buf_put_drops_outside() {
        let mut b = Buf(vec![0; 4]);
        b.put(-1, 1);
        b.put(4, 1);
        b.put(3, 2);
        assert_eq!(b.0, vec![0, 0, 0, 2]);
    }

    /// Ledger row c2aea50:ja1front:19198: the widest window = (384 - 32) / 16 = 22 tile columns.
    #[test]
    fn max_cols_is_22() {
        assert_eq!(MAX_COLS, 22);
    }

    fn sheets() -> FxSheets {
        // pixel value = column % 251 (+1), with key values in one column of each sheet
        let mut explode: Vec<u8> = (0..EXPLODE_W * EXPLODE_H).map(|i| ((i % EXPLODE_W) % 251) as u8 + 1).collect();
        explode[3] = 0x7B;
        let mut smoke: Vec<u8> = (0..SMOKE_W * SMOKE_H).map(|i| ((i % SMOKE_W) % 200) as u8 + 1).collect();
        smoke[SMOKE_W + 1] = 0xD3;
        FxSheets { explode, smoke }
    }

    #[test]
    fn effect_pictures() {
        let sh = sheets();
        let mut t0 = [0u8; 256];
        for (i, v) in t0.iter_mut().enumerate() {
            *v = (i as u8).wrapping_add(100);
        }
        let p = PITCH as usize;
        // Kind 3, frame 0 at (40, 30): 76 x 60 copied, the key pixel (column 3) not written.
        let mut bb = vec![0u8; p * 300];
        draw_effect(&mut bb, &sh, &t0, 1, 3, 40, 30);
        assert_eq!((bb[30 * p + 40], bb[30 * p + 43], bb[89 * p + 115], bb[90 * p + 40]), (1, 0, 76, 0));
        // Kind 3, frame 1 at x = -10: source column 76 + 10, drawn from x = 0, 66 wide.
        let mut bb = vec![0u8; p * 300];
        draw_effect(&mut bb, &sh, &t0, 2, 3, -10, 30);
        assert_eq!((bb[30 * p], bb[30 * p + 65], bb[30 * p + 66]), (87, 152, 0));
        // Kind 2: half size 38 x 30, every second pixel, remapped with table 0.
        let mut bb = vec![0u8; p * 300];
        draw_effect(&mut bb, &sh, &t0, 1, 2, 40, 30);
        assert_eq!((bb[30 * p + 40], bb[30 * p + 41], bb[59 * p + 77], bb[60 * p + 40]), (101, 103, 175, 0));
        // Other kinds: smoke 42 x 34, the key 0xD3 skipped.
        let mut bb = vec![0u8; p * 300];
        draw_effect(&mut bb, &sh, &t0, 2, 4, 40, 30);
        assert_eq!((bb[30 * p + 40], bb[31 * p + 40], bb[33 * p + 81], bb[30 * p + 82]), (43, 43, 84, 0));
        let mut bb = vec![0u8; p * 300];
        draw_effect(&mut bb, &sh, &t0, 1, 4, 40, 30);
        assert_eq!(bb[31 * p + 41], 0);
        // Out of the view: x >= 384 - 76, y > 252 - 60 draw nothing.
        let mut bb = vec![0u8; p * 300];
        draw_effect(&mut bb, &sh, &t0, 1, 3, 308, 30);
        draw_effect(&mut bb, &sh, &t0, 1, 3, 40, 193);
        draw_effect(&mut bb, &sh, &t0, 1, 3, -76, 30);
        assert!(bb.iter().all(|&v| v == 0));
    }

    /// Ledger rows 0b4d529:ja1front:20006/20007 (DrawGroundPass 0x16BAF..0x16BF4, tactical_map.md "Animated tiles"):
    fn test_scene<R>(f: impl FnOnce(&Common, &ExeTables, &Tileset) -> R) -> Option<R> {
        let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
        let (iso, exe) = (root.join("rom/JA1.gog"), root.join("build/game113b/JA.EXE"));
        if !iso.exists() || !exe.exists() {
            return None;
        }
        let (iso, exe) = (iso.to_str().unwrap(), exe.to_str().unwrap());
        let c = crate::common(iso).unwrap();
        let e = crate::exe_tables(exe).unwrap();
        let t = crate::tileset_data(iso, c.tileset(38)).unwrap();
        Some(f(&c, &e, &t))
    }

    /// Pass 3: a tall object (t1 class >= 7) on the column -1 or the row -1 around the window is not drawn; the
    /// same object on window tile (0, 3) is.
    #[test]
    fn tall_objects_only_on_window_tiles() {
        test_scene(|c, e, t| {
            let mut m = vec![0u8; 3136 * 8];
            let mut draw = |m: &[u8]| {
                Scene { common: c, exe: e, ts: t, sector: 38, c0: 5, r0: 5, cols: 14, rows: 12, tilemap: Some(m),
                        soldiers: None, cursor: None, lines: Vec::new(), portraits: None, fx: None, shadows: false,
                        roofs: false, dark: None, ground: None }.back_buffer()
            };
            let blank = draw(&m);
            let k = |cc: i32, rr: i32| (((5 + rr) * 56 + 5 + cc) * 8 + 1) as usize;
            let mut checked = 0;
            for t1 in 1..=255u8 {
                if c.t1class[t1 as usize] < 7 {
                    continue;
                }
                m[k(0, 3)] = t1;
                let on = draw(&m);
                m[k(0, 3)] = 0;
                if on == blank {
                    continue;
                }
                for (cc, rr) in [(-1, 3), (3, -1)] {
                    m[k(cc, rr)] = t1;
                    assert!(draw(&m) == blank, "t1 {t1} at ({cc}, {rr})");
                    m[k(cc, rr)] = 0;
                }
                checked += 1;
                if checked == 3 {
                    break;
                }
            }
            assert_eq!(checked, 3);
        });
    }

    /// Quirk of the shadow pass: a shadow 3 rows above the buffer top still draws 14 rows (buffer rows 0..13), reading
    /// bitmap rows 3..16, past its own 14 rows.
    #[test]
    fn a_shadow_above_the_top_draws_14_rows() {
        test_scene(|c, e, t| {
            let m = vec![0u8; 3136 * 8];
            let sc = Scene { common: c, exe: e, ts: t, sector: 38, c0: 5, r0: 5, cols: 14, rows: 12, tilemap: Some(&m),
                             soldiers: None, cursor: None, lines: Vec::new(), portraits: None, fx: None, shadows: true,
                             roofs: false, dark: None, ground: None };
            for (id, &(bmp, dx, dy)) in e.shadow.iter().enumerate() {
                let Some(&src) = t.offsets.get(RAW_FIRST + bmp as usize) else { continue };
                let row16: Vec<u8> = (0..16).map(|j| t.bank.get(src + 16 * 16 + j).copied().unwrap_or(0)).collect();
                if row16.iter().all(|&v| v == 0) {
                    continue;
                }
                let mut buf = Buf(vec![0x40u8; (PITCH * HEIGHT) as usize]);
                sc.shadow(&mut buf, id, 100 - dx as i32, -3 - dy as i32);
                for (j, &v) in row16.iter().enumerate() {
                    let want = if v == 0 { 0x40 } else if v == 0x17 { t.table28[0x40] } else { t.table0[v as usize] };
                    assert_eq!(buf.0[(13 * PITCH + 100) as usize + j], want, "shadow {id} pixel {j}");
                }
                return;
            }
            panic!("no shadow bitmap with data in row 16");
        });
    }

    /// a tile with t3 = k and a live animation k draws sprite 413 + frame at the tile corner; the same sprite as a
    /// tile object t2 = frame (413 + t2) on a tile without the animation.
    #[test]
    fn an_animated_tile_draws_sprite_413_plus_its_frame() {
        let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
        let (iso, exe) = (root.join("rom/JA1.gog"), root.join("build/game113b/JA.EXE"));
        if !iso.exists() || !exe.exists() {
            return;
        }
        let (iso, exe) = (iso.to_str().unwrap(), exe.to_str().unwrap());
        let mut w = crate::world::World::new_game(iso, exe, 0, 0).unwrap();
        w.cur_sector = 38; // a tactical sector (the new game starts in the office)
        let ts = crate::enter::fill_map(&mut w, iso).unwrap();
        let (c0, r0, k) = (20, 20, 3usize);
        let at = ((r0 + 4) * 56 + c0 + 5) as usize * 8;
        let mut draw = |t2: u8, frame: Option<u8>| {
            (w.tiles[at + 2], w.tiles[at + 3], w.tiles[at + 7]) = (t2, k as u8, 0);
            w.tile_frames = [None; 10];
            w.tile_frames[k] = frame;
            crate::enter::render_world(iso, exe, &mut w, ts, c0, r0, crate::enter::COLS, crate::enter::ROWS).unwrap()
        };
        let none = draw(0, None);
        let mut checked = 0;
        for f in 1..=12u8 {
            let as_object = draw(f, None);
            if as_object == none {
                continue; // t2 = f is not drawn in pass 1 (its class), no reference picture
            }
            assert_eq!(draw(0, Some(f)), as_object, "frame {f}");
            checked += 1;
        }
        assert!(checked >= 3, "{checked}");
    }
}
