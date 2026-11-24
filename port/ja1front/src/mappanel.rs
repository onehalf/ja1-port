//! The map panel of the strategic screen (docs/spec/strategic.md "The map panel").

use crate::invdraw::{inv_data, load_pcx};
use crate::panel::{self, Painter};
use crate::host::Ja1World;

/// Tabs 0..3: (x0, y0, x1, y1).
pub const TABS: [(i32, i32, i32, i32); 4] = [(22, 155, 57, 164), (76, 168, 111, 177), (76, 155, 111, 164), (190, 155, 225, 164)];
/// Number field boxes 0..4: (box x, box y); the mouse is in one when x..x+34, y..y+8.
pub const BOXES: [(i32, i32); 5] = [(22, 180), (138, 162), (138, 180), (233, 162), (233, 180)];
/// The back button of the finance page: (x, y) and its mouse rectangle (x0, y0, x1, y1).
pub const BACK: (i32, i32) = (74, 153);
pub const BACK_RECT: (i32, i32, i32, i32) = (74, 153, 99, 168);
/// The native wage box rectangle.
pub const WAGE_BOX: (i32, i32, i32, i32) = (138, 162, 172, 170);

/// Where a number field takes its value.
#[derive(Clone, Copy)]
enum Val {
    Acc(usize),
    OnDuty,
    /// `prev_balance` (payroll field 49).
    Prev,
}

struct Field {
    val: Val,
    x: i32,
    y: i32,
    digits: usize,
    /// None = the money colour.
    fg: Option<u8>,
    shadow: u8,
    bg: i32,
}

const fn f(val: Val, x: i32, y: i32, digits: usize, fg: Option<u8>, shadow: u8) -> Field {
    Field { val, x, y, digits, fg, shadow, bg: 13 }
}

const fn g(off: usize, x: i32, y: i32, fg: Option<u8>, shadow: u8) -> Field {
    Field { val: Val::Acc(off), x, y, digits: 6, fg, shadow, bg: 300 }
}

/// Number fields 16..25 of the finance page (the money colour without the g_balanceAfterDay test).
const FIN_FIELDS: [Field; 10] = [
    g(0x10, 77, 173, Some(50), 0),
    g(0x20, 77, 184, Some(50), 0),
    g(0x38, 177, 153, Some(121), 12),
    g(0x58, 177, 164, Some(121), 12),
    g(0x5C, 177, 175, Some(121), 12),
    g(0x64, 177, 186, Some(121), 12),
    g(0x04, 280, 153, Some(50), 0),
    g(0x24, 280, 164, Some(50), 0),
    g(0x28, 280, 175, None, 0),
    g(0x2C, 280, 186, None, 0),
];

/// Number fields 27..30 of the assignment screen (field 30: the money colour with the g_balanceAfterDay test).
const ASSIGN_FIELDS: [Field; 4] = [
    f(Val::Acc(0x30), 161, 164, 6, Some(121), 12),
    f(Val::Acc(0x34), 161, 182, 6, Some(121), 12),
    f(Val::Acc(0x38), 217, 164, 6, Some(121), 12),
    f(Val::Acc(0x28), 217, 182, 6, None, 0),
];

/// Payroll fields 43..51 (strategic.md "The payroll screen") and 52..53 (the blink; the money colour without the g_balanceAfterDay test).
const PAY_FIELDS: [Field; 11] = [
    Field { val: Val::Acc(0x1C), x: 78, y: 163, digits: 3, fg: Some(50), shadow: 0, bg: 300 },
    g(0x10, 66, 174, Some(50), 0),
    g(0x20, 66, 185, Some(50), 0),
    g(0x60, 150, 163, Some(121), 0),
    g(0x38, 150, 174, Some(121), 0),
    g(0x64, 150, 185, Some(121), 0),
    Field { val: Val::Prev, x: 241, y: 163, digits: 6, fg: None, shadow: 0, bg: 300 },
    g(0x24, 241, 174, None, 0),
    g(0x2C, 241, 185, None, 0),
    // 52, 53: the blink of "Not enough funds" (strategic.md): fields 47 and 50 in colour 41.
    g(0x38, 150, 174, Some(41), 0),
    g(0x24, 241, 174, Some(41), 0),
];

/// Number fields 0..15, then 16 = the pressed wage number.
const FIELDS: [Field; 17] = [
    f(Val::Acc(0x38), 30, 182, 6, Some(121), 12),
    f(Val::Acc(0x54), 146, 164, 6, Some(121), 12),
    f(Val::Acc(0x60), 146, 182, 6, Some(121), 12),
    f(Val::Acc(0x20), 241, 164, 6, Some(50), 12),
    f(Val::Acc(0x28), 241, 182, 6, None, 12),
    f(Val::Acc(0x48), 78, 182, 3, Some(39), 9),
    f(Val::Acc(0x3C), 97, 182, 3, Some(39), 9),
    f(Val::Acc(0x14), 192, 182, 3, Some(39), 9),
    f(Val::Acc(0x18), 211, 182, 3, Some(39), 9),
    f(Val::OnDuty, 32, 169, 1, Some(39), 9),
    f(Val::Acc(0x08), 43, 169, 1, Some(39), 9),
    f(Val::Acc(0x40), 120, 170, 3, Some(39), 9),
    f(Val::Acc(0x44), 120, 157, 3, Some(39), 9),
    f(Val::Acc(0x1C), 192, 169, 3, Some(39), 9),
    f(Val::Acc(0x84), 211, 169, 3, Some(39), 9),
    f(Val::Acc(0x70), 290, 165, 2, Some(29), 50),
    Field { val: Val::Acc(0x54), x: 145, y: 164, digits: 6, fg: Some(114), shadow: 89, bg: 56 },
];

fn acc_i32(acc: &[u8], off: usize) -> i32 {
    i32::from_le_bytes([acc[off], acc[off + 1], acc[off + 2], acc[off + 3]])
}

/// Number field k (0..16) on the painter's buffer.
pub fn field(p: &mut Painter, acc: &[u8], on_duty: i32, k: usize) {
    draw_field(p, acc, on_duty, &FIELDS[k], k == 4, None);
}

/// Number field k (16..25) of the finance page.
pub fn fin_field(p: &mut Painter, acc: &[u8], k: usize) {
    draw_field(p, acc, 0, &FIN_FIELDS[k - 16], false, None);
}

/// Number field k (27..30) of the assignment screen.
pub fn assign_field(p: &mut Painter, acc: &[u8], k: usize) {
    draw_field(p, acc, 0, &ASSIGN_FIELDS[k - 27], k == 30, None);
}

/// Payroll field k (43..53); `prev` = `prev_balance`.
pub fn pay_field(p: &mut Painter, acc: &[u8], prev: i32, k: usize) {
    draw_field(p, acc, prev, &PAY_FIELDS[k - 43], false, None);
}

/// Number fields 31..42 of the replay screen's panel (strategic.md "The day-end accounts": DrawPanelPccFields
/// (POSTPAN.PCC, 31, 42) in ReplayScreenInit?): 3 digits, shadow 0, no background; the day's values (Val::Prev)
/// come from `day`.
const REPLAY_FIELDS: [Field; 12] = [
    Field { val: Val::Prev, x: 78, y: 161, digits: 3, fg: Some(50), shadow: 0, bg: 300 },
    Field { val: Val::Prev, x: 78, y: 173, digits: 3, fg: Some(50), shadow: 0, bg: 300 },
    Field { val: Val::Prev, x: 78, y: 185, digits: 3, fg: Some(50), shadow: 0, bg: 300 },
    Field { val: Val::Acc(0x00), x: 108, y: 161, digits: 3, fg: Some(50), shadow: 0, bg: 300 },
    Field { val: Val::Acc(0x50), x: 108, y: 173, digits: 3, fg: Some(50), shadow: 0, bg: 300 },
    Field { val: Val::Acc(0x6C), x: 108, y: 185, digits: 3, fg: Some(50), shadow: 0, bg: 300 },
    Field { val: Val::Acc(0x74), x: 189, y: 161, digits: 3, fg: Some(50), shadow: 0, bg: 300 },
    Field { val: Val::Acc(0x18), x: 189, y: 173, digits: 3, fg: Some(50), shadow: 0, bg: 300 },
    Field { val: Val::Acc(0x14), x: 189, y: 185, digits: 3, fg: Some(50), shadow: 0, bg: 300 },
    Field { val: Val::Prev, x: 219, y: 161, digits: 3, fg: None, shadow: 0, bg: 300 },
    Field { val: Val::Prev, x: 219, y: 173, digits: 3, fg: None, shadow: 0, bg: 300 },
    Field { val: Val::Prev, x: 219, y: 185, digits: 3, fg: None, shadow: 0, bg: 300 },
];

/// The day values of the replay panel (ReplayScreenInit? 0x64274): `v` = dead mercs, dead allies, dead npcs, enemy
/// deaths, the day-start copies of g_sectorsOwned, g_treesOwned, g_treesWorkable; `acc` after the day-end accounts.
pub fn replay_day(acc: &[u8], v: [i32; 7]) -> [i32; 6] {
    [v[0], v[1].wrapping_add(v[2]), v[3], acc_i32(acc, 0x74).wrapping_sub(v[4]), acc_i32(acc, 0x18).wrapping_sub(v[5]),
     acc_i32(acc, 0x14).wrapping_sub(v[6])]
}

/// Replay panel fields 31..42; `day` = the values of fields 31, 32, 33, 40, 41, 42 (`dead_mercs`, `dead_allies` +
/// `dead_npc`, `enemy_deaths`, the day's changes of g_sectorsOwned, g_treesOwned, g_treesWorkable).
pub fn replay_fields(p: &mut Painter, acc: &[u8], day: [i32; 6]) {
    for (i, fd) in REPLAY_FIELDS.iter().enumerate() {
        let v = match i {
            0..=2 => day[i],
            9..=11 => day[i - 6],
            _ => 0,
        };
        draw_field(p, acc, v, fd, false, None);
    }
}

/// `other` = the value of Val::OnDuty / Val::Prev; `test` = the g_balanceAfterDay < g_balance test of the money
/// colour (field 4); `bg` overrides the background.
fn draw_field(p: &mut Painter, acc: &[u8], other: i32, fd: &Field, test: bool, bg: Option<i32>) {
    let v = match fd.val {
        Val::Acc(o) => acc_i32(acc, o),
        Val::OnDuty | Val::Prev => other,
    };
    let fg = fd.fg.unwrap_or(if v < 0 || (test && acc_i32(acc, 0x28) < acc_i32(acc, 0x24)) { 0x79 } else { 0x32 });
    let mut s = v.to_string().into_bytes();
    while s.len() < fd.digits {
        s.insert(0, b'^');
    }
    p.text_mode(&s, fd.x, fd.y, false, fg, fd.shadow, 0, bg.unwrap_or(fd.bg));
}

/// The box of number field k (0..4): pressed = element 38 + the field without background, else element 39 + the
/// field.
pub fn field_box(p: &mut Painter, acc: &[u8], on_duty: i32, k: usize, pressed: bool) {
    let (x, y) = BOXES[k];
    p.element(if pressed { 38 } else { 39 }, x, y, None);
    draw_field(p, acc, on_duty, &FIELDS[k], k == 4, if pressed { Some(300) } else { None });
}

/// The finance page: FINANCE.PCC at (0, 146), then number fields 16..25.
pub fn finance_page(p: &mut Painter, main: &crate::invdraw::InvData, acc: &[u8]) {
    load_pcx(p, main, "FINANCE.PCC", 0, 146);
    for k in 16..26 {
        fin_field(p, acc, k);
    }
}

/// MAPFLIP1..3.PCC (n = 1..3) at (0, 146).
pub fn flip(p: &mut Painter, main: &crate::invdraw::InvData, n: usize) {
    load_pcx(p, main, &format!("MAPFLIP{}.PCC", n), 0, 146);
}

/// The back button of the finance page, up or pressed.
pub fn back_button(p: &mut Painter, pressed: bool) {
    back_button_at(p, BACK, pressed);
}

/// A button with the back button's faces at (x, y) (the assignment screen's exit button 9 too).
pub fn back_button_at(p: &mut Painter, (x, y): (i32, i32), pressed: bool) {
    if pressed {
        p.element(21, x, y, None);
        p.element(58, x + 4, y + 3, None);
    } else {
        p.element(20, x, y, None);
        p.element(58, x + 5, y + 4, None);
    }
}

/// The processing plants, steps 1 (raise tab 3) .. 3; `connected` = Connected to home of a sector byte.
pub fn processing(p: &mut Painter, m: &crate::island::MapData, acc: &[u8], plants: &[u8], sectors: &[u8],
                  connected: &dyn Fn(u8) -> bool) {
    let (x0, y0, _, _) = TABS[3];
    p.element(37, x0, y0, None);
    let mut remaining = acc[0x1C];
    for k in 0..4 {
        let e = &plants[k * 12..k * 12 + 12];
        let (cap, sector, state) = (acc_i32(e, 0), acc_i32(e, 4), e[8]);
        let used = if !connected(sector as u8) || state != 2 {
            0
        } else if cap <= remaining as i32 {
            cap
        } else {
            remaining as i32
        };
        remaining = remaining.wrapping_sub(used as u8);
        // SPEC GAP: a sector outside 0..59 draws nothing.
        if !(0..60).contains(&sector) {
            continue;
        }
        let s = sector as usize;
        crate::island::cell(m, sectors[s * 9], s, p.buf);
        let (cx, row) = (sector % 10 * 31, sector / 10 * 23);
        p.element(if state == 2 { 114 } else { 115 }, cx + 7, row + 5, None);
        let y = row + 17;
        p.number2(used, cx + 5, y, 0xFF, 1, 1);
        p.text(b"/", cx + 17, y, false, 0xFF, 1, 1);
        p.number2(cap, cx + 23, y, 0xFF, 1, 1);
    }
    let t = format!("Total processing plant usage is  {}/{}", acc_i32(acc, 0x1C), acc_i32(acc, 0x84));
    p.text(t.as_bytes(), 159, 18, true, 0xFF, 1, 2);
}

/// Raise tab m (0..2): element 34 + m at the tab's (x0, y0). Returns the 36 x 10 pixels it covered.
pub fn raise_tab(p: &mut Painter, m: usize) -> Vec<u8> {
    let (x0, y0, _, _) = TABS[m];
    let mut keep = Vec::with_capacity(360);
    for r in 0..10 {
        for c in 0..36 {
            keep.push(p.get(x0 + c, y0 + r).unwrap_or(0));
        }
    }
    p.element(34 + m, x0, y0, None);
    keep
}

/// Lower tab m: put back the pixels `raise_tab` returned.
pub fn lower_tab(p: &mut Painter, m: usize, keep: &[u8]) {
    let (x0, y0, _, _) = TABS[m];
    for r in 0..10 {
        for c in 0..36 {
            p.put(x0 + c, y0 + r, keep[(r * 36 + c) as usize]);
        }
    }
}

/// Drawing the panel (steps 1..3). Returns the pixels under the raised tab.
pub fn draw_panel(p: &mut Painter, main: &crate::invdraw::InvData, acc: &[u8], on_duty: i32, mode: usize) -> Vec<u8> {
    load_pcx(p, main, "MAPPAN.PCC", 0, 146);
    let keep = raise_tab(p, mode);
    for k in 0..15 {
        field(p, acc, on_duty, k);
    }
    keep
}

/// Step 4 of the wage box: element 39 at (138, 162), then number field 1.
pub fn wage_box(p: &mut Painter, acc: &[u8], on_duty: i32) {
    p.element(39, WAGE_BOX.0, WAGE_BOX.1, None);
    field(p, acc, on_duty, 1);
}

/// Step 1 of the wage box (drawing part): element 43 at (138, 162), then the pressed wage number.
pub fn wage_pressed(p: &mut Painter, acc: &[u8], on_duty: i32) {
    p.element(43, WAGE_BOX.0, WAGE_BOX.1, None);
    field(p, acc, on_duty, 16);
}

/// mappanel_diff: the panel with tab `from` raised, then lowered, then tab `to` raised and the fields, as a tab click.
pub fn draw_tab_switch(iso: &str, exe: &str, acc: &[u8], on_duty: i32, from: usize, to: usize, screen: &mut [u8])
                       -> Result<(), i32> {
    if acc.len() < 0xA8 || screen.len() < 64000 || from >= 3 || to >= 3 {
        return Err(crate::E_ARG);
    }
    let (c, d, inv) = (crate::common(iso)?, panel::panel_data(iso, exe)?, inv_data(iso, exe)?);
    let mut table0 = c.remap[0];
    let mut p = Painter { d: &d, buf: &mut screen[..64000], table0: &mut table0, table29: panel::table29(&c) };
    load_pcx(&mut p, &inv, "MAPPAN.PCC", 0, 146);
    let keep = raise_tab(&mut p, from);
    lower_tab(&mut p, from, &keep);
    raise_tab(&mut p, to);
    for k in 0..15 {
        field(&mut p, acc, on_duty, k);
    }
    Ok(())
}

/// The test entry points: `what` = None = the whole panel with map mode `mode`; Some(k) = field k (0..16), 17 =
/// step 4 of the wage box, 18 = the finance page, 19 / 20 = the back button up / pressed, 21 / 22 = the field 4 box
/// pressed / up, 23 / 24 = the same for field 0, 25..27 = MAPFLIP1..3.PCC, 28 + 2 i (+ 1) = the top strip's button i
/// up (pressed).
pub fn draw(iso: &str, exe: &str, acc: &[u8], on_duty: i32, mode: i32, what: Option<i32>, screen: &mut [u8]) -> Result<(), i32> {
    if acc.len() < 0xA8 || screen.len() < 64000 || !(0..3).contains(&mode) {
        return Err(crate::E_ARG);
    }
    let (c, d, inv) = (crate::common(iso)?, panel::panel_data(iso, exe)?, inv_data(iso, exe)?);
    let mut table0 = c.remap[0];
    let mut p = Painter { d: &d, buf: &mut screen[..64000], table0: &mut table0, table29: panel::table29(&c) };
    match what {
        None => {
            draw_panel(&mut p, &inv, acc, on_duty, mode as usize);
        }
        Some(17) => wage_box(&mut p, acc, on_duty),
        Some(18) => finance_page(&mut p, &inv, acc),
        Some(19) => back_button(&mut p, false),
        Some(20) => back_button(&mut p, true),
        Some(21) => field_box(&mut p, acc, on_duty, 4, true),
        Some(22) => field_box(&mut p, acc, on_duty, 4, false),
        Some(23) => field_box(&mut p, acc, on_duty, 0, true),
        Some(24) => field_box(&mut p, acc, on_duty, 0, false),
        Some(k @ 25..=27) => flip(&mut p, &inv, (k - 24) as usize),
        Some(k @ 28..=39) => crate::strips::button_face(&mut p, ((k - 28) / 2) as usize, (k - 28) % 2 == 1),
        Some(k) if (0..17).contains(&k) => field(&mut p, acc, on_duty, k as usize),
        Some(_) => return Err(crate::E_ARG),
    }
    Ok(())
}

extern "C" {
    fn ja1_connected_home(w: *mut Ja1World, h: *const crate::host::Ja1Host, sector: i32) -> i32;
}

/// Connected to home (strategic.md) of sector byte c over `sectors` (60 x 9 bytes), through ja1logic.
pub fn connected_home(sectors: &[u8], c: u8) -> bool {
    let mut tbl = sectors[..540].to_vec();
    // SAFETY: Ja1World is plain pointers and integers (all zero is a valid value); connected_home reads only
    // `sector_state`, which points to `tbl` for the call.
    unsafe {
        let mut w: Ja1World = std::mem::zeroed();
        w.sector_state = tbl.as_mut_ptr();
        ja1_connected_home(&mut w, std::ptr::null(), c as i32) != 0
    }
}

/// The test entry point of The processing plants (steps 1..3 on `screen`).
pub fn draw_processing(iso: &str, exe: &str, acc: &[u8], plants: &[u8], sectors: &[u8], screen: &mut [u8]) -> Result<(), i32> {
    if acc.len() < 0xA8 || plants.len() < 48 || sectors.len() < 540 || screen.len() < 64000 {
        return Err(crate::E_ARG);
    }
    let (c, d, m) = (crate::common(iso)?, panel::panel_data(iso, exe)?, crate::island::map_data(iso, exe)?);
    let mut table0 = c.remap[0];
    let mut p = Painter { d: &d, buf: &mut screen[..64000], table0: &mut table0, table29: panel::table29(&c) };
    processing(&mut p, &m, acc, plants, sectors, &|b| connected_home(sectors, b));
    Ok(())
}

#[cfg(test)]
mod arg_tests {
    use super::*;

    /// The test entry points check their buffers before any file is read: a short buffer is E_ARG.
    #[test]
    fn short_buffers_are_e_arg() {
        let (acc, short_acc) = (vec![0u8; 0xA8], vec![0u8; 0xA7]);
        let (mut scr, mut short_scr) = (vec![0u8; 64000], vec![0u8; 63999]);
        assert_eq!(draw_tab_switch("", "", &acc, 0, 0, 1, &mut short_scr), Err(crate::E_ARG));
        assert_eq!(draw_tab_switch("", "", &short_acc, 0, 0, 1, &mut scr), Err(crate::E_ARG));
        assert_eq!(draw("", "", &acc, 0, 0, None, &mut short_scr), Err(crate::E_ARG));
        assert_eq!(draw("", "", &short_acc, 0, 0, None, &mut scr), Err(crate::E_ARG));
        let (pl, se) = (vec![0u8; 48], vec![0u8; 540]);
        assert_eq!(draw_processing("", "", &short_acc, &pl, &se, &mut scr), Err(crate::E_ARG));
        assert_eq!(draw_processing("", "", &acc, &pl, &se, &mut short_scr), Err(crate::E_ARG));
        assert_ne!(draw_processing("", "", &acc, &pl, &se, &mut scr), Err(crate::E_ARG));
    }
}
