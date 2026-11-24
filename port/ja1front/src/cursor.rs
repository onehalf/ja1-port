//! The tactical tile cursor (docs/spec/cursor.md).
use crate::panel::Font;
use crate::render::PITCH;
use crate::sprite::Px;
use crate::host::{Ja1Host, Ja1World};

/// cursor.md "Cursor state" (C layout: the test interface passes it as it is).
#[repr(C)]
#[derive(Clone, Copy, Debug)]
pub struct CursorState {
    pub over_map: u8,
    pub tactical_on: u8,
    pub overlay_up: u8,
    pub value: u8,
    pub mode: u8,
    pub out_of_range: u8,
    pub combat: u8,
    pub ap_fg: u8,
    pub ap_shadow: u8,
    pub blink_phase: u8,
    pub no_blink: u8,
    pub under_soldier: u8,
    pub wobble_dir: i8,
    pub pad: [u8; 3],
    pub tile: i32,
    pub ap: i32,
    pub blink_timer: i32,
    pub wobble: i32,
    pub wobble_dx: i16,
    pub wobble_dy: i16,
    pub label_x: i16,
    pub label_y: i16,
    pub label: [u8; 16],
    /// cursor.md "Tile marker".
    pub marker_kind: i32,
    pub marker_tile: i32,
    pub marker_life: i32,
    pub marker_blink: i32,
    pub marker_phase: u8,
    pub pad2: [u8; 3],
    /// cursor.md "Soldier and tile marks": the world's shown_slot, spotted, view_tile (copied in before a draw).
    pub shown_slot: i32,
    pub spotted: i32,
    pub view_tile: i32,
    pub mark_timer: i32,
    pub mark_phase: i8,
    pub pad3: [u8; 3],
}

/// ClearMarksBeforeQuote? 0x11418 after its HidePathMarks(0) (library, op 348): AP number 0x229DEC = 0, cursor tile
/// 0x229D2C = 0xC41, mouse wobble 0x22A47E / 0x22A47C = 0, over_map 0x284F13 = 0.
pub fn clear_marks_before_quote(c: &mut CursorState) {
    c.over_map = 0;
    c.wobble_dx = 0;
    c.wobble_dy = 0;
    c.ap = 0;
    c.tile = 0xC41;
}

impl Default for CursorState {
    fn default() -> CursorState {
        CursorState {
            over_map: 0,
            tactical_on: 1,
            overlay_up: 0,
            value: 0x8E,
            mode: 1,
            out_of_range: 0,
            combat: 0,
            ap_fg: 0xFF,
            ap_shadow: 0xFE,
            blink_phase: 0,
            no_blink: 0,
            under_soldier: 0,
            wobble_dir: 0,
            pad: [0; 3],
            tile: 0,
            ap: 0,
            blink_timer: 200,
            wobble: -1000,
            wobble_dx: 0,
            wobble_dy: 0,
            label_x: -1,
            label_y: 0,
            label: [0; 16],
            marker_kind: 0,
            marker_tile: 0xC41,
            marker_life: 0,
            marker_blink: 200,
            marker_phase: 0,
            pad2: [0; 3],
            shown_slot: 0x40,
            spotted: 0x40,
            view_tile: 0xC41,
            mark_timer: 100,
            mark_phase: 0,
            pad3: [0; 3],
        }
    }
}

/// The cursor table of JA.EXE (182 pairs: picture in range, out of range).
pub const TABLE_FILE_OFF: usize = 0xDC4FE;
pub const TABLE_LEN: usize = 182 * 2;

/// What the two drawing steps need besides the state.
pub struct CursorEnv<'a> {
    pub table: &'a [u8],
    pub font0: &'a Font,
    pub table0: [u8; 256],
    pub view_x: i32,
    pub view_y: i32,
    pub view_w: i32,
    pub view_h: i32,
}

fn put(buf: &mut [u8], x: i32, y: i32, v: u8) {
    let off = y as i64 * PITCH as i64 + x as i64;
    if off >= 0 && (off as usize) < buf.len() {
        buf[off as usize] = v;
    }
}

/// panel.md "Text" into the back buffer: clip width 384 (cursor.md "Text"), mode 300.
#[allow(clippy::too_many_arguments)]
pub fn text(buf: &mut [u8], f: &Font, table0: &mut [u8; 256], s: &[u8], mut x: i32, mut y: i32, centred: bool,
            fg: u8, shadow: u8, font0: bool) {
    let s = &s[..s.iter().position(|&b| b == 0).unwrap_or(s.len())];
    if centred {
        let sum: i32 = s
            .iter()
            .map(|&b| {
                // SPEC GAP: the width sum for a byte outside 0x20..0xA2 is not described; counted as 0.
                let c = b as i32 - 0x20;
                if (0..131).contains(&c) {
                    f.widths[c as usize] as i32
                } else {
                    0
                }
            })
            .sum();
        x -= sum / 2;
    }
    if font0 {
        y -= 1;
    }
    let (s_ff, s_fe) = (table0[0xFF], table0[0xFE]);
    table0[0xFF] = fg;
    table0[0xFE] = shadow;
    for &b in s {
        let c = b as i32 - 0x20;
        if !(0..131).contains(&c) {
            return; // the two table bytes stay changed
        }
        let mut w = f.widths[c as usize] as i32;
        if (x as u32).wrapping_add(w as u32) > PITCH as u32 {
            w = PITCH - x;
        }
        let gx = f.x_of(c as usize) as i32;
        for j in 0..f.height as i32 {
            for i in 0..w {
                // SPEC GAP: glyph reads past the font sheet give 0.
                let off = j as i64 * f.pitch as i64 + (gx + i) as i64;
                let p = if off >= 0 { f.sheet.get(off as usize).copied().unwrap_or(0) } else { 0 };
                if shadow != 0 {
                    if p != 0 {
                        put(buf, x + i, y + j, table0[p as usize]);
                    }
                } else {
                    let v = table0[p as usize];
                    if v != 0 {
                        put(buf, x + i, y + j, v);
                    }
                }
            }
        }
        x += w;
        if x > PITCH - 1 {
            break;
        }
    }
    table0[0xFF] = s_ff;
    table0[0xFE] = s_fe;
}

/// ApNumberText? 0x1D644: the AP number as decimal text (the original itoa's it into 0x229DE0; the +4
/// when aiming is possible is added where the number is worked out, 0x1D690).
fn ap_text(ap: i32) -> Vec<u8> {
    ap.to_string().into_bytes()
}

fn draw_sprite(buf: &mut [u8], sprites: &[Vec<Px>], n: usize, x: i32, y: i32) {
    if let Some(px) = sprites.get(n) {
        for p in px {
            put(buf, x + p.col, y + p.row, p.val);
        }
    }
}

/// cursor.md "Cursor sprite" with offset (dx, dy).
pub fn cursor_sprite(buf: &mut [u8], sprites: &[Vec<Px>], env: &mut CursorEnv, st: &mut CursorState, dx: i16, dy: i16) {
    let x = 16 * (st.tile % 56) - env.view_x + 32;
    let y = 14 * (st.tile / 56) - env.view_y + 28;
    if !(x > 16 && y > 14 && x < env.view_w + 32 && y < env.view_h + 28) {
        return;
    }
    if st.blink_timer <= 0 {
        st.blink_phase ^= 1;
        st.blink_timer = 200;
    }
    let (fg, sh) = if st.no_blink == 0 && st.blink_phase != 0 { (0xFF, 0xFE) } else { (st.ap_fg, st.ap_shadow) };
    let red = st.out_of_range != 0 && st.blink_phase != 0;
    let mode = st.mode as i32;
    if st.wobble <= 0 && !red {
        // SPEC GAP: a value past the table (> 181) is not described; picture 0 then.
        let k = st.value as usize * 2 + (st.out_of_range != 0) as usize;
        let p = env.table.get(k).copied().unwrap_or(0);
        let (sx, sy) = (x + dx as i32, y + dy as i32);
        draw_sprite(buf, sprites, 413 + p as usize, sx, sy);
        if st.ap != 0 && st.combat != 0 {
            text(buf, env.font0, &mut env.table0, &ap_text(st.ap), sx + 9 - mode, sy + 4, true, fg, sh, true);
        }
    } else {
        let (mut p, mut ox, mut oy) = (st.value, dx, dy);
        if st.wobble > 0 {
            p = 0xA4;
            (ox, oy) = if st.wobble_dir == -2 { (-4, 0) } else { (0, -4) };
            (st.wobble_dx, st.wobble_dy) = (ox, oy);
        }
        let (sx, sy) = (x + ox as i32, y + oy as i32);
        draw_sprite(buf, sprites, 413 + p as usize, sx, sy);
        if st.ap != 0 && st.combat != 0 && p != 0xA4 {
            text(buf, env.font0, &mut env.table0, &ap_text(st.ap), sx + 9 - mode, sy + 4, true, fg, sh, true);
        }
    }
}

/// cursor.md "Target mark"; `occupant` = tile byte 5 of the mouse tile.
pub fn target_mark(buf: &mut [u8], sprites: &[Vec<Px>], env: &mut CursorEnv, st: &mut CursorState, occupant: u8) {
    if st.over_map == 0 || st.tactical_on == 0 {
        return;
    }
    if !((0x8E..=0x91).contains(&st.value) || st.value == 0xA4) {
        return;
    }
    if occupant & 0x7F < 0x30 {
        cursor_sprite(buf, sprites, env, st, 0, 0);
    } else {
        st.under_soldier = 1;
    }
}

/// cursor.md "Cursor pass": steps 1 and 2 (over the map), the soldier and tile marks (with the soldier records and
/// the tile map in `marks`), then the tile marker.
pub fn cursor_pass(buf: &mut [u8], sprites: &[Vec<Px>], env: &mut CursorEnv, st: &mut CursorState,
                   marks: Option<(&[u8], &[u8])>) {
    if st.tactical_on == 0 || st.overlay_up != 0 {
        return;
    }
    if st.over_map != 0 {
        cursor_steps(buf, sprites, env, st);
    }
    if let Some((recs, tm)) = marks {
        for slot in [st.shown_slot, st.spotted] {
            if slot < 0x40 {
                soldier_mark(buf, sprites, env, st, recs, slot);
            }
        }
        if st.view_tile != 0xC41 {
            tile_mark(buf, sprites, env, st, tm);
        }
    }
    if st.marker_tile != 0xC41 {
        tile_marker(buf, sprites, env, st);
    }
}

/// cursor.md "Tile marker": Set (op 130, the ALL-move).
pub fn marker_set(st: &mut CursorState, kind: i32, tile: i32) {
    st.marker_kind = kind;
    st.marker_tile = tile;
    st.marker_blink = 200;
    st.marker_life = 1000;
}

/// cursor.md "Tile marker": one timer tick; `lock2_open` = round_lock2 = 0, `open` = that and round_lock = 0.
pub fn marker_tick(st: &mut CursorState, lock2_open: bool, open: bool) {
    if lock2_open && st.marker_tile != 0xC41 {
        st.marker_life = st.marker_life.wrapping_sub(10);
        if st.marker_life <= 0 {
            st.marker_tile = 0xC41;
        }
    }
    if open {
        st.marker_blink = st.marker_blink.wrapping_sub(10);
    }
}

/// cursor.md "Soldier and tile marks", step 1 for soldier `slot`.
fn soldier_mark(buf: &mut [u8], sprites: &[Vec<Px>], env: &CursorEnv, st: &mut CursorState, recs: &[u8], slot: i32) {
    if !(0..0x30).contains(&slot) {
        return; // the SPEC GAP of a slot past the 48 records: no mark
    }
    let o = slot as usize * 512;
    let w = |k: usize| i16::from_le_bytes([recs[o + k], recs[o + k + 1]]) as i32;
    if w(0x114) == st.tile {
        return;
    }
    let (x, y) = (w(0x116) - env.view_x + 22, w(0x118) - env.view_y + 19);
    mark_steps(buf, sprites, env, st, if slot >= 0x28 { 0xDC } else { 0xBB }, x, y);
}

/// cursor.md "Soldier and tile marks", step 1 for the tile view_tile.
fn tile_mark(buf: &mut [u8], sprites: &[Vec<Px>], env: &CursorEnv, st: &mut CursorState, tm: &[u8]) {
    let t = st.view_tile;
    let (mut x, mut y) = (16 * (t % 56) - env.view_x + 32, 14 * (t / 56) - env.view_y + 28);
    let obj = if (0..3136).contains(&t) { tm[t as usize * 8 + 1] } else { 0 };
    if obj != 0 {
        x -= 3;
        y -= 3;
    }
    mark_steps(buf, sprites, env, st, 0xBB, x, y);
}

/// cursor.md "Soldier and tile marks", steps 2 and 3.
fn mark_steps(buf: &mut [u8], sprites: &[Vec<Px>], env: &CursorEnv, st: &mut CursorState, base: i32, x: i32, y: i32) {
    if x > 16 && y > 14 && x < env.view_w + 32 && y < env.view_h + 28 {
        let n = 413 + base + st.mark_phase as i32;
        draw_sprite(buf, sprites, n as usize, x, y);
    }
    if st.mark_timer <= 0 {
        st.mark_phase = st.mark_phase.wrapping_add(1);
        if st.mark_phase > 4 {
            st.mark_phase = 0;
        }
        st.mark_timer = 100;
    }
}

/// cursor.md "Soldier and tile marks" and "Cursor state": one timer tick of the blink and mark clocks
/// (`lock2_open` = round_lock2 = 0).
pub fn clocks_tick(st: &mut CursorState, lock2_open: bool) {
    if lock2_open {
        st.blink_timer = st.blink_timer.wrapping_sub(10);
        st.mark_timer = st.mark_timer.wrapping_sub(10);
    }
}

/// cursor.md "Tile marker": Drawing.
fn tile_marker(buf: &mut [u8], sprites: &[Vec<Px>], env: &mut CursorEnv, st: &mut CursorState) {
    let x = 16 * (st.marker_tile % 56) - env.view_x + 32;
    let y = 14 * (st.marker_tile / 56) - env.view_y + 28;
    if st.marker_blink <= 0 {
        st.marker_phase ^= 1;
        st.marker_blink = 200;
    }
    if st.marker_phase != 0 && x > 16 && y > 14 && x < env.view_w + 32 && y < env.view_h + 28 {
        // SPEC GAP: a kind past the sprite bank is not described; nothing is drawn then.
        if (0..0x10000).contains(&st.marker_kind) {
            draw_sprite(buf, sprites, 413 + st.marker_kind as usize, x, y);
        }
    }
}

/// cursor.md "Cursor pass", steps 1 and 2.
fn cursor_steps(buf: &mut [u8], sprites: &[Vec<Px>], env: &mut CursorEnv, st: &mut CursorState) {
    if st.under_soldier == 0 {
        if !(0x8E..=0x91).contains(&st.value) {
            cursor_sprite(buf, sprites, env, st, -2, -2);
        }
    } else if st.value == 0xA4 {
        let (dx, dy) = (st.wobble_dx, st.wobble_dy);
        cursor_sprite(buf, sprites, env, st, dx, dy);
    } else {
        cursor_sprite(buf, sprites, env, st, 0, 0);
    }
    if st.label_x >= 0 {
        let label = st.label;
        let (lx, ly) = (st.label_x as i32, st.label_y as i32);
        text(buf, env.font0, &mut env.table0, &label, lx, ly, true, 0xFF, 1, true);
    }
}

/// cursor.md "Hover": the hover state (C layout: the test interface passes it as it is).
#[repr(C)]
#[derive(Clone, Copy, Debug)]
pub struct HoverState {
    pub ap_timer: i32,
    pub item_timer: i32,
    pub light_timer: i32,
    pub target: i32,
    pub lit: u8,
    pub named: u8,
    pub light_wait: u8,
    pub redraw_off: u8,
}

impl Default for HoverState {
    fn default() -> HoverState {
        HoverState { ap_timer: 0, item_timer: 0, light_timer: 300, target: 0x40, lit: 0x40, named: 0x40, light_wait: 0, redraw_off: 0 }
    }
}

/// Original TacticalKeyHandler helper 0x1D2BC: repair only the cursor modes that depend on the selected soldier.
pub fn dead_cursor_fix(value: u8, hp: u8, mode: u8, profile: u8) -> u8 {
    if !matches!(value, 0x91 | 0xB5 | 0xA4) {
        value
    } else if hp != 0 {
        0xA1
    } else if mode == 1 {
        0x8E
    } else if profile < 8 {
        0x8C
    } else {
        value
    }
}

#[no_mangle]
pub extern "C" fn ja1f_cursor_dead_fix(value: i32, hp: i32, mode: i32, profile: i32) -> i32 {
    dead_cursor_fix(value as u8, hp as u8, mode as u8, profile as u8) as i32
}

/// TacticalKeyHandler 0x13490, the mouse outside the map window (asm 0x14308..0x14358): in the frame after the
/// mouse left the map (over_map 0x284F13 was 1 from the last hover step) the original
///   1. unlights the lit portrait (Unlight 0x33570, when lit 0x284F0E < 0x40),
///   2. sets the name 0x95E0C = 0x40,
///   3. hides the path marks (HidePathMarks 0x1D8B8(0): AP number 0x229DEC = 0, cursor tile 0x229D2C = 0xC41),
///   4. zeroes the mouse wobble 0x22A47E / 0x22A47C,
///   5. clears the cursor tile 0x229D2C = 0xC41,
///   6. sets over_map 0x284F13 = 0,
/// and then runs CursorStateAfterKey 0x1D2BC on g_soldiers[lit]: past the 48 records that pointer is not a record,
/// its hp byte (address 0x183 of the zero page) is non-zero in the loaded image, so a 0x91 / 0xB5 / 0xA4 cursor
/// becomes 0xA1 (over_map 0: not drawn). The name and the lit face frame (panel.md "Drawing one slot" step 8) are
/// drawn from lit / named, so both go in the same frame. The animated tile marker 0x229CAC keeps its life.
pub fn off_map(st: &mut CursorState, hs: &mut HoverState, w: *mut Ja1World, h: *const Ja1Host) {
    if st.over_map == 0 {
        return;
    }
    if hs.lit < 0x40 {
        st.label[0] = 0;
        if hs.lit < 8 {
            hs.lit = 0x40;
        }
    }
    hs.named = 0x40;
    st.ap = 0;
    st.tile = 0xC41;
    // SAFETY: the caller passes a valid world and host for this step.
    unsafe { ja1_hide_path_marks(w, h, 0) };
    st.wobble_dx = 0;
    st.wobble_dy = 0;
    st.over_map = 0;
    st.value = dead_cursor_fix(st.value, 1, st.mode, 0);
    crate::enter::label_slots(hs.lit, hs.named);
}

extern "C" {
    fn ja1_hide_path_marks(w: *mut Ja1World, h: *const Ja1Host, force: i32);
    fn ja1_action_ap(w: *mut Ja1World, h: *const Ja1Host, slot: i32, tile: i32, flag: i32) -> i32;
    fn ja1_path_ap_marks(w: *mut Ja1World, h: *const Ja1Host, slot: i32, tile: i32, mark: i32, extra: i32, sneak: i32,
                         byte177: i32, alt: i32) -> i32;
    fn ja1_use_cost(w: *mut Ja1World, h: *const Ja1Host, slot: i32, spot: i32, tile: i32, g: i32, kind: i32) -> i32;
    fn ja1_on_screen(w: *mut Ja1World, h: *const Ja1Host, slot: i32) -> i32;
}

/// OnScreen 0x1FC34 of merc k (SlotMouse 0x118EA asks it before "cont" shows).
pub fn slot_on_screen(w: *mut Ja1World, h: *const Ja1Host, k: u8) -> bool {
    // SAFETY: the caller passes a valid world and host.
    k < 8 && unsafe { ja1_on_screen(w, h, k as i32) } != 0
}

const REC: usize = 512;
/// The table 0x964EC, 10 bytes per word. SetSpeakerName has no range test: the word number is the low byte of hp / 15, so
/// hp 105..119 and 120..127 read the next table (FAINT, DEFINITE), and hp -15..-128 (numbers 248..255) reads into the item
/// name table at 0x96E9C (bytes of the exe, cut at their 10-byte steps). No game path is known to write such an hp.
fn health_word(k: usize) -> &'static [u8] {
    const WORDS: [&[u8]; 9] = [b"DYING", b"CRITICAL", b"WOUNDED", b"POOR", b"HEALTHY", b"STRONG", b"EXCELLENT", b"FAINT", b"DEFINITE"];
    const PAST: [&[u8]; 8] = [b"   ", b" Ammo   ", b"etonator     ", b"   ", b"n       ", b"niper Scope  ", b"e  ", b"Of Steel"];
    if k >= 248 { PAST[k - 248] } else { WORDS[k] }
}

fn rec_byte(wd: &Ja1World, r: usize, off: usize) -> u8 {
    if r >= 48 || wd.soldiers.is_null() {
        return 0;
    }
    // SAFETY: soldiers = 48 records of 512 bytes.
    unsafe { *wd.soldiers.add(r * REC + off) }
}

/// The animation word +0x1E8 + 2 * [+0x1F3] of soldier r (0x1C3D1 `mov ax, [ebx + eax*2 + 0x1e8]`): the
/// original has no bound, so an index of 12 or more reads into the next soldier records.
/// SPEC GAP: a word past the 48 records reads 0.
fn anim_word_of(wd: &Ja1World, r: usize) -> u16 {
    let i = r * REC + 0x1E8 + 2 * rec_byte(wd, r, 0x1F3) as usize;
    if r >= 48 || wd.soldiers.is_null() || i + 1 >= 48 * REC {
        return 0;
    }
    // SAFETY: i + 1 is inside the 48 records of 512 bytes.
    unsafe { *wd.soldiers.add(i) as u16 | (*wd.soldiers.add(i + 1) as u16) << 8 }
}

fn item_byte(wd: &Ja1World, ty: u8, k: usize) -> u8 {
    if wd.item_types.is_null() {
        return 0;
    }
    // SAFETY: item types = 256 entries of 12 bytes.
    unsafe { *wd.item_types.add(12 * ty as usize + k) }
}

/// WeaponCursor? 0x1C39C (EAX soldier): cursor 0x284F1D = the hand item's picture when the soldier can use it
/// (hp 15+, not in water, the animation word +0x1E8 + 2 * [+0x1F3] above 3, and the status word +2 above 9 or an
/// item without the kinds 7 and 9); else 0xA1.
pub fn weapon_cursor_state(st: &mut CursorState, w: *mut Ja1World, s: usize) {
    if w.is_null() {
        st.value = 0xA1;
        return;
    }
    // SAFETY: the caller passes a valid world.
    let wd = unsafe { &*w };
    let (g, o) = (rec_byte(wd, s, 0x1B2), rec_byte(wd, s, 0x1B4));
    let water = g >= 0x6C && o != 0x1C && o != 0x1F;
    let anim = anim_word_of(wd, s);
    let ty = rec_byte(wd, s, 0);
    if rec_byte(wd, s, 0x183) as i8 as i32 > 14 && !water && anim > 3
        && (rec_byte(wd, s, 2) as i8 as i32 > 9 || (item_byte(wd, ty, 7) == 0 && item_byte(wd, ty, 9) == 0))
    {
        st.value = item_byte(wd, ty, 2);
    } else {
        st.value = 0xA1;
    }
}

/// MoveCursorState? 0x1C464 (EAX soldier): HidePathMarks(0), then the cursor 0x284F1D by the mode 0x284F1B:
/// 0 -> hp (0xA1, mode 1) / team slot (0x8C), 1 -> hp 15+ (0x8E) / hp (0xA1) / 0x8C with mode 0,
/// 2 -> WeaponCursor, 3 -> 0x94.
pub fn move_cursor_state(st: &mut CursorState, w: *mut Ja1World, h: *const Ja1Host, s: usize) {
    st.ap = 0;
    st.tile = 0xC41;
    // SAFETY: the caller passes a valid world and host.
    unsafe { ja1_hide_path_marks(w, h, 0) }
    if w.is_null() {
        return;
    }
    // SAFETY: as above.
    let wd = unsafe { &*w };
    let hp = rec_byte(wd, s, 0x183) as i8 as i32;
    match st.mode {
        0 => {
            if hp != 0 {
                st.value = 0xA1;
                st.mode = 1;
            } else if rec_byte(wd, s, 0x17A) < 8 {
                st.value = 0x8C;
            }
        }
        1 => {
            if hp > 14 {
                st.value = 0x8E;
            } else if hp != 0 {
                st.value = 0xA1;
            } else {
                st.mode = 0;
                st.value = 0x8C;
            }
        }
        2 => weapon_cursor_state(st, w, s),
        3 => st.value = 0x94,
        _ => {}
    }
}

/// The key latches of TacticalKeyHandler 0x1409B..0x141A8: Alt 0x284F20, Ctrl 0x284F1F, Shift 0x284F21, and the cursor
/// mode 0x284F22 kept while Ctrl holds the hand cursor.
#[derive(Clone, Copy, Default, Debug, PartialEq, Eq)]
pub struct KeyLatches {
    pub alt: bool,
    pub ctrl: bool,
    pub shift: bool,
    pub saved_mode: u8,
}

/// What 0x1409B..0x141A8 calls for the Ctrl key.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum CtrlCall {
    None,
    /// MoveCursorState? 0x1C464 with the mode 3 stored.
    Hand,
    /// CursorModeSet? 0x1C438 with this mode.
    Back(u8),
}

/// TacticalKeyHandler 0x1409B..0x141A8, `flags` = keyboard flags 0x22A46C. A change of Alt or Shift clears the cursor tile, so
/// the next mouse step works out the cursor again. Holding Ctrl (0x104) stores the mode and sets mode 3, the hand cursor.
/// The release sets the stored mode, only when the mode is still 3.
pub fn key_flags_pass(st: &mut CursorState, l: &mut KeyLatches, flags: u16) -> CtrlCall {
    if (flags & 0x208 != 0) != l.alt {
        l.alt = !l.alt;
        st.tile = 0xC41;
    }
    let mut call = CtrlCall::None;
    if flags & 0x104 != 0 {
        if !l.ctrl {
            l.ctrl = true;
            l.saved_mode = st.mode;
            st.mode = 3;
            call = CtrlCall::Hand;
        }
    } else if l.ctrl {
        l.ctrl = false;
        if st.mode == 3 {
            call = CtrlCall::Back(l.saved_mode);
        }
    }
    if (flags & 3 != 0) != l.shift {
        l.shift = !l.shift;
        st.tile = 0xC41;
    }
    call
}

/// CursorModeSet? 0x1C438 (EAX soldier, DL mode; host op 185 click_panel of TacticalClick, SoldierDies and
/// StoryFind): store the mode 0x284F1B (a mode above 2 becomes 1), then run MoveCursorState 0x1C464 of that
/// soldier. The front-end's `slot` takes the place of the soldier pointer.
pub fn click_panel(st: &mut CursorState, w: *mut Ja1World, h: *const Ja1Host, slot: i32, mode: i32) {
    st.mode = mode as u8;
    if st.mode > 2 {
        st.mode = 1;
    }
    if (0..48).contains(&slot) {
        move_cursor_state(st, w, h, slot as usize);
    }
}

/// One hover step (cursor.md "Hover") at mouse tile `t`.
pub struct Hover<'a> {
    pub w: *mut Ja1World,
    pub h: *const Ja1Host,
    pub st: &'a mut CursorState,
    pub hs: &'a mut HoverState,
    /// gv_t3 (tactical_map.md).
    pub gv_t3: &'a [u8; 256],
    pub shift: bool,
    pub alt: bool,
    /// The confirm-move option (save byte 21552): a walk needs a second click on the same tile.
    pub confirm_move: bool,
    /// Control Panel switch 13 Fast Firing: a weapon click fires at once, with no aim step (cursor.md Mode 2 step 5).
    pub fast_fire: bool,
    /// cursor.md "Drag to an edge": the edge (10..13) of the left press's drag at its release; 0 = no drag.
    pub drag: i32,
    /// cursor.md "ALL-move": 1 = the walk's move order moves the squad (the right button came during the drag).
    pub squad: i32,
}

impl Drop for Hover<'_> {
    /// tactical_map.md "Soldier texts" read the lit and the named slot of the last hover step.
    fn drop(&mut self) {
        crate::enter::label_slots(self.hs.lit, self.hs.named);
    }
}

impl<'a> Hover<'a> {
    fn wd(&self) -> &Ja1World {
        // SAFETY: the caller gives a valid world for the step's life.
        unsafe { &*self.w }
    }
    fn rb(&self, r: usize, off: usize) -> u8 {
        if r >= 48 || self.wd().soldiers.is_null() {
            return 0;
        }
        // SAFETY: soldiers = 48 records of 512 bytes.
        unsafe { *self.wd().soldiers.add(r * REC + off) }
    }
    fn rs(&self, r: usize, off: usize) -> i32 {
        self.rb(r, off) as i8 as i32
    }
    fn tb(&self, t: i32, k: usize) -> u8 {
        if !(0..3136).contains(&t) || self.wd().tiles.is_null() {
            // SPEC GAP: tile bytes off the map are not described; read as 0.
            return 0;
        }
        // SAFETY: tiles = 3136 x 8 bytes.
        unsafe { *self.wd().tiles.add(t as usize * 8 + k) }
    }
    fn it(&self, ty: u8, k: usize) -> u8 {
        if self.wd().item_types.is_null() {
            return 0;
        }
        // SAFETY: item types = 256 entries of 12 bytes.
        unsafe { *self.wd().item_types.add(12 * ty as usize + k) }
    }
    fn sel(&self) -> usize {
        self.wd().selected as usize
    }
    fn s_hp(&self) -> i32 {
        self.rs(self.sel(), 0x183)
    }
    fn hand_kind(&self) -> u8 {
        self.it(self.rb(self.sel(), 0), 2)
    }
    fn ground_item(&self, t: i32) -> bool {
        let i = self.tb(t, 6) as usize;
        if i >= 200 || self.wd().ground_items.is_null() {
            return false;
        }
        // SAFETY: ground items = 200 entries of 12 bytes.
        let (ty, b6) = unsafe { (*self.wd().ground_items.add(12 * i), *self.wd().ground_items.add(12 * i + 6)) };
        ty != 0 && (b6 as i8) > 0
    }
    fn set_label(&mut self, s: &[u8]) {
        let n = s.iter().position(|&b| b == 0).unwrap_or(s.len()).min(15);
        self.st.label = [0; 16];
        self.st.label[..n].copy_from_slice(&s[..n]);
    }

    fn hide_marks(&mut self) {
        self.st.ap = 0;
        self.st.tile = 0xC41;
        // SAFETY: valid world and host.
        unsafe { ja1_hide_path_marks(self.w, self.h, 0) }
    }
    /// Original 0x33570 Unlight: label 0x22A444 = byte 0x907E4 (0) and, for a slot below 8, lit 0x284F0E = 0x40;
    /// the slot and portrait draws after that are drawing only (stubbed in hover_diff).
    fn unlight(&mut self, p: u8) {
        self.st.label[0] = 0;
        if p < 8 {
            self.hs.lit = 0x40;
        }
    }
    fn nickname(&self, o: u8) -> Vec<u8> {
        // SPEC GAP: a nickname without its zero within the 9 bytes is cut there.
        (0..9).map(|k| self.rb(o as usize, 0x16E + k)).take_while(|&b| b != 0).collect()
    }
    /// Original 0x3349C Light: the redraw-off branches, the unlight of the old lit portrait, lit 0x284F0E = slot,
    /// then the nickname goes to label 0x22A444.
    fn light(&mut self, o: u8) {
        if self.hs.redraw_off != 1 || o >= 8 {
            if self.hs.redraw_off >= 2 && o < 8 {
                self.hs.lit = o;
            }
        } else {
            if self.hs.lit < 0x40 && o != self.hs.lit {
                let l = self.hs.lit;
                self.unlight(l);
            }
            self.hs.lit = o;
        }
        let n = self.nickname(o);
        self.set_label(&n);
    }
    /// Original 0x336E8 SetSpeakerName: named 0x95E0C = slot, label 0x22A444 = "SANTINO" for side 4 (+0xBA),
    /// else the DYING word of +0x183 / 15 (0x964EC, 10 bytes each).
    fn name(&mut self, o: u8) {
        self.hs.named = o;
        if self.rs(o as usize, 0xBA) == 4 {
            self.set_label(b"SANTINO");
        } else {
            let k = ((self.rs(o as usize, 0x183) / 15) & 0xFF) as usize;
            self.set_label(health_word(k));
        }
    }
    // Original 0x1D2BC repairs a stale walk cursor from the selected merc's hp, combat mode and slot.
    fn dead_fix(&mut self) {
        self.st.value = dead_cursor_fix(self.st.value, self.rb(self.sel(), 0x183), self.st.mode,
                                        self.rb(self.sel(), 0x17A));
    }
    /// cursor.md "Weapon cursor" for the selected merc (the strip click's op 352).
    pub fn weapon(&mut self) {
        self.weapon_cursor();
    }
    fn weapon_cursor(&mut self) {
        let s = self.sel();
        self.weapon_cursor_soldier(s);
    }
    fn weapon_cursor_soldier(&mut self, s: usize) {
        weapon_cursor_state(self.st, self.w, s);
    }
    fn mode_cursor(&mut self) {
        let s = self.sel();
        self.mode_cursor_soldier(s);
    }
    /// MoveCursorState? 0x1C464 (EAX soldier): HidePathMarks(0), then cursor 0x284F1D by mode 0x284F1B: 0 -> hp
    /// (0xA1, mode 1) / slot (0x8C), 1 -> hp >= 15 (0x8E) / hp (0xA1) / 0x8C with mode 0, 2 -> weapon cursor,
    /// 3 -> 0x94.
    fn mode_cursor_soldier(&mut self, s: usize) {
        move_cursor_state(self.st, self.w, self.h, s);
    }
    /// CursorModeSet? 0x1C438 (host op 185 click_panel of TacticalClick, SoldierDies and StoryFind): the mode setter
    /// 0x284F1B (a mode above 2 becomes 1), then MoveCursorState 0x1C464 of the soldier passed (the callers pass
    /// the selected one).
    pub fn click_panel(&mut self, slot: i32, mode: i32) {
        click_panel(self.st, self.w, self.h, slot, mode);
    }
    fn ap_colours(&mut self) {
        let left = self.rs(self.sel(), 0x1A5);
        (self.st.ap_fg, self.st.ap_shadow) = if left < self.st.ap { (1, 0) } else { (0xFF, 0xFE) };
    }
    fn target_ap(&mut self) {
        let k = self.hand_kind();
        if k != 0x92 && k != 0x95 {
            return;
        }
        self.hs.ap_timer -= 10;
        if self.hs.ap_timer <= 0 {
            self.hs.ap_timer = 0;
            // SAFETY: valid world and host.
            let a = unsafe { ja1_action_ap(self.w, self.h, self.sel() as i32, self.st.tile, 1) };
            self.st.ap = a as i8 as i32;
            let left = self.rs(self.sel(), 0x1A5);
            if self.rb(self.sel(), 0x16A) != 0 && self.st.ap < left {
                self.st.ap = if self.st.ap + 4 <= left { self.st.ap + 4 } else { left };
            }
            self.ap_colours();
        }
    }
    fn tile_standable(&self, t: i32) -> i32 {
        let o = self.tb(t, 5) & 0x7F;
        if o < 0x30 && self.rs(o as usize, 0x9C) > 0 {
            return 0;
        }
        let t1 = self.tb(t, 1);
        let class = if self.wd().t1_class.is_null() { 0 } else { unsafe { *self.wd().t1_class.add(t1 as usize) } };
        if class <= 5 {
            return 1;
        }
        match t1 {
            0x3B | 0x41 | 0x5A | 0x60 => 1,
            0x3D | 0x3E | 0x5C | 0x5D => -2,
            0x43 | 0x45 | 0x62 | 0x64 => -1,
            // Original 0x23A78: g_tileMap[tile].t3, the tile class byte (UseItemAt reads it at 0x1EA56 / 0x1EA7D /
            // 0x1EAAA); the port reads the same byte below and in the mode cursor.
            _ => (self.tb(t, 3) == 0x73) as i32,
        }
    }

    /// Hover steps 1-4 (also the common steps of a click); false when step 1 stops.
    fn common(&mut self, t: i32) -> bool {
        // 1.
        self.st.over_map = 1;
        if self.st.overlay_up != 0 || self.st.tactical_on == 0 || self.sel() >= 8 {
            return false;
        }
        let s = self.sel();
        // 2. (strip_hover is the front-end's own; the window clears it.)
        // 3.
        if t != self.st.tile {
            self.hs.ap_timer = 800;
            self.hs.item_timer = 2000;
            if self.st.mode == 1 {
                self.st.value = 0x8E;
            }
            self.hide_marks();
            self.st.tile = t;
        }
        // 4.
        if self.rb(s, 0x1B8) == 0 {
            self.hs.ap_timer = 0;
            if self.st.ap != 0 {
                self.hide_marks();
            }
        }
        true
    }

    /// The step (cursor.md "Hover" steps 1-10).
    pub fn step(&mut self, t: i32) {
        if !self.common(t) {
            return;
        }
        // 5.
        let o = self.tb(t, 5) & 0x7F;
        self.st.wobble_dir = 0;
        // 6.
        if self.hs.ap_timer > 0 && matches!(self.st.value, 0x74 | 0x75 | 0x92 | 0x95) {
            self.target_ap();
        }
        // 7.
        if o < 0x30 && self.rs(o as usize, 0x9C) > 0 {
            self.soldier_tile(t, o);
        } else {
            self.empty_tile(t);
        }
        // 8.
        if self.st.wobble_dir >= 0 {
            self.st.wobble = -1000;
        }
        // 9.
        if self.st.mode == 1 && self.hs.item_timer > 0 {
            self.hs.item_timer -= 10;
            if self.hs.item_timer <= 0 {
                self.hs.item_timer = 0;
                if self.ground_item(t) {
                    self.st.value = 0x94;
                    self.st.ap = 0;
                }
            }
        }
        // 10.
        self.range_test();
    }

    /// Hover step 10: the range test of the weapon / item cursor (also after each click). The 0x95 branch is
    /// the original 0x58AB8: reach = item range * 15 + hp - 50, then less (100 - condition) / 200 of it, floor 0.
    fn range_test(&mut self) {
        let s = self.sel();
        self.st.out_of_range = 0;
        if self.st.mode >= 2 {
            let ty = self.rb(s, 0);
            let limit: i32 = if self.st.mode == 3 {
                15
            } else if self.it(ty, 2) == 0x95 {
                let mut r = self.it(ty, 11) as i32 * 15 + self.rs(s, 0x183) - 50;
                r -= (100 - self.rs(s, 0x18A)) * r / 200;
                r.max(0) as i16 as i32
            } else {
                (self.it(ty, 11) as i32 * 15) as i16 as i32
            };
            let x = (self.rb(s, 0x116) as u16 | (self.rb(s, 0x117) as u16) << 8) as i16 as i32;
            let y = (self.rb(s, 0x118) as u16 | (self.rb(s, 0x119) as u16) << 8) as i16 as i32;
            let tile = self.st.tile;
            let q = tile / 56;
            let (px, py) = ((tile - 56 * q) * 16 + 5, q * 14 + 5);
            let dx = (x - px).abs() * 15 / 16;
            let dy = (y - py).abs() * 15 / 14;
            let d = ((dx as i64 * dx as i64 + dy as i64 * dy as i64) as f64).sqrt() as i64;
            let d = isqrt_fix(dx as i64 * dx as i64 + dy as i64 * dy as i64, d);
            if d > limit as i64 {
                self.st.out_of_range = 1;
            }
        }
    }

    fn soldier_tile(&mut self, t: i32, o: u8) {
        // 7a.1
        self.st.wobble = -1000;
        let sel = self.sel() as u8;
        if o < 8 {
            // 7a.2
            self.hs.named = 0x40;
            if o != self.hs.lit {
                if self.hs.redraw_off < 2 {
                    self.light(o);
                } else {
                    self.hs.lit = 0x40;
                    if self.hs.light_wait == 0 {
                        self.hs.light_wait = 1;
                    } else if self.hs.light_timer < 1 {
                        self.light(o);
                        self.hs.light_wait = 0;
                        self.hs.light_timer = 300;
                    } else {
                        self.hs.light_timer -= 10;
                    }
                }
            }
            let v = self.st.value;
            if self.st.mode < 2 {
                if o == sel {
                    if matches!(v, 0x91 | 0xB5 | 0xA4) {
                        self.dead_fix();
                    }
                } else if self.rb(o as usize, 0x194) == 0 {
                    self.st.value = 0x91;
                }
            } else if o == sel && self.st.mode == 2 && v != 0x93 {
                self.st.value = 0xB5;
            } else if v == 0xB5 {
                if self.st.mode == 2 {
                    self.weapon_cursor();
                } else {
                    self.st.value = 0x94;
                }
            } else if self.s_hp() < 15 {
                self.st.value = 0xA1;
            }
        } else {
            // 7a.3
            self.hs.light_wait = 0;
            self.hs.light_timer = 300;
            if self.hs.lit < 0x40 {
                let l = self.hs.lit;
                self.unlight(l);
                if self.st.value == 0x91 {
                    self.dead_fix();
                }
            }
            if self.st.mode == 1 && o != self.hs.named {
                self.name(o);
                if self.st.value == 0xA4 {
                    self.mode_cursor();
                } else if self.s_hp() < 15 {
                    self.st.value = 0xA1;
                }
            }
            if self.st.mode == 1 || self.st.mode == 2 {
                match self.st.value {
                    0x92 | 0x95 | 0x96 => {
                        self.st.value = match self.st.value {
                            0x92 => 0x74,
                            0x95 => 0x75,
                            _ => 0x76,
                        };
                        self.hs.target = o as i32;
                    }
                    0xB5 => self.weapon_cursor(),
                    _ => {}
                }
            }
        }
        // 7a.4
        if self.st.mode == 3 {
            self.st.value = if self.ground_item(t) { 0x77 } else { 0x94 };
        }
    }

    fn empty_tile(&mut self, t: i32) {
        // 1.
        self.hs.light_timer = 300;
        let s = self.sel();
        // 2.
        if self.s_hp() < 15 && self.st.mode != 0 {
            self.st.value = 0xA1;
        } else if self.st.mode == 1 && self.hs.ap_timer > 0 && matches!(self.st.value, 0x8E | 0xA4)
            && self.rb(s, 0x1BE) == 0 && self.rb(s, 0x1BF) == 0
        {
            self.hs.ap_timer -= 10;
            if self.hs.ap_timer <= 0 {
                self.hs.ap_timer = 0;
                let mark = (self.wd().path_preview == 0) as i32;
                // SAFETY: valid world and host.
                self.st.ap = unsafe {
                    ja1_path_ap_marks(self.w, self.h, s as i32, self.st.tile, mark, 0, self.shift as i32,
                                      self.rs(s, 0x177), self.alt as i32)
                };
                self.ap_colours();
            }
        }
        // 3.
        self.hs.named = 0x40;
        self.hs.target = 0x40;
        if self.hs.lit < 0x40 {
            let l = self.hs.lit;
            self.unlight(l);
        } else if self.st.value == 0x91 {
            self.dead_fix();
        }
        // 4.
        let d = self.tile_standable(t) as i8;
        self.st.wobble_dir = d;
        // 5.
        if d < 1 {
            if self.st.mode == 1 {
                if d == 0 {
                    if self.st.value != 0x94 {
                        self.st.value = 0xB5;
                    }
                } else {
                    self.st.value = if self.st.wobble > 0 { 0xA4 } else { 0xB5 };
                    self.st.ap = 0;
                }
            } else if self.st.value == 0xB5 {
                self.mode_cursor();
            }
            if self.st.wobble == -1000 {
                let low = |k: i32| self.tb(k, 3) <= 0x50;
                if low(t) || (d == -2 && low(t - 112)) || (d != -2 && low(t - 1)) {
                    self.st.wobble = 550;
                }
            } else if self.st.mode != 1 {
                self.st.wobble -= 10;
            }
        } else {
            let t3 = self.tb(t, 3);
            let t0 = self.tb(t, 0);
            let c0 = if self.wd().t0_class.is_null() { 0 } else { unsafe { *self.wd().t0_class.add(t0 as usize) } };
            if self.st.mode == 2 || c0 != 3 || t3 == 0 || (self.gv_t3[t3 as usize] != 0 && !matches!(t3, 0x54 | 0x5F | 0x6A)) {
                if matches!(self.st.value, 0xB5 | 0xA4) {
                    self.mode_cursor();
                }
            } else {
                self.st.value = 0xB5;
            }
        }
        // 6.
        if self.st.mode == 3 {
            self.st.value = if self.ground_item(t) { 0x77 } else { 0x94 };
        } else {
            self.st.value = match (self.st.value, d >= 0) {
                (0x74, _) => 0x92,
                (0x75, _) => 0x95,
                (0x76, _) => 0x96,
                (0x8D, _) => 0x93,
                (0x8B, true) => 0x97,
                (0xA2, true) => 0x98,
                (0x97, false) => 0x8B,
                (0x98, false) => 0xA2,
                (v, _) => v,
            };
        }
    }
}

/// One poll of WaitConfirmClick? 0x1C514: the answer it ends with, or None while it waits. `under` = the map tile under
/// the mouse (None = the mouse is off the map, which is never the tile). Escape first, then the mouse off the tile or
/// the right button (a left press in the same poll counts for nothing), then the left press.
pub fn confirm_poll(tile: i32, under: Option<i32>, left: bool, right: bool, escape: bool) -> Option<Answer> {
    if escape {
        Some(Answer::Escape)
    } else if under != Some(tile) {
        Some(Answer::Off)
    } else if right {
        Some(Answer::Right)
    } else if left {
        Some(Answer::Left(tile))
    } else {
        None
    }
}

/// The end of WaitConfirmClick? 0x1C514: true on the left press on the tile. Else the tile cursor goes off the map, and
/// all answers but Escape also start the 2000 ms item timer.
pub fn confirm_end(a: Answer, tile: i32, cursor_tile: &mut i32, item_timer: &mut i32) -> bool {
    if a == Answer::Left(tile) {
        return true;
    }
    *cursor_tile = 0xC41;
    if a != Answer::Escape {
        *item_timer = 2000;
    }
    false
}

/// A wait that a click opens (cursor.md "Clicks"): the game waits for the next click or Escape.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ClickWait {
    /// The aim of a weapon click in combat (left click 2.3.6): the shot's cost and the highest AP number.
    Aim { cost: i32, max: i32 },
    /// The confirm of a hand click on a small object (left click 3.3).
    Confirm { tile: i32 },
    /// The confirm of a walk (Walk (S, T) with the confirm option on): the merc's path before the marked one.
    /// `fire`: the wait came from "Fire" (its end runs after the answer), not from a library click in a pass.
    Walk { tile: i32, save: [u8; 80], detector: bool, fire: bool },
    /// The right button is held (right step 3): each `Answer::Poll` runs one poll.
    Hold(Hold),
    /// The confirm of a posture (right step 3.3); the hold goes on after it.
    Posture(Hold),
}

/// The state of a right button hold (cursor.md "Clicks", right step 3).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Hold {
    pub tile: i32,
    pub press_y: i32,
    pub saved: u8,
    pub o: u8,
    pub turn: bool,
    pub look: bool,
    pub posture: bool,
    /// After a posture confirm: the next poll only gives the button state that ends the hold (step 3.4).
    pub check: bool,
    /// Step 3.1: the turn waits for the left button to come up.
    pub left_wait: bool,
}

/// The answer to an open wait.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Answer {
    /// A left click on tile T (the aim ignores the tile: the mouse stays on its tile).
    Left(i32),
    Right,
    Escape,
    /// The mouse left the tile of a confirm (the aim keeps its wait: its mouse stays on its tile).
    Off,
    /// One mouse poll of a right button hold: the buttons and the mouse y (pixels).
    Poll { left: bool, right: bool, y: i32 },
}

/// What a click leaves for the front-end: a status line text and a wait.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct ClickOut {
    /// The texts of the click in order, one per line (a merc text first, then the status line text).
    pub status: Option<String>,
    pub wait: Option<ClickWait>,
    /// The hold ended with a look (right step 4: the look panel routine, SPEC GAP).
    pub look: bool,
    /// The hold that ended in this answer (its turn / look / posture flags).
    pub ended: Option<Hold>,
    /// cursor.md "Drag to an edge": the walk ended in a drag to this edge; the front-end runs `ja1_traverse`.
    pub traverse: Option<i32>,
    /// cursor.md "ALL-move": the squad's move order gave 1; the tile marker becomes kind 0x0F on this tile (after
    /// the order's own op 130).
    pub all_marker: Option<i32>,
    /// cursor.md "Tile popups": right step 2 asks for the popup of this tile (the front-end shows it).
    pub popup: Option<i32>,
    /// cursor.md "Debug keys (level 4)": Shift + left click asks for the inventory screen and the debug pages of
    /// this soldier (the front-end shows them).
    pub debug_pages: Option<usize>,
}

/// A library call of a click (the test interface records them in place of the library: lib.rs ja1f_click).
#[derive(Clone, Copy, Debug)]
pub enum LibCall {
    Click { slot: i32, tile: i32, item: i32 },
    /// Tactical click with the mine detector 0x4D (`ja1_click_detector`): result 2 = the move order is for the caller.
    ClickDetector { slot: i32, tile: i32 },
    Reload { slot: i32 },
    Select { slot: i32, force: i32, quote: i32 },
    /// `walk` = Some(walk AP): `ja1_move_confirmed` after a confirm click; None: `ja1_move_order`.
    Move { slot: i32, tile: i32, marker: i32, sneak: i32, b177: i32, alt: i32, walk: Option<i32>, squad: i32 },
    Gate { slot: i32, tile: i32 },
    /// Turn (S, d, ticks) (`ja1_turn`).
    Turn { slot: i32, d: i32, ticks: i32 },
    /// Set anim (S, code, keep, reset) (`ja1_anim_set`).
    Anim { slot: i32, code: i32, keep: i32, reset: i32 },
    /// A merc message (S, quote, near): host op 54 merc_message (soldier_update.md "Merc sighting": the balloon).
    Message { slot: i32, quote: i32, near: i32 },
    /// EffectSlot 0x17B70 (S, kind): host op 65 effect_slot, the merc's own sound (kind 1 = OK).
    Effect { slot: i32, kind: i32 },
}

type LibHook = Box<dyn FnMut(LibCall) -> i32>;
thread_local! {
    /// Set only by the test interface: replaces the library calls of the clicks.
    pub static LIB_HOOK: std::cell::RefCell<Option<LibHook>> = const { std::cell::RefCell::new(None) };
}

extern "C" {
    fn ja1_click(w: *mut Ja1World, h: *const Ja1Host, slot: i32, tile: i32, item: i32) -> i32;
    fn ja1_click_detector(w: *mut Ja1World, h: *const Ja1Host, slot: i32, tile: i32) -> i32;
    fn ja1_reload(w: *mut Ja1World, h: *const Ja1Host, slot: i32) -> i32;
    fn ja1_select_merc(w: *mut Ja1World, h: *const Ja1Host, slot: i32, force: i32, quote: i32) -> i32;
    fn ja1_move_order(w: *mut Ja1World, h: *const Ja1Host, slot: i32, tile: i32, marker: i32, sneak: i32, byte177: i32,
                      alt: i32, squad: i32) -> i32;
    fn ja1_move_gate(w: *mut Ja1World, h: *const Ja1Host, slot: i32, tile: i32) -> i32;
    fn ja1_turn(w: *mut Ja1World, h: *const Ja1Host, slot: i32, d: i32, ticks: i32) -> i32;
    fn ja1_anim_set(w: *mut Ja1World, h: *const Ja1Host, slot: i32, code: i32, keep: i32, reset: i32);
    fn ja1_direction(x1: i32, y1: i32, x2: i32, y2: i32) -> i32;
    fn ja1_move_confirmed(w: *mut Ja1World, h: *const Ja1Host, slot: i32, tile: i32, marker: i32, sneak: i32,
                          byte177: i32, alt: i32, walk_ap: i32) -> i32;
}

/// `ja1_move_gate` (the test interface runs it in the library).
pub fn move_gate(w: *mut Ja1World, h: *const Ja1Host, slot: i32, tile: i32) -> i32 {
    // SAFETY: the caller passes a valid world and host.
    unsafe { ja1_move_gate(w, h, slot, tile) }
}

impl<'a> Hover<'a> {
    fn wb(&mut self, r: usize, off: usize, v: u8) {
        if r >= 48 || self.wd().soldiers.is_null() {
            return;
        }
        // SAFETY: soldiers = 48 records of 512 bytes.
        unsafe { *self.wd().soldiers.add(r * REC + off) = v }
    }
    fn w16(&mut self, r: usize, off: usize, v: u16) {
        self.wb(r, off, v as u8);
        self.wb(r, off + 1, (v >> 8) as u8);
    }
    fn r16(&self, r: usize, off: usize) -> i32 {
        (self.rb(r, off) as u16 | (self.rb(r, off + 1) as u16) << 8) as i16 as i32
    }
    fn anim_word(&self, r: usize) -> u16 {
        anim_word_of(self.wd(), r)
    }
    fn in_water(&self, r: usize) -> bool {
        let (g, o) = (self.rb(r, 0x1B2), self.rb(r, 0x1B4));
        g >= 0x6C && o != 0x1C && o != 0x1F
    }
    fn occupant(&self, t: i32) -> u8 {
        // SPEC GAP: a tile off the map has nobody on it.
        if (0..3136).contains(&t) { self.tb(t, 5) & 0x7F } else { 0x40 }
    }
    /// "Not enough AP (S, c)"; Some(text) when the action is refused.
    fn not_enough_ap(&self, c: i32) -> Option<String> {
        let s = self.sel();
        // The AP number is compared as an unsigned byte (a cost below 0 is never enough).
        if self.wd().combat != 0 && self.rs(s, 0x1A5) + self.rs(s, 0x1E2) < (c & 0xFF) {
            let n = self.nickname(s as u8);
            return Some(format!("{} doesn't have enough Action Points!", String::from_utf8_lossy(&n)));
        }
        None
    }
    fn hide_marks_force(&mut self) {
        self.st.ap = 0;
        self.st.tile = 0xC41;
        let f = (self.wd().path_preview != 0) as i32;
        // SAFETY: valid world and host.
        unsafe { ja1_hide_path_marks(self.w, self.h, f) }
    }
    fn lib(&mut self, c: LibCall) -> i32 {
        if let Some(r) = LIB_HOOK.with(|k| k.borrow_mut().as_mut().map(|f| f(c))) {
            return r;
        }
        // SAFETY: valid world and host.
        let r = unsafe {
            match c {
                LibCall::Click { slot, tile, item } => ja1_click(self.w, self.h, slot, tile, item),
                LibCall::ClickDetector { slot, tile } => ja1_click_detector(self.w, self.h, slot, tile),
                LibCall::Reload { slot } => ja1_reload(self.w, self.h, slot),
                LibCall::Select { slot, force, quote } => ja1_select_merc(self.w, self.h, slot, force, quote),
                LibCall::Move { slot, tile, marker, sneak, b177, alt, walk: None, squad } =>
                    ja1_move_order(self.w, self.h, slot, tile, marker, sneak, b177, alt, squad),
                LibCall::Move { slot, tile, marker, sneak, b177, alt, walk: Some(r), .. } =>
                    ja1_move_confirmed(self.w, self.h, slot, tile, marker, sneak, b177, alt, r),
                LibCall::Gate { slot, tile } => ja1_move_gate(self.w, self.h, slot, tile),
                LibCall::Turn { slot, d, ticks } => ja1_turn(self.w, self.h, slot, d, ticks),
                LibCall::Anim { slot, code, keep, reset } => {
                    ja1_anim_set(self.w, self.h, slot, code, keep, reset);
                    0
                }
                LibCall::Message { slot, quote, near } => {
                    let a = [slot, quote, near];
                    ((*self.h).call)((*self.h).ctx, 54, a.as_ptr(), 3, std::ptr::null_mut(), 0);
                    0
                }
                LibCall::Effect { slot, kind } => {
                    let a = [slot, kind];
                    ((*self.h).call)((*self.h).ctx, 65, a.as_ptr(), 2, std::ptr::null_mut(), 0);
                    0
                }
            }
        };
        // CursorModeSet? 0x1C438: host op 185 click_panel of the library call runs here, inside the click handler
        // (UseItemAt 0x1D9B8 reads the cursor mode right after TacticalClick returns at 0x1ECFD, before its
        // `set cursor after the click` step).
        if let Some((slot, mode)) = crate::play::take_click_panel_now(self.h) {
            click_panel(self.st, self.w, self.h, slot, mode);
        }
        r
    }
    fn click_lib(&mut self, t: i32, item: i32) -> i32 {
        let slot = self.sel() as i32;
        self.lib(LibCall::Click { slot, tile: t, item })
    }
    /// The hand click of UseItemAt (0x1DC6E, 0x1E0E4): TacticalClick with item 0; a walk to a use spot plays the
    /// merc's OK sound (0x1DC88, 0x1E0FE; live: timer +0xC0 = 400 after a Ctrl click, #31).
    fn hand_click(&mut self, t: i32) {
        if self.click_lib(t, 0) != 0 {
            self.ok_sound();
        }
    }
    /// EffectSlot (+0x17A, 1).
    fn ok_sound(&mut self) {
        let slot = self.rb(self.sel(), 0x17A) as i32;
        self.lib(LibCall::Effect { slot, kind: 1 });
    }
    /// "Target memory (S, T)". Original 0x1CAD0 (called by UseItemAt at 0x1E066): when the soldier's anim byte
    /// +0x1C2 is 1..16, store the tile's occupant in +0x1C2 and the target in +0x1F7 / +0x1BE / +0xDC.
    fn target_memory(&mut self, t: i32) {
        let s = self.sel();
        if !(1..=16).contains(&self.rb(s, 0)) {
            return;
        }
        let o = self.occupant(t);
        self.wb(s, 0x1C2, o);
        if o == 0x40 {
            self.wb(s, 0x1F7, 0x40);
            self.wb(s, 0x1BE, 1);
            self.w16(s, 0xDC, t as u16);
            self.wb(s, 0x1C2, 0x3F);
        } else if o >= 0x28 {
            if self.rs(o as usize, 0x9C) > 0 {
                self.wb(s, 0x1F7, o);
                self.w16(s, 0x1F8, t as u16);
            } else {
                self.wb(s, 0x1F7, 0x40);
                self.wb(s, 0x1C2, 0x3F);
            }
        }
    }
    /// "Fire".
    fn fire(&mut self, t: i32, out: &mut ClickOut) {
        let ty = self.rb(self.sel(), 0) as i32;
        let r = if ty == 0x4D {
            let slot = self.sel() as i32;
            match self.lib(LibCall::ClickDetector { slot, tile: t }) {
                2 => {
                    self.detector_move(t, out, true);
                    0
                }
                r => r,
            }
        } else {
            self.click_lib(t, ty)
        };
        if out.wait.is_some() {
            return; // (the end of "Fire" runs after the answer)
        }
        if r != 0 {
            self.target_memory(t);
            let s = self.sel();
            self.wb(s, 0x197, 0);
            // 0x1E078..0x1E089.
            if self.rb(s, 0x1A6) != 0 {
                self.ok_sound();
            }
        }
        self.st.ap = 0;
    }
    /// Tactical click item 0x4D, the mine detector: MoveMercTo 0x1C608 (S, T, marker 1, sneak 0, +0x177 = 1, Alt 0, confirm 1,
    /// squad 0). A merc with the option on waits for a second click (0x1C7A8..0x1C7C5, live check #45).
    /// `fire` = false: the library's click in a pass left the move order (ActiveFrame 0x1BFD9 at a walk's end, Ja1World
    /// detector_tile); the original waits inside the pass, the front-end right after it.
    fn detector_move(&mut self, t: i32, out: &mut ClickOut, fire: bool) {
        let s = self.sel();
        if self.confirm_move && s < 8 && self.lib(LibCall::Gate { slot: s as i32, tile: t }) != 0 {
            let mut save = [0u8; 80];
            // SAFETY: paths = 48 x 80 bytes.
            unsafe { std::ptr::copy_nonoverlapping(self.wd().paths.add(80 * s), save.as_mut_ptr(), 80) };
            // SAFETY: valid world and host.
            let r = unsafe { ja1_path_ap_marks(self.w, self.h, s as i32, t, 1, 0, 0, 1, 0) };
            if r <= 0 {
                out.status = Some("Can't move there!".into());
                return;
            }
            self.st.value = 0x8F;
            self.st.tile = t;
            out.wait = Some(ClickWait::Walk { tile: t, save, detector: true, fire });
            return;
        }
        self.lib(LibCall::Move { slot: s as i32, tile: t, marker: 1, sneak: 0, b177: 1, alt: 0, walk: None, squad: 0 });
    }
    /// The detector move order a library click in a pass left (Ja1World detector_tile): the confirm wait or the move.
    pub fn detector_pass(&mut self, t: i32) -> ClickOut {
        let mut out = ClickOut::default();
        self.detector_move(t, &mut out, false);
        out
    }
    /// "Walk (S, T)".
    fn walk(&mut self, t: i32, out: &mut ClickOut) {
        let s = self.sel();
        for off in [0x177, 0x1A8, 0x196, 0x16D, 0x1A6, 0x1A7, 0x95] {
            self.wb(s, off, 0);
        }
        self.w16(s, 0x134, 0xC41);
        self.w16(s, 0x96, 0xC41);
        let (shift, alt) = (self.shift as i32, self.alt as i32);
        // SAFETY: valid world and host.
        unsafe {
            (*self.w).key_shift = shift as u8;
            (*self.w).key_alt = alt as u8;
        }
        if self.drag != 0 {
            out.traverse = Some(self.drag);
            return;
        }
        // The walk confirm: a merc, the option on, no squad walk (cursor.md "ALL-move"), the move order's gates passed.
        if self.confirm_move && self.squad == 0 && s < 8 && self.lib(LibCall::Gate { slot: s as i32, tile: t }) != 0 {
            let mut save = [0u8; 80];
            // SAFETY: paths = 48 x 80 bytes.
            unsafe { std::ptr::copy_nonoverlapping(self.wd().paths.add(80 * s), save.as_mut_ptr(), 80) };
            // SAFETY: valid world and host.
            let r = unsafe { ja1_path_ap_marks(self.w, self.h, s as i32, t, 1, 0, shift, 0, alt) };
            if r <= 0 {
                out.status = Some("Can't move there!".into());
                return;
            }
            self.st.value = 0x8F;
            self.st.tile = t;
            out.wait = Some(ClickWait::Walk { tile: t, save, detector: false, fire: false });
            return;
        }
        let (marker, sneak, _, alt, squad) = crate::drag::move_args(shift, alt, self.squad);
        let r = self.lib(LibCall::Move { slot: s as i32, tile: t, marker, sneak, b177: 0, alt, walk: None, squad });
        if crate::drag::all_marker(self.squad != 0, r) {
            out.all_marker = Some(t);
        }
    }

    /// SlotMouse 0x119CC: a left click on the selected merc's portrait while it shows "cont": the merc goes on with
    /// its open order (the task +0x95 / tile +0x96 at its walk's end, else the walk to +0x120).
    pub fn cont(&mut self) -> ClickOut {
        let mut out = ClickOut::default();
        let s = self.sel();
        if s >= 8 {
            return out;
        }
        self.hide_marks_force();
        // SAFETY: soldiers = 48 records of 512 bytes, and no other reference to them lives in this block.
        let plan = crate::slotmouse::cont_plan(unsafe { std::slice::from_raw_parts_mut(self.wd().soldiers.add(s * REC), REC) });
        match plan {
            crate::slotmouse::ContPlan::Use { here, tile, task } => {
                let g = self.tb(tile & 0xFFFF, 6) as i32;
                // SAFETY: valid world and host.
                let cost = unsafe { ja1_use_cost(self.w, self.h, s as i32, here, tile, g, task as i32) };
                if let Some(m) = self.not_enough_ap(cost) {
                    out.status = Some(m);
                    return out;
                }
                self.click_lib(tile, crate::slotmouse::click_item(task));
            }
            crate::slotmouse::ContPlan::Walk { dest, sneak, b177, b196 } => {
                // SAFETY: valid world and host.
                unsafe { ja1_move_order(self.w, self.h, s as i32, dest, 1, sneak as i32, b177 as i32, b196 as i32, 0) };
            }
        }
        out
    }

    /// panel.md "The overview mouse step" (mode 0) at mouse point (x, y) with `buttons` (1 left, 2 right). The
    /// cursor tile, ap_timer and AP number are the cursor's; `ov` keeps the overview colour and "over" byte.
    /// Returns the click's out and true when the overview click (right button) follows.
    pub fn overview(&mut self, ov: &mut crate::viewmode::OvMouse, x: i32, y: i32, buttons: i32) -> (ClickOut, bool) {
        use crate::viewmode::OvCall;
        let s = self.sel();
        *ov = crate::viewmode::OvMouse { shown: 1, mouse_on: 1, x, y, buttons, tile: self.st.tile, ap_timer: self.hs.ap_timer,
                                         cost: self.st.ap, sel: s as i32, sel_1b8: self.rb(s, 0x1B8) as i32,
                                         sel_hp: self.rs(s, 0x183), sel_ap: self.rs(s, 0x1A5), sel_177: self.rb(s, 0x177) as i32,
                                         shift: self.shift as i32, alt: self.alt as i32, ..*ov };
        let (mut out, mut jump) = (ClickOut::default(), false);
        let (shift, alt, b177) = (ov.shift, ov.alt, ov.sel_177);
        ov.step(&mut |c| match c {
            OvCall::Occupant(t) => self.occupant(t) as i32,
            OvCall::Hp(o) => self.rs(o as usize, 0x183),
            OvCall::ShowMouse => 0,
            // SAFETY: valid world and host.
            OvCall::HideMarks => unsafe {
                ja1_hide_path_marks(self.w, self.h, 1);
                0
            },
            OvCall::Select(o) => self.lib(LibCall::Select { slot: o as i32, force: 0, quote: 1 }),
            OvCall::NotAvailable => {
                out.status = Some("Not available!".into());
                0
            }
            OvCall::Walk(t) => {
                // OverviewMouse 0x2474B calls AllMoveTraverse? with BL = 0: the walk has no confirm.
                self.confirm_move = false;
                self.walk(t, &mut out);
                0
            }
            // SAFETY: valid world and host.
            OvCall::Cost(t) => unsafe { ja1_path_ap_marks(self.w, self.h, s as i32, t, 1, 0, shift, b177, alt) },
            OvCall::Jump => {
                jump = true;
                0
            }
        });
        self.st.tile = ov.tile;
        self.hs.ap_timer = ov.ap_timer;
        self.st.ap = ov.cost;
        (out, jump)
    }

    /// cursor.md "Clicks", left button at tile `t`.
    /// UseItemAt 0x1D9B8: does a left press at `t` reach AllMoveTraverse? 0x1CFEC? Only then does the press start the
    /// edge drag. A press on a merc selects at the press, and the other arms act at the press too. No state changes.
    pub fn press_walks(&self, t: i32) -> bool {
        let s = self.sel();
        if self.st.overlay_up != 0 || self.st.tactical_on == 0 || s >= 8 || self.st.mode != 1 {
            return false;
        }
        let o = self.occupant(t);
        if crate::debugkeys::level() == 4 && self.shift && o < 0x30 {
            return false;
        }
        // The common step: a new tile in mode 1 puts the value to 0x8E before the arms read it.
        let value = if t != self.st.tile { 0x8E } else { self.st.value };
        let (t0, t3) = (self.tb(t, 0), self.tb(t, 3));
        // SAFETY: a table that is not null has 256 entries.
        let c0 = if self.wd().t0_class.is_null() { 0 } else { unsafe { *self.wd().t0_class.add(t0 as usize) } };
        if c0 == 3 && t3 != 0 && value != 0xA4 && (self.gv_t3[t3 as usize] == 0 || matches!(t3, 0x54 | 0x5F | 0x6A)) {
            return false;
        }
        o >= 8 && !matches!(value, 0xA4 | 0x94 | 0xB5)
    }

    pub fn left(&mut self, t: i32) -> ClickOut {
        if !self.common(t) {
            return ClickOut::default();
        }
        // cursor.md "Debug keys (level 4)": Shift + left click on a soldier opens its inventory screen, then its
        // debug pages, in place of the left button steps; the click then ends.
        let o = self.occupant(t);
        if crate::debugkeys::level() == 4 && self.shift && o < 0x30 {
            return ClickOut { debug_pages: Some(o as usize), ..Default::default() };
        }
        let out = self.left_click(t);
        if out.wait.is_none() {
            self.range_test();
        }
        out
    }

    fn left_click(&mut self, t: i32) -> ClickOut {
        let mut out = ClickOut::default();
        let s = self.sel();
        let o = self.occupant(t);
        // SAFETY: valid world.
        unsafe { ((*self.w).spotted, (*self.w).shown_slot, (*self.w).view_tile) = (0x40, 0x40, 0xC41) };
        let (in_cur, hp) = (self.rb(s, 0x182), self.s_hp());
        match self.st.mode {
            3 => {
                if in_cur != 0 && hp > 14 {
                    self.hand_click(t);
                }
            }
            2 => {
                let a = self.anim_word(s);
                if in_cur == 0 || hp < 15 || !(4..=0x31).contains(&a) || self.st.value == 0xA1 {
                    return out;
                }
                let ty = self.rb(s, 0);
                if self.rs(s, 2) < 10 && self.it(ty, 9) != 0 {
                    out.status = Some("Can't use that!".into());
                    return out;
                }
                let k = self.it(ty, 2);
                if !matches!(k, 0x92 | 0x95 | 0x96) {
                    self.fire(t, &mut out);
                    return out;
                }
                if self.rb(s, 1) == 0 && self.lib(LibCall::Reload { slot: s as i32 }) == 1 {
                    self.lib(LibCall::Message { slot: s as i32, quote: 5, near: 0x40 });
                    self.wb(s, 0x1CD, 1);
                    return out;
                }
                let u = self.r16(s, 0xD0);
                if self.rb(s, 0x19C) == 3 && u != 0xC41 && u != t {
                    let q = self.occupant(u);
                    if q < 0x40 && self.rs(q as usize, 0x183) > 0 {
                        self.lib(LibCall::Message { slot: s as i32, quote: 8, near: 0x40 });
                        return out;
                    }
                }
                // SAFETY: valid world and host.
                // TargetApNumber? 0x1D690: ActionApCost, then +4 when aiming is possible (0x16A) and
                // below the AP left; the colours 0xC2 / 0xFF follow.
                let cost = unsafe { ja1_action_ap(self.w, self.h, s as i32, t, 1) } as i8 as i32;
                let left = self.rs(s, 0x1A5);
                self.st.ap = cost;
                if self.rb(s, 0x16A) != 0 && cost < left {
                    self.st.ap = if cost + 4 <= left { cost + 4 } else { left };
                }
                if self.r16(s, 0x114) == t {
                    self.st.ap = 0;
                    return out;
                }
                if let Some(m) = self.not_enough_ap(self.st.ap) {
                    self.st.ap = 0;
                    out.status = Some(m);
                    return out;
                }
                if self.fast_fire {
                    self.wb(s, 0x1FD, 0);
                    self.fire(t, &mut out);
                    return out;
                }
                if self.wd().combat == 0 {
                    self.wb(s, 0x1FD, 4);
                    self.weapon_cursor();
                    self.st.ap = 0;
                    self.fire(t, &mut out);
                    return out;
                }
                self.st.value = 0x9C;
                self.st.tile = t;
                self.st.wobble = 0;
                self.st.no_blink = 1;
                self.st.ap_shadow = 0xFE;
                self.st.ap_fg = if left == self.st.ap || cost < self.st.ap { 0xC2 } else { 0xFF };
                let m = (cost + 4) as i8 as i32;
                self.wb(s, 0x1FD, 0);
                out.wait = Some(ClickWait::Aim { cost, max: if left < m { left } else { m } });
            }
            _ => {
                self.hide_marks_force();
                let (t0, t3) = (self.tb(t, 0), self.tb(t, 3));
                let c0 = if self.wd().t0_class.is_null() { 0 } else { unsafe { *self.wd().t0_class.add(t0 as usize) } };
                if c0 == 3 && t3 != 0 && self.st.value != 0xA4
                    && (self.gv_t3[t3 as usize] == 0 || matches!(t3, 0x54 | 0x5F | 0x6A)) {
                    return out;
                }
                if matches!(self.st.value, 0xA4 | 0x94) {
                    if in_cur != 0 && hp > 14 {
                        self.wb(s, 0x1B0, 0);
                        if self.st.value == 0x94 && self.tb(t, 1) < 0x21 {
                            self.st.value = 0x77;
                            self.st.tile = t;
                            out.wait = Some(ClickWait::Confirm { tile: t });
                            return out;
                        }
                        self.hand_click(t);
                    }
                    return out;
                }
                if o >= 8 {
                    if self.st.mode != 0 {
                        if self.st.value == 0xB5 {
                            self.st.value = 0x94;
                            self.st.tile = t;
                        } else {
                            self.wb(s, 0x1B0, 0);
                            self.walk(t, &mut out);
                        }
                    }
                    return out;
                }
                if self.rs(o as usize, 0x183) != 0 {
                    self.st.mode = 1;
                    // Alt on the selected merc's own tile: the posture first (as the right hold's posture yes).
                    if self.alt && o as usize == s && self.anim_word(s) >= 4 && !self.in_water(s) {
                        if self.anim_word(s) == 5 {
                            self.lib(LibCall::Anim { slot: s as i32, code: 7, keep: 1, reset: 0 });
                        } else if let Some(text) = self.not_enough_ap(2) {
                            out.status = Some(text);
                        } else {
                            self.lib(LibCall::Anim { slot: s as i32, code: 6, keep: 1, reset: 0 });
                        }
                    }
                    if self.lib(LibCall::Select { slot: o as i32, force: 1, quote: 1 }) != 0 {
                        self.mode_cursor();
                        return out;
                    }
                }
                // After a merc text (the posture's AP text) the status line text comes second.
                out.status = Some(match out.status.take() {
                    Some(m) => m + "\nNot available!",
                    None => "Not available!".into(),
                });
            }
        }
        out
    }

    /// cursor.md "Clicks", right button press at tile `t` with the mouse at y: steps 1 and 2, then the hold (a wait
    /// that `Answer::Poll` answers).
    pub fn right(&mut self, t: i32, y: i32) -> ClickOut {
        if !self.common(t) {
            return ClickOut::default();
        }
        let mut out = ClickOut::default();
        let o = self.occupant(t);
        // SAFETY: valid world.
        unsafe { ((*self.w).spotted, (*self.w).shown_slot, (*self.w).view_tile) = (0x40, 0x40, 0xC41) };
        self.hide_marks_force();
        if self.st.mode == 3 || self.st.value == 0x94 {
            // cursor.md "Tile popups": the front-end shows the popup of T with the right-button wait.
            self.range_test();
            out.popup = Some(t);
            return out;
        }
        out.wait = Some(ClickWait::Hold(Hold { tile: t, press_y: y, saved: self.st.value, o, turn: false, look: false,
                                               posture: false, check: false, left_wait: false }));
        out
    }

    /// One poll of a right button hold (right step 3).
    fn hold_poll(&mut self, mut g: Hold, left: bool, right: bool, y: i32) -> ClickOut {
        let mut out = ClickOut::default();
        let s = self.sel();
        let t = g.tile;
        if g.check {
            g.check = false;
        } else if g.left_wait || (left && self.s_hp() >= 15) {
            if !g.left_wait {
                self.st.value = 0x0E;
                self.st.tile = t;
                g.turn = true;
                g.left_wait = true;
            }
            // The wait for the left button: this poll only reads the mouse while the left button is down.
            if left {
                out.wait = Some(ClickWait::Hold(g));
                return out;
            }
            g.left_wait = false;
            let st = self.r16(s, 0x114);
            let px = |t: i32| ((t % 56) * 16 + 5, (t / 56) * 14 + 5);
            let ((x1, y1), (x2, y2)) = (px(st), px(t));
            // SAFETY: a pure function.
            let d = unsafe { ja1_direction(x1, y1, x2, y2) };
            if d == self.rs(s, 0x1A1) {
                self.wb(s, 0x9D, 1);
            } else if let Some(text) = self.not_enough_ap(1) {
                out.status = Some(text);
            } else if self.lib(LibCall::Turn { slot: s as i32, d, ticks: 30 }) != 0 {
                self.wb(s, 0x1D6, 1);
                self.wb(s, 0x1D1, 1);
            }
        } else if y - g.press_y > 10 {
            g.look = true;
            self.st.tile = t;
            self.st.value = 0x46;
        } else if g.o as usize == s && self.anim_word(s) >= 4 && !self.in_water(s) {
            self.st.value = 0x8F;
            self.st.tile = t;
            out.wait = Some(ClickWait::Posture(g));
            return out;
        }
        if right {
            out.wait = Some(ClickWait::Hold(g));
            return out;
        }
        // Step 4.
        if !g.turn && !g.look && !g.posture && self.st.mode != 0 && self.rb(s, 0) != 0 && self.s_hp() > 14 {
            self.st.mode += 1;
            if self.st.mode > 2 {
                self.st.mode = 1;
            }
            self.mode_cursor();
        }
        if g.look {
            self.st.value = g.saved;
            // panel.md "View modes": Set mode (0) runs after the pass (Play::view_key).
            out.look = true;
        } else if g.turn {
            self.st.value = g.saved;
        }
        self.range_test();
        out.ended = Some(g);
        out
    }

    /// The answer to an open wait (cursor.md "Clicks": the aim of left step 2.3.6, the confirm of left step 3.3).
    pub fn answer(&mut self, wait: ClickWait, a: Answer) -> ClickOut {
        let mut out = ClickOut::default();
        let s = self.sel();
        // A poll answers only a hold (the other waits wait for a click or a key).
        if matches!(a, Answer::Poll { .. }) && !matches!(wait, ClickWait::Hold(_) | ClickWait::Posture(_)) {
            out.wait = Some(wait);
            return out;
        }
        match wait {
            ClickWait::Aim { cost, max } => match a {
                Answer::Left(_) => {
                    self.wb(s, 0x1FD, (self.st.ap - cost) as u8);
                    self.st.no_blink = 0;
                    self.weapon_cursor();
                    self.st.ap = 0;
                    let t = self.st.tile;
                    self.fire(t, &mut out);
                }
                Answer::Right => {
                    self.st.ap += 1;
                    if self.st.ap > max {
                        self.st.ap = cost;
                    }
                    self.st.ap_fg = if self.st.ap == max { 0xC2 } else { 0xFF };
                    self.st.ap_shadow = 0xFE;
                    out.wait = Some(wait);
                }
                Answer::Escape => {
                    self.st.no_blink = 0;
                    self.st.tile = 0xC41;
                    self.weapon_cursor();
                    self.st.ap = 0;
                }
                Answer::Off | Answer::Poll { .. } => out.wait = Some(wait),
            },
            ClickWait::Confirm { tile } => {
                if confirm_end(a, tile, &mut self.st.tile, &mut self.hs.item_timer) {
                    self.hand_click(tile);
                }
            }
            ClickWait::Hold(g) => match a {
                Answer::Poll { left, right, y } => return self.hold_poll(g, left, right, y),
                _ => out.wait = Some(wait),
            },
            ClickWait::Posture(mut g) => {
                if let Answer::Poll { .. } = a {
                    out.wait = Some(wait);
                    return out;
                }
                if a == Answer::Left(g.tile) {
                    if self.anim_word(s) == 5 {
                        self.lib(LibCall::Anim { slot: s as i32, code: 7, keep: 1, reset: 0 });
                    } else if let Some(text) = self.not_enough_ap(2) {
                        out.status = Some(text);
                    } else {
                        self.lib(LibCall::Anim { slot: s as i32, code: 6, keep: 1, reset: 0 });
                    }
                    g.posture = true;
                } else {
                    self.st.tile = 0xC41;
                    if a != Answer::Escape {
                        self.hs.item_timer = 2000;
                    }
                    self.mode_cursor();
                }
                g.check = true;
                out.wait = Some(ClickWait::Hold(g));
                return out;
            }
            ClickWait::Walk { tile, save, detector, fire } => {
                let ok = a == Answer::Left(tile);
                if !ok {
                    self.st.tile = 0xC41;
                    if a != Answer::Escape {
                        self.hs.item_timer = 2000;
                    }
                }
                self.mode_cursor();
                if ok {
                    let walk = self.wd().walk_ap;
                    let (sneak, b177, alt) = if detector {
                        (0, 1, 0)
                    } else {
                        (self.wd().key_shift as i32, 0, self.wd().key_alt as i32)
                    };
                    self.lib(LibCall::Move { slot: s as i32, tile, marker: 1, sneak, b177, alt, walk: Some(walk), squad: 0 });
                } else {
                    // SAFETY: paths = 48 x 80 bytes.
                    unsafe { std::ptr::copy_nonoverlapping(save.as_ptr(), self.wd().paths.add(80 * s), 80) };
                }
                if detector && fire {
                    self.st.ap = 0; // (the end of "Fire")
                }
            }
        }
        if out.wait.is_none() {
            self.range_test();
        }
        out
    }
}

/// Exact integer square root (rounded down) from a float guess.
fn isqrt_fix(n: i64, mut g: i64) -> i64 {
    while g * g > n {
        g -= 1;
    }
    while (g + 1) * (g + 1) <= n {
        g += 1;
    }
    g
}

#[cfg(test)]
mod tests {
    use super::*;

    struct Fixture {
        w: Box<Ja1World>,
        soldiers: Box<[u8]>,
        items: Box<[u8]>,
        h: Ja1Host,
    }

    unsafe extern "C" fn dummy_random(_ctx: *mut std::ffi::c_void, _n: i32) -> i32 {
        0
    }

    unsafe extern "C" fn dummy_call(_ctx: *mut std::ffi::c_void, _op: i32, _args: *const i32, _nargs: i32,
                                    _out: *mut u8, _out_size: i32) -> u32 {
        0
    }

    /// The weapon cursor's animation word with an index of 12 or more reads the next soldier record, as the
    /// original 0x1C3D1 does (no bound); past the last record it reads 0. Ledger row ja1front:7190.
    #[test]
    fn anim_word_reads_next_record() {
        let mut f = fixture();
        *f.sol(2, 0x183) = 20;
        *f.sol(2, 2) = 10;
        *f.sol(2, 0) = 7;
        *f.item(7, 2) = 0x55;
        *f.sol(2, 0x1F3) = 12; // word at +0x200 = bytes 0..1 of record 3
        *f.sol(3, 0) = 5;
        let mut st = CursorState::default();
        weapon_cursor_state(&mut st, &mut *f.w, 2);
        assert_eq!(st.value, 0x55);
        *f.sol(3, 0) = 3;
        weapon_cursor_state(&mut st, &mut *f.w, 2);
        assert_eq!(st.value, 0xA1);
        *f.sol(2, 0x1F3) = 13; // +0x202 = bytes 2..3 of record 3
        *f.sol(3, 2) = 4;
        weapon_cursor_state(&mut st, &mut *f.w, 2);
        assert_eq!(st.value, 0x55);
        // Record 47: past the 48 records reads 0.
        *f.sol(47, 0x183) = 20;
        *f.sol(47, 2) = 10;
        *f.sol(47, 0) = 7;
        *f.sol(47, 0x1F3) = 12;
        weapon_cursor_state(&mut st, &mut *f.w, 47);
        assert_eq!(st.value, 0xA1);
    }

    fn fixture() -> Fixture {
        let mut soldiers = vec![0u8; 48 * REC].into_boxed_slice();
        let items = vec![0u8; 256 * 12].into_boxed_slice();
        // SAFETY: Ja1World is plain C data (pointers and integers); zero means null and 0.
        let mut w: Box<Ja1World> = unsafe { Box::new(std::mem::zeroed()) };
        w.soldiers = soldiers.as_mut_ptr();
        w.item_types = items.as_ptr();
        let h = Ja1Host { ctx: std::ptr::null_mut(), random: dummy_random, call: dummy_call, host_ops: 0 };
        Fixture { w, soldiers, items, h }
    }

    impl Fixture {
        fn sol(&mut self, slot: usize, off: usize) -> &mut u8 {
            &mut self.soldiers[slot * REC + off]
        }
        fn item(&mut self, ty: usize, k: usize) -> &mut u8 {
            &mut self.items[12 * ty + k]
        }
    }

    #[test]
    fn cursor_mode_set_branches() {
        // CursorModeSet 0x1C438 / MoveCursorState 0x1C464 / WeaponCursor 0x1C39C: every branch.
        let mut f = fixture();
        let mut st = CursorState::default();
        let panel = |f: &mut Fixture, st: &mut CursorState, slot: i32, mode: i32| {
            click_panel(st, f.w.as_mut(), &f.h, slot, mode);
        };
        // mode 0, hp != 0: 0xA1 and the mode goes to 1.
        *f.sol(0, 0x183) = 20;
        st.ap = 123;
        st.tile = 0x999;
        panel(&mut f, &mut st, 0, 0);
        assert_eq!((st.value, st.mode), (0xA1, 1));
        assert_eq!((st.ap, st.tile), (0, 0xC41));
        // mode 0, hp 0, team slot 3: 0x8C, mode stays 0.
        *f.sol(0, 0x183) = 0;
        *f.sol(0, 0x17A) = 3;
        st = CursorState::default();
        st.value = 0x77;
        panel(&mut f, &mut st, 0, 0);
        assert_eq!((st.value, st.mode), (0x8C, 0));
        // mode 0, hp 0, team slot 9: no change.
        *f.sol(0, 0x17A) = 9;
        st = CursorState::default();
        st.mode = 0;
        st.value = 0x77;
        panel(&mut f, &mut st, 0, 0);
        assert_eq!((st.value, st.mode), (0x77, 0));
        // mode 1: hp 15+ 0x8E, hp 1..14 0xA1, hp 0 0x8C with mode 0.
        st = CursorState::default();
        *f.sol(1, 0x183) = 15;
        panel(&mut f, &mut st, 1, 1);
        assert_eq!((st.value, st.mode), (0x8E, 1));
        *f.sol(1, 0x183) = 10;
        st.value = 0;
        panel(&mut f, &mut st, 1, 1);
        assert_eq!((st.value, st.mode), (0xA1, 1));
        *f.sol(1, 0x183) = 0;
        *f.sol(1, 0x17A) = 2;
        panel(&mut f, &mut st, 1, 1);
        assert_eq!((st.value, st.mode), (0x8C, 0));
        // mode 2: the weapon cursor (hp 15+, not in water, anim > 3, status > 9) and its item picture.
        *f.sol(2, 0x183) = 20;
        *f.sol(2, 2) = 10;
        *f.sol(2, 0) = 7;
        *f.sol(2, 0x1F3) = 2;
        *f.sol(2, 0x1E8 + 4) = 5;
        *f.item(7, 2) = 0x55;
        st = CursorState::default();
        panel(&mut f, &mut st, 2, 2);
        assert_eq!((st.value, st.mode), (0x55, 2));
        // mode 2 fallback: a water tile gives 0xA1.
        *f.sol(2, 0x1B2) = 0x6C;
        *f.sol(2, 0x1B4) = 0;
        st.value = 0;
        panel(&mut f, &mut st, 2, 2);
        assert_eq!(st.value, 0xA1);
        // mode 3: 0x94 (MoveCursorState direct; click_panel clamps every mode above 2 to 1).
        st = CursorState::default();
        st.mode = 3;
        move_cursor_state(&mut st, f.w.as_mut(), &f.h, 3);
        assert_eq!((st.value, st.mode), (0x94, 3));
        // a mode above 2 becomes 1: 0x8E for hp 15+.
        st = CursorState::default();
        *f.sol(4, 0x183) = 15;
        panel(&mut f, &mut st, 4, 7);
        assert_eq!((st.value, st.mode), (0x8E, 1));
        // a slot outside 0..48 sets the mode only; the cursor state is untouched.
        st = CursorState::default();
        st.ap = 123;
        st.tile = 0x999;
        panel(&mut f, &mut st, 48, 3);
        assert_eq!((st.value, st.mode, st.ap, st.tile), (0x8E, 1, 123, 0x999));
    }

    /// A one-row font: glyph 'A' 4 wide, 'B' 1 wide, every sheet pixel 1 (drawn as table0[1] = 7).
    fn tiny_font() -> Font {
        let mut w = [0u8; 131];
        (w[33], w[34]) = (4, 1);
        Font { sheet: vec![1; 5], pitch: 5, height: 1, widths: Box::leak(Box::new(w)) }
    }

    /// Ledger rows ja1front:6407, ja1front:6425, c2aea50:ja1front:6926 (panel.md "Text", clip width = PITCH): a glyph
    /// that passes the right edge is cut there (nothing wraps to the next row), the text stops once x passes
    /// PITCH - 1 (a glyph that starts at PITCH - 1 is still drawn), and pixels outside the buffer are dropped.
    #[test]
    fn text_clips_at_the_right_edge_and_buffer() {
        let f = tiny_font();
        let mut t0 = [0u8; 256];
        t0[1] = 7;
        let mut buf = vec![0u8; PITCH as usize * 2];
        text(&mut buf, &f, &mut t0, b"A", PITCH - 2, 0, false, 0, 0, false);
        assert_eq!(&buf[PITCH as usize - 2..PITCH as usize + 2], &[7, 7, 0, 0]);
        let mut buf = vec![0u8; PITCH as usize * 2];
        text(&mut buf, &f, &mut t0, b"AB", PITCH - 5, 0, false, 0, 0, false);
        assert_eq!(&buf[PITCH as usize - 5..PITCH as usize + 1], &[7, 7, 7, 7, 7, 0]);
        let mut buf = vec![0u8; PITCH as usize];
        text(&mut buf, &f, &mut t0, b"A", 0, -1, false, 0, 0, false);
        text(&mut buf, &f, &mut t0, b"A", 0, 1, false, 0, 0, false);
        assert!(buf.iter().all(|&b| b == 0));
    }

    /// Ledger rows ja1front:6720/6721/6723 (cursor.md "Mouse off the map", TacticalKeyHandler 0x1517C): only a
    /// cursor that was over the map is reset; the label clears for a lit slot below 0x40 and the lit slot goes to
    /// 0x40 only for a merc slot below 8.
    #[test]
    fn off_map_resets_only_from_the_map() {
        let mut f = fixture();
        let (w, h) = (f.w.as_mut() as *mut Ja1World, &f.h as *const Ja1Host);
        let mut st = CursorState::default();
        (st.over_map, st.tile, st.label[0]) = (0, 0x999, 5);
        let mut hs = HoverState { lit: 3, ..HoverState::default() };
        off_map(&mut st, &mut hs, w, h);
        assert_eq!((st.tile, st.label[0], hs.lit), (0x999, 5, 3));
        for (lit, label, lit_after) in [(3u8, 0u8, 0x40u8), (0x20, 0, 0x20), (0x40, 5, 0x40), (8, 0, 8)] {
            (st.over_map, st.tile, st.label[0]) = (1, 0x999, 5);
            hs.lit = lit;
            off_map(&mut st, &mut hs, w, h);
            assert_eq!((st.tile, st.over_map, st.label[0], hs.lit, hs.named), (0xC41, 0, label, lit_after, 0x40), "lit {lit}");
        }
    }

    /// Ledger row c2aea50:ja1front:7201 (SlotMouse 0x118EA asks OnScreen 0x1FC34 only for merc slots): slot 8 and
    /// up is never on screen; with the on-screen option off (zero world) every merc slot is.
    #[test]
    fn slot_on_screen_needs_a_merc_slot() {
        let mut f = fixture();
        let (w, h) = (f.w.as_mut() as *mut Ja1World, &f.h as *const Ja1Host);
        assert!(slot_on_screen(w, h, 0));
        assert!(!slot_on_screen(w, h, 8));
    }

    fn hover<'a>(f: &'a mut Fixture, st: &'a mut CursorState, hs: &'a mut HoverState, gv: &'a [u8; 256]) -> Hover<'a> {
        Hover { w: f.w.as_mut(), h: &f.h, st, hs, gv_t3: gv, shift: false, alt: false, confirm_move: false,
                fast_fire: false, drag: 0, squad: 0 }
    }

    /// Ledger rows ja1front:6808, ja1front:6823, ja1front:6856: the hover step hands its lit and named slots to the
    /// soldier texts when it ends; a tile byte off the map or with no tile table reads 0; a ground item counts only
    /// with a type and a positive byte 6.
    #[test]
    fn hover_end_tile_bytes_and_ground_items() {
        let mut f = fixture();
        let mut tiles = vec![0u8; 3136 * 8];
        let mut ground = vec![0u8; 200 * 12];
        let (mut st, mut hs, gv) = (CursorState::default(), HoverState { lit: 3, named: 5, ..HoverState::default() }, [0u8; 256]);
        crate::enter::label_slots(0x40, 0x40);
        drop(hover(&mut f, &mut st, &mut hs, &gv));
        assert_eq!(crate::enter::lit_slot(), 3);
        {
            let hv = hover(&mut f, &mut st, &mut hs, &gv);
            assert_eq!(hv.tb(5, 0), 0);
        }
        tiles[5 * 8 + 6] = 2;
        tiles[6 * 8 + 6] = 3;
        (ground[24], ground[30]) = (1, 0);
        (ground[36], ground[42]) = (1, 1);
        f.w.tiles = tiles.as_mut_ptr();
        f.w.ground_items = ground.as_mut_ptr();
        let hv = hover(&mut f, &mut st, &mut hs, &gv);
        assert_eq!((hv.tb(-1, 6), hv.tb(3136, 6), hv.tb(5, 6)), (0, 0, 2));
        assert!(!hv.ground_item(5));
        assert!(hv.ground_item(6));
    }

    /// Ledger rows c2aea50:ja1front:7772, 0b4d529:ja1front:7765 (panel.md "The overview mouse step"): a step that
    /// neither moves to a new tile nor works out a cost keeps the cursor's AP number.
    #[test]
    fn overview_step_keeps_the_ap_number() {
        let mut f = fixture();
        *f.sol(0, 0x1B8) = 1;
        let (mut st, mut hs, gv) = (CursorState::default(), HoverState::default(), [0u8; 256]);
        let (x, y) = (48 + 4 * 10, 14 + 3 * 2);
        st.tile = crate::viewmode::overview_tile(x, y);
        st.ap = 7;
        let mut ov = crate::viewmode::OvMouse { cost: 99, ..Default::default() };
        let mut hv = hover(&mut f, &mut st, &mut hs, &gv);
        hv.overview(&mut ov, x, y, 0);
        drop(hv);
        assert_eq!(st.ap, 7);
    }
}
