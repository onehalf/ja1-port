//! strategic.md "The replay pictures": the island map of the day replay with its flags (green = mercs, red =
//! enemies), drawn for host ops 355 replay_anim and 356 replay_draw. Each picture goes into the replay's timeline
//! as a `ReplayEv::Frame` with its wait; the window shows them in real time.
use std::sync::Arc;

use crate::island::{self, MapData};
use crate::panel;
use crate::play::ReplayEv;

const SW: usize = 320;
const SCREEN: usize = 320 * 200;
/// REPLAY.PCX: 330 x 48.
const FW: usize = 330;
const FH: usize = 48;

/// A 15 x 15 block of the flag sheet (x, y).
type Block = Option<(usize, usize)>;

fn burn_green(j: usize) -> Block { Some((15 * j, 0)) }
fn burn_red(j: usize) -> Block { Some((15 * j, 15)) }
fn rise_green(j: usize) -> Block { Some((15 * j, 30)) }
fn rise_red(j: usize) -> Block { Some((120 + 15 * j, 30)) }
fn wave_green(i: usize) -> Block { Some((240 + 15 * i, 30)) }
fn wave_red(i: usize) -> Block { Some((285 + 15 * i, 30)) }

pub struct ReplayPic {
    map: Arc<MapData>,
    sheet: Arc<panel::PanelData>,
    flags: Vec<u8>,
    pub screen: Vec<u8>,
    clean: Vec<u8>,
    /// The POSTPAN.PCC palette, 8-bit.
    pub palette: [u8; 768],
    /// The same palette, 6-bit.
    pal6: [u8; 768],
    /// The pictures and waits of the replay so far.
    pub out: Vec<ReplayEv>,
    /// The fifth flag frame of the replay (JA1_REPLAY_PNG) and the flag frame count.
    pub shot: Option<Vec<u8>>,
    /// Battle frame 40 of the first battle (JA1_REPLAY_PNG when the replay holds a battle).
    pub battle_shot: Option<Vec<u8>>,
    /// The first frame of the first death picture (JA1_REPLAY_PNG when the replay holds no battle).
    pub death_shot: Option<Vec<u8>>,
    flag_frames: u32,
    /// The screen at the start (the fade in shows it).
    pub start: Vec<u8>,
    /// The world's `replay_carry` at the op 355 call (the march of enemies moves this many).
    pub carry: u8,
    /// The battle frames' scratch picture (pitch 384, 200 rows).
    scratch: Vec<u8>,
    exe: String,
    common: Arc<crate::Common>,
}

const XP: usize = 384;

fn main_file(exe: &str, name: &str) -> Result<Vec<u8>, i32> {
    let dir = std::path::Path::new(exe).parent().map(|p| p.to_path_buf()).unwrap_or_default();
    let main = std::fs::read(dir.join("DAT").join("MAIN.DAT")).map_err(|_| crate::E_FILE)?;
    panel::archive_member(&main, name).map_err(|_| crate::E_FILE)
}

fn main_member(exe: &str, name: &str) -> Result<(usize, usize, Vec<u8>), i32> {
    panel::pcx(&main_file(exe, name)?).map_err(|_| crate::E_FILE)
}

/// ReadPalPalette? 0x53204: the file's last 769 bytes = 0x0C, then 256 x RGB, each shifted right by 2.
fn pcc_palette(file: &[u8]) -> Result<[u8; 768], i32> {
    if file.len() < 769 || file[file.len() - 769] != 0x0C {
        return Err(crate::E_FILE);
    }
    let mut pal = [0u8; 768];
    for (o, v) in pal.iter_mut().zip(&file[file.len() - 768..]) {
        *o = v >> 2;
    }
    Ok(pal)
}

/// Workable trees (strategic.md) of sector s on the table `r` (writes byte 7).
fn workable(r: &mut [u8], s: usize) {
    let b0 = r[s * 9];
    let v = if b0 & 1 != 0 && crate::mappanel::connected_home(r, s as u8) {
        let si = s as i32;
        let owned = |n: i32| r[n as usize * 9] & 1 != 0;
        // (side is the map edge, neighbour, barrier bit): north, east, south, west.
        let sides = [(si < 10, si - 10, 0x02u8), (si % 10 == 9, si + 1, 0x04), (si >= 50, si + 10, 0x08), (si % 10 == 0, si - 1, 0x10)];
        let security = sides.iter().filter(|&&(edge, nb, bit)| edge || owned(nb) || b0 & bit != 0).count() as u32 * 25;
        let trees = r[s * 9 + 6];
        trees.wrapping_sub(r[s * 9 + 8]).min((trees as u32 * security / 100) as u8)
    } else {
        0
    };
    r[s * 9 + 7] = v;
}

/// Set owned (s, 1) on the table `r`: bit 0, then the workable trees of s and its neighbours.
fn set_owned(r: &mut [u8], s: usize) {
    r[s * 9] |= 1;
    workable(r, s);
    let (row, col) = (s / 10, s % 10);
    for (ok, n) in [(row < 5, s + 10), (row > 0, s.wrapping_sub(10)), (col < 9, s + 1), (col > 0, s.wrapping_sub(1))] {
        if ok {
            workable(r, n);
        }
    }
}

/// The POSTPAN.PCC palette, 8-bit: the palette of the whole day-end screen. ReplayScreenInit? 0x64274 sets it, and
/// nothing after it in DayEndScreen? 0x65BD4 sets another one: PayrollScreen 0x44380 and DrawBassMap 0x4420C load
/// BASSMAP.PCC with its palette into a buffer only (0x10AD58), so the payroll, the morning boxes and the Fire
/// question show in this palette (live shot of the original 2026-11-05: blue-grey payroll, not the island map's brown).
pub fn postpan_palette(exe: &str) -> Result<[u8; 768], i32> {
    let post_file = main_file(exe, "POSTPAN.PCC")?;
    Ok(crate::eod::scale_palette(&pcc_palette(&post_file)?))
}

impl ReplayPic {
    /// The replay screen at the start: the map of R (`r` = 60 x 9 bytes, marks 0) over POSTPAN.PCC.
    pub fn new(iso: &str, exe: &str, r: &[u8]) -> Result<ReplayPic, i32> {
        if r.len() < 540 {
            return Err(crate::E_ARG);
        }
        let map = island::map_data(iso, exe)?;
        let sheet = panel::panel_data(iso, exe)?;
        let common = crate::common(iso)?;
        let (fw, fh, flags) = main_member(exe, "REPLAY.PCX")?;
        let (pw, ph, post) = main_member(exe, "POSTPAN.PCC")?;
        let post_file = main_file(exe, "POSTPAN.PCC")?;
        if fw != FW || fh != FH || pw != SW || ph != 54 {
            return Err(crate::E_FILE);
        }
        // ReplayScreenInit? 0x64274: DrawStrategicMap puts MAP.PCC into the clean map, PanelTopToScreen copies rows
        // 0..145 to the screen, DrawMapTable then draws only the cells and markers: the gaps between the cells keep
        // the map's pixels. The palette comes from POSTPAN.PCC, not from MAP.PCC.
        let mut clean = vec![0u8; SCREEN];
        for y in 0..146 {
            clean[y * SW..(y + 1) * SW].copy_from_slice(&map.buf[y * 2 * SW + SW..(y + 1) * 2 * SW]);
        }
        let mut screen = clean.clone();
        for s in 0..60 {
            island::sector(&map, &sheet.sheet, Self::rec(r, s), island::guard_marks()[s], s, &mut screen);
        }
        screen[146 * SW..].copy_from_slice(&post[..54 * SW]);
        clean[146 * SW..].copy_from_slice(&post[..54 * SW]);
        let pal6 = pcc_palette(&post_file)?;
        let palette = crate::eod::scale_palette(&pal6);
        Ok(ReplayPic { map, sheet, flags, clean, start: screen.clone(), screen, palette, pal6, out: Vec::new(), shot: None, battle_shot: None, death_shot: None, flag_frames: 0, carry: 0, scratch: vec![0; XP * 200], exe: exe.to_string(), common })
    }

    /// strategic.md "The day-end accounts": the panel's number fields 31..42 (mappanel::replay_fields) on the start
    /// picture and the screen.
    pub fn panel_numbers(&mut self, acc: &[u8], day: [i32; 6]) {
        for buf in [&mut self.start, &mut self.screen] {
            let mut table0 = self.common.remap[0];
            let mut p = panel::Painter { d: &self.sheet, buf, table0: &mut table0, table29: panel::table29(&self.common) };
            crate::mappanel::replay_fields(&mut p, acc, day);
        }
    }

    /// Put a clean map (320 x 200) in place (a diff test starts from any picture).
    pub fn set_clean(&mut self, c: &[u8]) {
        self.clean.copy_from_slice(c);
    }

    /// The clean map (320 x 200).
    pub fn clean(&self) -> &[u8] {
        &self.clean
    }

    /// The fade in of the replay screen: levels l = 0..24 of the map palette (6-bit colour x l / 24), 8-bit.
    pub fn fade_in(&self) -> Vec<[u8; 768]> {
        (0..=24).map(|l| crate::eod::scale_palette(&crate::sunrise::level(&self.pal6, l, 24))).collect()
    }

    fn rec(r: &[u8], s: usize) -> &[u8] {
        &r[s * 9..s * 9 + 9]
    }

    fn frame(&mut self, wait: u32) {
        self.out.push(ReplayEv::Frame(self.screen.clone()));
        if wait > 0 {
            self.out.push(ReplayEv::Wait(wait));
        }
    }

    /// The area (x, y, w, h) of the clean map with flag blocks at screen places (x, y), onto the screen.
    fn area(&mut self, (ax, ay, aw, ah): (usize, usize, usize, usize), blocks: &[(Block, usize, usize)]) {
        let mut b = vec![0u8; aw * ah];
        for y in 0..ah {
            let a = (ay + y) * SW + ax;
            b[y * aw..y * aw + aw].copy_from_slice(&self.clean[a..a + aw]);
        }
        for &(blk, px, py) in blocks {
            if let Some((sx, sy)) = blk {
                for y in 0..15 {
                    for x in 0..15 {
                        // Pixels outside the area are not shown (a march between sectors that are not neighbours).
                        let p = self.flags[(sy + y) * FW + sx + x];
                        let (qx, qy) = (px + x, py + y);
                        if p != 0 && (ax..ax + aw).contains(&qx) && (ay..ay + ah).contains(&qy) {
                            b[(qy - ay) * aw + qx - ax] = p;
                        }
                    }
                }
            }
        }
        for y in 0..ah {
            let a = (ay + y) * SW + ax;
            self.screen[a..a + aw].copy_from_slice(&b[y * aw..y * aw + aw]);
        }
    }

    /// Flag frame (s, red, green): the box of s from the clean map with the flags, onto the screen; 90 ms.
    fn flag_frame(&mut self, s: usize, red: Block, green: Block) {
        let (bx, by) = ((s % 10) * 31 + 5, (s / 10) * 23 + 3);
        self.area((bx, by, 32, 24), &[(red, bx + 2, by + 3), (green, bx + 14, by + 9)]);
        self.flag_frames += 1;
        if self.flag_frames == 5 {
            self.shot = Some(self.screen.clone());
        }
        self.frame(90);
    }

    fn clean_cell(&mut self, r: &[u8], s: usize) {
        island::sector(&self.map, &self.sheet.sheet, Self::rec(r, s), island::guard_marks()[s], s, &mut self.clean);
    }

    fn copy_cell(&mut self, s: usize) {
        let (cx, cy) = ((s % 10) * 31 + 6, (s / 10) * 23 + 4);
        for y in 0..22 {
            let a = (cy + y) * SW + cx;
            self.screen[a..a + 30].copy_from_slice(&self.clean[a..a + 30]);
        }
    }

    /// Host op 356 replay_draw (k, a) on R.
    pub fn draw(&mut self, k: i32, a: i32, r: &[u8]) {
        let a = a.clamp(0, 59) as usize;
        match k {
            0 => {
                for s in 0..60 {
                    island::sector(&self.map, &self.sheet.sheet, Self::rec(r, s), island::guard_marks()[s], s, &mut self.screen);
                }
                self.frame(0);
            }
            1 => {
                // Original bug (issue #2): the red flag stays on once one sector with mercs had enemies.
                let mut red = false;
                for s in 0..60 {
                    self.clean_cell(r, s);
                    if r[s * 9 + 2] != 0 {
                        red |= r[s * 9 + 1] != 0;
                        self.flag_frame(s, if red { wave_red(1) } else { None }, wave_green(1));
                    }
                }
            }
            2 => {
                for s in (0..60).filter(|&s| s != a && r[s * 9 + 2] == 0) {
                    self.copy_cell(s);
                }
                self.frame(0);
            }
            _ => self.redraw_cell(r, a, true),
        }
    }

    /// replay_draw 3 of sector a; `flags` = 0: always the cell copy (no flag frame).
    fn redraw_cell(&mut self, r: &[u8], a: usize, flags: bool) {
        self.clean_cell(r, a);
        if !flags || r[a * 9 + 2] == 0 {
            self.copy_cell(a);
            self.frame(0);
        } else {
            self.flag_frame(a, if r[a * 9 + 1] != 0 { wave_red(1) } else { None }, wave_green(1));
        }
    }

    /// Host op 355 replay_anim (k, a1 .. a5) with its pictures (k = 0 .. 5; a strip that does not load: the sound
    /// timeline only). `r` = R at the call (the picture's copy).
    pub fn anim(&mut self, k: i32, a: [i32; 5], r: &[u8]) {
        let mut r = r.to_vec();
        let s = a[0].clamp(0, 59) as usize;
        match k {
            0 => {
                let (mode2, whoosh) = (a[1] == 2, a[2] != 0);
                let (mercs, enemies) = (r[s * 9 + 2] != 0, r[s * 9 + 1] != 0);
                let pick = |j: usize, w: usize, rising: bool| -> (Block, Block) {
                    if mode2 {
                        (if rising { rise_red(j) } else { None }, if mercs { wave_green(w) } else { None })
                    } else {
                        (if enemies { wave_red(w) } else { None }, if rising { rise_green(j) } else { None })
                    }
                };
                for n in 0..8 {
                    if whoosh && n == 3 {
                        self.out.push(ReplayEv::Sound("WHOOSH", 88));
                    }
                    let (rd, gr) = pick(if whoosh { n } else { 7 - n }, n % 3, true);
                    self.flag_frame(s, rd, gr);
                }
                let (rd, gr) = if !whoosh {
                    pick(0, 7 % 3, false)
                } else if mode2 {
                    (wave_red(2), if mercs { wave_green(2) } else { None })
                } else {
                    (if enemies { wave_red(2) } else { None }, wave_green(2))
                };
                self.flag_frame(s, rd, gr);
            }
            1 => {
                let (f, t, kind2) = (s, a[1].clamp(0, 59) as usize, a[2] == 2);
                let bx = |s: usize| ((s % 10) * 31 + 5) as i32;
                let by = |s: usize| ((s / 10) * 23 + 3) as i32;
                let (fx, fy, tx, ty) = (bx(f), by(f), bx(t), by(t));
                let (ax, ay, aw, ah, dx, dy, c) = if fx != tx {
                    (fx.min(tx), fy, 64, 24, if fx < tx { 4 } else { -4 }, 0, 6)
                } else if fy < ty {
                    (fx, fy, 32, 48, 0, 3, 4)
                } else if fy > ty {
                    (fx, ty, 32, 48, 0, -3, 7)
                } else {
                    return;
                };
                let (ox, oy) = if kind2 { (2, 3) } else { (14, 9) };
                let stay = !kind2 && r[f * 9 + 1] != 0;
                let there = kind2 && r[t * 9 + 2] != 0;
                let both = there && r[t * 9 + 1] != 0;
                let (mut x, mut y) = (fx + ox, fy + oy);
                let u = |v: i32| v as usize;
                for n in 0..9 {
                    self.out.push(ReplayEv::Sound("MARCH2", 88));
                    if n == c {
                        if kind2 {
                            r[t * 9 + 1] = r[t * 9 + 1].wrapping_add(self.carry);
                            r[f * 9 + 1] = r[f * 9 + 1].wrapping_sub(self.carry);
                        } else {
                            r[t * 9 + 2] = r[f * 9 + 2];
                            r[f * 9 + 2] = 0;
                        }
                        self.redraw_cell(&r, f, kind2);
                        self.redraw_cell(&r, t, kind2);
                    }
                    let w = n as usize % 3;
                    let mover = if kind2 { wave_red(w) } else { wave_green(w) };
                    self.area((u(ax), u(ay), aw, ah), &[
                        (mover, u(x), u(y)),
                        (if stay { wave_red(w) } else { None }, u(fx + 2), u(fy + 3)),
                        (if both { wave_red(w) } else { None }, u(tx + 2), u(ty + 3)),
                        (if there { wave_green(w) } else { None }, u(tx + 14), u(ty + 9)),
                    ]);
                    self.frame(150);
                    x += dx;
                    y += dy;
                }
            }
            3 => {
                let mode2 = a[1] == 2;
                for n in 0..22 {
                    if n == 0 {
                        self.out.push(ReplayEv::Sound("BURN", 88));
                    } else if n == 11 {
                        self.out.push(ReplayEv::Sound("ASHES", 88));
                    }
                    if n == 21 {
                        if mode2 {
                            set_owned(&mut r, s);
                            r[s * 9 + 1] = 0;
                        }
                        self.draw(1, 1, &r);
                        self.draw(2, s as i32, &r);
                    }
                    let (rd, gr) = self.burn_flags(&r, s, mode2, n, true);
                    self.flag_frame(s, rd, gr);
                }
                let (rd, gr) = self.burn_flags(&r, s, mode2, 21, false);
                self.flag_frame(s, rd, gr);
            }
            4 => {
                let (bx, by) = ((s % 10) * 31 + 5, (s / 10) * 23 + 3);
                for t in 0..3 {
                    self.out.push(ReplayEv::Sound("EXPLODE", 104));
                    for (e, wait) in [(114, if t == 0 { 400 } else { 200 }), (115, if t == 2 { 600 } else { 400 })] {
                        self.draw(3, s as i32, &r);
                        self.element(e, bx + 9, by + 9);
                        self.frame(wait);
                    }
                }
                self.draw(3, s as i32, &r);
            }
            2 => {
                let j = a[1];
                if j == 0 {
                    self.out.push(ReplayEv::Sound("BATTLE", 88));
                }
                self.battle_frame(&r, s, j, [a[2], a[3], a[4]]);
                if j == 40 && self.battle_shot.is_none() {
                    self.battle_shot = Some(self.screen.clone());
                }
                self.frame(30);
            }
            5 => {
                if self.death(a[0], a[1].clamp(0, 59) as usize, a[2]).is_err() {
                    self.out.extend(crate::play::replay_steps(k, [a[0], a[1], a[2]]));
                }
            }
            _ => self.out.extend(crate::play::replay_steps(k, [a[0], a[1], a[2]])),
        }
    }

    /// The merc's death picture: the morph strip of picture c (10 frames of 28 x 25) at the box of s.
    fn death(&mut self, c: i32, s: usize, slot: i32) -> Result<(), i32> {
        let dir = std::path::Path::new(&self.exe).parent().map(|p| p.to_path_buf()).unwrap_or_default();
        let arc = std::fs::read(dir.join("DAT").join(if c < 30 { "MF1.DAT" } else { "MF2.DAT" })).map_err(|_| crate::E_FILE)?;
        let m = panel::archive_member(&arc, &format!("MORF{}.PCC", c)).map_err(|_| crate::E_FILE)?;
        let (sw, sh, strip) = panel::pcx(&m).map_err(|_| crate::E_FILE)?;
        if sw < 280 || sh < 25 {
            return Err(crate::E_FILE);
        }
        let face = |i: usize| -> Vec<u8> { (0..25).flat_map(|r| strip[r * sw + 28 * i..r * sw + 28 * i + 28].to_vec()).collect() };
        let (x, y) = ((s % 10) * 31 + 7, (s / 10) * 23 + 3);
        // The background: 30 x 27 at (x - 1, y - 1).
        let bg: Vec<u8> = (0..27).flat_map(|r| self.screen[(y - 1 + r) * SW + x - 1..(y - 1 + r) * SW + x + 29].to_vec()).collect();
        let put = |scr: &mut [u8], pic: &[u8], w: usize, px: usize, py: usize| {
            for r in 0..pic.len() / w {
                scr[(py + r) * SW + px..(py + r) * SW + px + w].copy_from_slice(&pic[r * w..r * w + w]);
            }
        };
        self.out.push(ReplayEv::Fx(0x30 + 4 * slot, 50));
        let zoom = |me: &mut Self, f: &[u8], out: bool| {
            // float32 steps of 28 x 0.1f and 25 x 0.1f, cut to an integer, at least 1.
            let (sw, sh) = (28.0 * 0.1f32, 25.0 * 0.1f32);
            let (mut cw, mut ch) = if out { (28.0f32, 25.0f32) } else { (0.0, 0.0) };
            for _ in 0..10 {
                if out {
                    cw -= sw;
                    ch -= sh;
                } else {
                    cw += sw;
                    ch += sh;
                }
                let (w, h) = ((cw as i32).max(1) as u32, (ch as i32).max(1) as u32);
                let mut pic: Vec<u8> = (0..25).flat_map(|r| bg[(1 + r) * 30 + 1..(1 + r) * 30 + 29].to_vec()).collect();
                let (ox, oy) = ((28 - w as usize) / 2, (25 - h as usize) / 2);
                let (fx, fy) = ((28u32 << 22) / w, (25u32 << 22) / h);
                for j in 0..h {
                    let row = (fy / 2).wrapping_add(j.wrapping_mul(fy)) >> 22;
                    for i in 0..w {
                        let col = (fx / 2).wrapping_add(i.wrapping_mul(fx)) >> 22;
                        pic[(oy + j as usize) * 28 + ox + i as usize] = f[row as usize * 28 + col as usize];
                    }
                }
                put(&mut me.screen, &pic, 28, x, y);
                me.frame(20);
            }
        };
        zoom(self, &face(0), false);
        {
            let mut table0 = self.common.remap[0];
            let mut p = panel::Painter { d: &self.sheet, buf: &mut self.screen, table0: &mut table0, table29: panel::table29(&self.common) };
            p.frame(x as i32 - 1, y as i32 - 1, x as i32 + 28, y as i32 + 25, 0x27);
        }
        for i in 0..10 {
            put(&mut self.screen, &face(i), 28, x, y);
            self.frame(if i == 0 { 450 + 30 } else { 30 });
            if i == 0 && self.death_shot.is_none() {
                self.death_shot = Some(self.screen.clone());
            }
        }
        self.out.push(ReplayEv::Wait(600));
        put(&mut self.screen, &bg, 30, x - 1, y - 1);
        put(&mut self.screen, &face(9), 28, x, y);
        zoom(self, &face(9), true);
        put(&mut self.screen, &bg, 30, x - 1, y - 1);
        self.frame(0);
        Ok(())
    }

    /// Battle frame j of sector s with the bursts b (z | m << 8 | x << 16 | y << 24), onto the screen.
    fn battle_frame(&mut self, r: &[u8], s: usize, j: i32, b: [i32; 3]) {
        let (bx, by) = ((s % 10) * 31 + 5, (s / 10) * 23 + 3);
        if j == 0 {
            island::sector(&self.map, &self.sheet.sheet, Self::rec(r, s), island::guard_marks()[s], s, &mut self.screen);
            for y in 0..24 {
                let a = (by + y) * SW + bx;
                self.scratch[y * XP + 64..y * XP + 96].copy_from_slice(&self.screen[a..a + 32]);
            }
        }
        for y in 0..24 {
            self.scratch.copy_within(y * XP + 64..y * XP + 96, y * XP);
        }
        for v in b {
            let (z, m, x, y) = (v & 0xFF, (v >> 8) & 0xFF, ((v >> 16) & 0xFF) as usize, ((v >> 24) & 0xFF) as usize);
            if z == 2 {
                continue;
            }
            let sx = if z == 1 { 0 } else { 24 } + 3 * m as usize;
            for dy in 0..3 {
                for dx in 0..3 {
                    let p = self.flags.get((45 + dy) * FW + sx + dx).copied().unwrap_or(0);
                    let o = (y + dy) * XP + x + dx;
                    if p != 0 && o < self.scratch.len() {
                        self.scratch[o] = p;
                    }
                }
            }
        }
        island::markers(&self.sheet.sheet, Self::rec(r, s), island::guard_marks()[s], s, &mut self.scratch, XP);
        let w = ((j.max(0) as usize + 1) / 4) % 3;
        let put = |blk: Block, px: usize, py: usize, sc: &mut Vec<u8>, fl: &[u8]| {
            if let Some((sx, sy)) = blk {
                for y in 0..15 {
                    for x in 0..15 {
                        let p = fl[(sy + y) * FW + sx + x];
                        if p != 0 {
                            sc[(py + y) * XP + px + x] = p;
                        }
                    }
                }
            }
        };
        put(wave_red(w), 2, 3, &mut self.scratch, &self.flags);
        if r[s * 9 + 2] != 0 {
            put(wave_green(w), 14, 9, &mut self.scratch, &self.flags);
        }
        for y in 0..24 {
            let a = (by + y) * SW + bx;
            self.screen[a..a + 32].copy_from_slice(&self.scratch[y * XP..y * XP + 32]);
        }
    }

    fn burn_flags(&self, r: &[u8], s: usize, mode2: bool, n: usize, burning: bool) -> (Block, Block) {
        let w = n % 3;
        if mode2 {
            (if burning { burn_red(n) } else { None }, if r[s * 9 + 2] != 0 { wave_green(w) } else { None })
        } else {
            (if r[s * 9 + 1] != 0 { wave_red(w) } else { None }, if burning { burn_green(n) } else { None })
        }
    }

    /// Panel element e at (x, y) on the screen; sheet pixel 0 is not written.
    fn element(&mut self, e: usize, x: usize, y: usize) {
        let (sx, sy, w, h) = panel::element(e);
        for row in 0..h as usize {
            for c in 0..w as usize {
                let p = self.sheet.sheet[(sy as usize + row) * SW + sx as usize + c];
                let o = (y + row) * SW + x + c;
                if p != 0 && o < SCREEN {
                    self.screen[o] = p;
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Ledger row ja1front:5688 (ReadPalPalette? 0x53204): a file of exactly 769 bytes (0x0C + 256 RGB) is a palette.
    #[test]
    fn a_769_byte_file_is_a_palette() {
        let mut f = vec![0x0Cu8];
        f.extend((0..768).map(|i| (i % 256) as u8));
        let pal = pcc_palette(&f).unwrap();
        assert_eq!((pal[0], pal[767]), (0, 63));
        assert!(pcc_palette(&f[1..]).is_err());
    }

    /// Without the pictures the timeline of k = 0, 3, 4 is the sound timeline of `replay_steps` (sector with no
    /// mercs: no flag frames from replay_draw inside the animation).
    #[test]
    fn same_sounds_and_waits() {
        let iso = concat!(env!("CARGO_MANIFEST_DIR"), "/../../rom/JA1.gog");
        let exe = concat!(env!("CARGO_MANIFEST_DIR"), "/../../rom/JA.EXE");
        let r = vec![0u8; 540];
        let Ok(mut p) = ReplayPic::new(iso, exe, &r) else { return };
        // (1, [5, 5, ..]): ReplayMarchAnim 0x64D20 returns at once when from = to (strategic.md "k = 1"): no sound.
        for (k, a) in [(1, [5, 5, 1, 0, 0]), (1, [5, 6, 1, 0, 0]), (1, [6, 5, 1, 0, 0]), (1, [15, 5, 1, 0, 0]), (1, [5, 15, 1, 0, 0]), (1, [35, 5, 1, 0, 0]), (0, [5, 2, 1, 0, 0]), (0, [5, 1, 0, 0, 0]), (3, [7, 2, 0, 0, 0]), (3, [7, 1, 0, 0, 0]), (4, [9, 0, 0, 0, 0]), (2, [5, 0, 0x0505_0001, 0x1410_0300, 2]), (2, [5, 7, 2, 2, 0x0303_0000]), (5, [3, 12, 1, 0, 0]), (5, [41, 59, 4, 0, 0])] {
            p.out.clear();
            p.anim(k, a, &r);
            let merge = |v: Vec<ReplayEv>| {
                let mut got: Vec<ReplayEv> = Vec::new();
                for e in v {
                    match (e, got.last_mut()) {
                        (ReplayEv::Frame(_), _) => {}
                        (ReplayEv::Wait(b), Some(ReplayEv::Wait(a))) => *a += b,
                        (e, _) => got.push(e),
                    }
                }
                got
            };
            assert!(k == 1 || p.out.iter().any(|e| matches!(e, ReplayEv::Frame(_))), "k {} {:?}: no pictures", k, a);
            assert_eq!(merge(std::mem::take(&mut p.out)), merge(crate::play::replay_steps(k, [a[0], a[1], a[2]])), "k {} {:?}", k, a);
        }
    }
}
