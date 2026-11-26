//! The A.I.M. grid (#42, Insert #13454; port mod, not in the original, setting `aim_browser`, default off): all
//! mercs on pages of 6 x 4 faces, with a sort and a filter. This file holds the list rules (which mercs, in which
//! order, on which page); the view state stays here and never goes into the world data, so saves stay the same.
//! The merc page, the interview, hiring, the quotes, EXIT and INFO stay the code of aim.rs.

/// Profile record size and the fields the grid reads (the A.I.M. page of aim.rs page_draw reads the same ones).
const PROFILE: usize = 0x5A;
const SOLDIER: usize = 0x200;
const P_SALARY: usize = 0x28;
const P_HEALTH: usize = 0x2A;
const P_WISDOM: usize = 0x2E;
const P_HIRE_ANYWAY: usize = 0x2F;
const P_TEAM_STATUS: usize = 0x4B;

pub const COLS: usize = 6;
pub const ROWS: usize = 4;
pub const PER: usize = COLS * ROWS;
/// The cell size and the top left corner of the grid (the mockup's layout).
pub const CW: i32 = 38;
pub const CH: i32 = 38;
pub const GX: i32 = 4;
pub const GY: i32 = 16;

/// The sort keys: the header text, the profile byte (None = number, name or salary) and whether the first order
/// is high to low.
pub const SORTS: [(&str, Key, bool); 12] = [
    ("NUMBER", Key::Number, false), ("NAME", Key::Name, false), ("SALARY", Key::Salary, false),
    ("HEALTH", Key::Byte(0x2A), true), ("AGILITY", Key::Byte(0x2C), true), ("DEXTERITY", Key::Byte(0x42), true),
    ("WISDOM", Key::Byte(0x2E), true), ("MEDICAL", Key::Byte(0x40), true), ("EXPLOSIVES", Key::Byte(0x43), true),
    ("MECHANICAL", Key::Byte(0x41), true), ("MARKSMANSHIP", Key::Byte(0x2D), true), ("EXP. CLASS", Key::Byte(0x44), true),
];
pub const FILTERS: [&str; 3] = ["ALL", "NOT AWAY", "CAN HIRE"];

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Key {
    Number,
    Name,
    Salary,
    Byte(usize),
}

/// What an interview with the merc gives now (the refusal reasons of MercRefusalReason 0x219CC, read without the
/// voice remark: ja1_aim_refusal_rule).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Hire {
    /// Dead, on assignment or on the team: the merc page shows the face dark.
    Away,
    /// Reason 0, or reason 8 with the "hires anyway" byte 3 (aim.rs interview hires then).
    Yes,
    /// Reason 6 (asks for more money) or 12 (old debt to pay first): a hire after an offer; the cell has a mark.
    Offer,
    /// Any other reason.
    No,
}

/// The kind of profile p: `reason` = the refusal reason of the rule (only read when the merc is not away).
pub fn hire_kind(profiles: &[u8], soldiers: &[u8], p: usize, reason: impl Fn(usize) -> i32) -> Hire {
    let pr = &profiles[p * PROFILE..(p + 1) * PROFILE];
    if pr[P_HEALTH] == 0 || pr[P_TEAM_STATUS] != 0 || on_team(soldiers, p) {
        return Hire::Away;
    }
    match reason(p) {
        0 => Hire::Yes,
        8 if pr[P_HIRE_ANYWAY] == 3 => Hire::Yes,
        6 | 12 => Hire::Offer,
        _ => Hire::No,
    }
}

/// A team record (+0x182 in the game) holds profile p (+0x179): aim.rs team_slot.
pub fn on_team(soldiers: &[u8], p: usize) -> bool {
    (0..8).any(|k| soldiers[k * SOLDIER + 0x182] != 0 && soldiers[k * SOLDIER + 0x179] as usize == p)
}

/// The nickname of profile p (bytes 0..10, zero ended).
pub fn nick(profiles: &[u8], p: usize) -> &[u8] {
    let b = &profiles[p * PROFILE..p * PROFILE + 10];
    &b[..b.iter().position(|&c| c == 0).unwrap_or(10)]
}

/// The number a cell shows under the name: the salary for the number, name and salary sorts, else the sorted stat.
pub fn cell_value(profiles: &[u8], p: usize, sort: usize) -> i32 {
    let pr = &profiles[p * PROFILE..(p + 1) * PROFILE];
    match SORTS[sort].1 {
        Key::Byte(o) => pr[o] as i32,
        _ => u16::from_le_bytes([pr[P_SALARY], pr[P_SALARY + 1]]) as i32,
    }
}

/// The PANEL_S values of profile p (the team panel's order, aim.rs salary_values): salary, health, agility,
/// dexterity, wisdom, medical, explosives, mechanical, marksmanship, level.
pub fn stats(profiles: &[u8], p: usize) -> [i32; 10] {
    let pr = &profiles[p * PROFILE..(p + 1) * PROFILE];
    let mut v = [0i32; 10];
    v[0] = u16::from_le_bytes([pr[P_SALARY], pr[P_SALARY + 1]]) as i32;
    for (i, o) in [0x2A, 0x2C, 0x42, 0x2E, 0x40, 0x43, 0x41, 0x2D, 0x44].iter().enumerate() {
        v[i + 1] = pr[*o] as i32;
    }
    v
}

/// The view state of the grid (not in the world data).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct View {
    pub sort: usize,
    pub desc: bool,
    pub filter: usize,
    pub page: usize,
}

impl Default for View {
    /// The mockup's start: salary, low to high, all mercs, page 1.
    fn default() -> View {
        View { sort: 2, desc: false, filter: 0, page: 0 }
    }
}

impl View {
    /// A pick in the SORT menu: the same sort again reverses it, a new sort starts at its first order.
    pub fn pick_sort(&mut self, i: usize) {
        if i == self.sort {
            self.desc = !self.desc;
        } else if i < SORTS.len() {
            self.sort = i;
            self.desc = SORTS[i].2;
        }
        self.page = 0;
    }

    pub fn pick_filter(&mut self, i: usize) {
        if i < FILTERS.len() {
            self.filter = i;
        }
        self.page = 0;
    }

    /// The page count of a list of n mercs (at least 1).
    pub fn pages(n: usize) -> usize {
        n.div_ceil(PER).max(1)
    }

    /// PREV (d = -1) / NEXT (d = 1) on the grid: the pages wrap.
    pub fn flip(&mut self, n: usize, d: i32) {
        let k = Self::pages(n) as i32;
        self.page = ((self.page.min(k as usize - 1) as i32 + d).rem_euclid(k)) as usize;
    }

    /// The page that holds profile p in `list` (INDEX goes back to it).
    pub fn show(&mut self, list: &[usize], p: usize) {
        if let Some(i) = list.iter().position(|&q| q == p) {
            self.page = i / PER;
        }
    }
}

/// The mercs of the filter in the order of the sort: every profile 0..59 with wisdom != 0 (as ja1_aim_browse),
/// `kinds[p]` = hire_kind of p. Equal keys keep the number order.
pub fn list(profiles: &[u8], kinds: &[Hire; 60], v: &View) -> Vec<usize> {
    let mut l: Vec<usize> = (0..60)
        .filter(|&p| profiles[p * PROFILE + P_WISDOM] != 0)
        .filter(|&p| match v.filter {
            1 => kinds[p] != Hire::Away,
            2 => matches!(kinds[p], Hire::Yes | Hire::Offer),
            _ => true,
        })
        .collect();
    let key = SORTS[v.sort.min(SORTS.len() - 1)].1;
    l.sort_by(|&a, &b| {
        let o = match key {
            Key::Number => a.cmp(&b),
            Key::Name => nick(profiles, a).to_ascii_uppercase().cmp(&nick(profiles, b).to_ascii_uppercase()),
            _ => cell_value(profiles, a, v.sort).cmp(&cell_value(profiles, b, v.sort)),
        };
        (if v.desc { o.reverse() } else { o }).then(a.cmp(&b))
    });
    l
}

/// The count of each filter (the FILTER menu shows them).
pub fn counts(profiles: &[u8], kinds: &[Hire; 60]) -> [usize; 3] {
    [0, 1, 2].map(|f| list(profiles, kinds, &View { filter: f, ..View::default() }).len())
}

/// The list index under the mouse on the shown page, if a merc is there.
pub fn cell_at(x: i32, y: i32, page: usize, n: usize) -> Option<usize> {
    if x < GX || y < GY {
        return None;
    }
    let (c, r) = (((x - GX) / CW) as usize, ((y - GY) / CH) as usize);
    if c >= COLS || r >= ROWS {
        return None;
    }
    let i = page * PER + r * COLS + c;
    (i < n).then_some(i)
}

/// The merc page's PREV / NEXT with the grid on: the next merc of the current list (wraps); a merc that is not in
/// the list (the filter changed) goes to the first or the last one.
pub fn step(list: &[usize], p: usize, d: i32) -> Option<usize> {
    if list.is_empty() {
        return None;
    }
    let n = list.len() as i32;
    let i = match list.iter().position(|&q| q == p) {
        Some(i) => i as i32 + d,
        None => if d > 0 { 0 } else { n - 1 },
    };
    Some(list[i.rem_euclid(n) as usize])
}

fn fill(p: &mut crate::panel::Painter, x: i32, y: i32, w: i32, h: i32, v: u8) {
    for r in y..y + h {
        for c in x..x + w {
            p.put(c, r, v);
        }
    }
}

fn frame(p: &mut crate::panel::Painter, x: i32, y: i32, w: i32, h: i32, v: u8) {
    fill(p, x, y, w, 1, v);
    fill(p, x, y + h - 1, w, 1, v);
    fill(p, x, y, 1, h, v);
    fill(p, x + w - 1, y, 1, h, v);
}

/// The left part of the screen with the grid on (over the screen picture `scr` that aim.rs redraw put down):
/// the marble of the text panel over the whole merc area, 24 cells (frame, small face or the dead face, darkened when
/// away, TEAM on a team merc (and the mark 0x11 when hired in this visit), OUT on a merc on assignment, a $ for an
/// offer, nickname, value), the
/// sort header with its order arrow, the filter with its count, the page number and the labels EXIT SORT FILTER.
pub fn draw(p: &mut crate::panel::Painter, scr: &(usize, usize, Vec<u8>), dark: &[u8; 256], profiles: &[u8],
            soldiers: &[u8], kinds: &[Hire; 60], v: &View, hover: Option<usize>) {
    let (sw, _, px) = scr;
    // Insert #13517 item 2: the text panel of the merc page (its marble rows 92..169 with the side lines at x 3 and
    // 230) as one panel with its own top line (row 91 of the picture) and bottom line (row 170) ...
    for y in 14..172usize {
        let sy = match y {
            14 => 91,
            171 => 170,
            _ => 92 + (y - 15) % 78,
        };
        for x in 3..234 {
            if let Some(&c) = px.get(sy * sw + x) {
                p.put(x as i32, y as i32, c);
            }
        }
    }
    // ... and the header bar over the stats frame of the merc page (rows 0..13, x 72..233): clean stone of rows
    // 0..9 above that frame, then the double frame of the bottom button strip (its rows 173..197) at this size.
    for y in 0..14usize {
        for x in 72..234 {
            if let Some(&c) = px.get((y % 10) * sw + x) {
                p.put(x as i32, y as i32, c);
            }
        }
    }
    strip_frame(p, 72, 0, 230, 13);
    let l = list(profiles, kinds, v);
    let page = v.page.min(View::pages(l.len()) - 1);
    for (i, &m) in l.iter().enumerate().skip(page * PER).take(PER) {
        let j = (i - page * PER) as i32;
        let (x, y) = (GX + CW * (j % COLS as i32), GY + CH * (j / COLS as i32));
        let (fx, fy) = (x + 5, y + 1);
        let on = hover == Some(m);
        frame(p, fx - 1, fy - 1, 30, 27, if on { 0x28 } else { 0 });
        let pr = &profiles[m * PROFILE..(m + 1) * PROFILE];
        for r in 0..25 {
            for c in 0..28 {
                let v = if pr[P_HEALTH] == 0 {
                    p.d.deadface.get(r * 28 + c).copied().unwrap_or(0)
                } else {
                    p.d.faces.get((29 * (m / 11) + r) * 320 + 28 * (m % 11) + c).copied().unwrap_or(0)
                };
                p.put(fx + c as i32, fy + r as i32, if kinds[m] == Hire::Away { dark[v as usize] } else { v });
            }
        }
        if let Some(k) = (0..8).find(|&k| soldiers[k * SOLDIER + 0x182] != 0 && soldiers[k * SOLDIER + 0x179] as usize == m) {
            if soldiers[k * SOLDIER + 0x19D] != 0 {
                p.element(0x11, fx, fy, None);   // hired in this visit, as aim.rs team
            }
            p.text(b"TEAM", fx + 14, fy + 9, true, 0xFF, 0x32, 1);
        } else if pr[P_TEAM_STATUS] != 0 && pr[P_HEALTH] != 0 {
            p.text(b"OUT", fx + 14, fy + 9, true, 0xFF, 0x32, 1);
        }
        if kinds[m] == Hire::Offer {
            p.text(b"$", fx + 21, fy + 1, false, 0xFF, 0x32, 1);
        }
        p.text(nick(profiles, m), x + 19, fy + 26, true, if on { 0xFF } else { 0x2A }, 0xFE, 0);
        let n = cell_value(profiles, m, v.sort);
        let t = match SORTS[v.sort].1 { Key::Byte(_) => n.to_string(), _ => format!("${}", n) };
        p.text(t.as_bytes(), x + 19, fy + 32, true, 0x27, 0xFE, 0);
    }
    if l.is_empty() {
        p.text(b"NO MERC FITS THIS FILTER", GX + 114, 80, true, 0x2A, 0xFE, 1);
    }
    let head = SORTS[v.sort].0.as_bytes();
    p.text(head, 78, 5, false, 0x2A, 3, 1);
    let ax = 79 + crate::panel::text_width(&p.d.fonts, 1, head);
    for i in 0..4 {
        let k = if v.desc { 3 - i } else { i };
        fill(p, ax + 3 - k, 6 + i, 1 + 2 * k, 1, 0x2A);
    }
    let f = format!("{} {}", FILTERS[v.filter], l.len());
    p.text(f.as_bytes(), 226 - crate::panel::text_width(&p.d.fonts, 1, f.as_bytes()), 5, false, 0x2A, 3, 1);
    p.text(format!("{}/{}", page + 1, View::pages(l.len())).as_bytes(), 190, 185, true, 0x2A, 0xFE, 0);
}

/// The double frame of the bottom button strip of AIMSCRN.PCX (outer rect (3, 173)..(230, 197), inner rect two
/// pixels in) for the outer rect (x0, y0)..(x1, y1): outer top and left 0x2F, bottom and right 0x0E; inner top 0x2B,
/// left 0x0E, bottom 0xBE, right 0x2F.
pub fn strip_frame(p: &mut crate::panel::Painter, x0: i32, y0: i32, x1: i32, y1: i32) {
    fill(p, x0, y0, x1 - x0, 1, 0x2F);
    fill(p, x0, y0, 1, y1 - y0 + 1, 0x2F);
    fill(p, x0 + 1, y1, x1 - x0, 1, 0x0E);
    fill(p, x1, y0 + 1, 1, y1 - y0, 0x0E);
    let (a, b, c, d) = (x0 + 2, y0 + 2, x1 - 2, y1 - 2);
    fill(p, a + 1, b, c - a - 1, 1, 0x2B);
    fill(p, a, b + 1, 1, d - b, 0x0E);
    fill(p, a + 1, d, c - a - 1, 1, 0xBE);
    fill(p, c, b + 1, 1, d - b - 1, 0x2F);
}

/// The grid state of one A.I.M. visit (setting on): the view, the grid shown (else the merc page), the merc under
/// the mouse, the open menu (0 sort, 1 filter) and the hire kinds of the last grid entry.
pub struct Grid {
    pub view: View,
    pub shown: bool,
    pub hover: Option<usize>,
    pub menu: Option<u8>,
    pub kinds: [Hire; 60],
    /// Insert #13517 item 3: the item of the open menu under the mouse.
    pub menu_hover: Option<usize>,
    /// Insert #13517 item 6: the mouse is on the A.I.M. title of the merc page (the link back to the grid).
    pub title_lit: bool,
}

impl Grid {
    /// `aim_browser = on` (JA1_AIM_BROWSER = 1): A.I.M. opens on the grid.
    pub fn from_env() -> Option<Grid> {
        crate::config::read::aim_browser(std::env::var("JA1_AIM_BROWSER").ok().as_deref()).then(Grid::new)
    }

    pub fn new() -> Grid {
        Grid { view: View::default(), shown: true, hover: None, menu: None, kinds: [Hire::No; 60], menu_hover: None,
               title_lit: false }
    }

    /// The list of the current view.
    pub fn list(&self, profiles: &[u8]) -> Vec<usize> {
        list(profiles, &self.kinds, &self.view)
    }
}

/// The bottom buttons of the grid: x, y of EXIT, SORT, FILTER (aim.rs BUTTONS 0..2, 37 x 15).
pub const BUTTON_XY: [(i32, i32); 3] = [(11, 178), (59, 178), (108, 178)];
pub const BUTTON_W: i32 = 37;

/// The text of item i of menu m (0 sort, 1 filter; FILTER items carry their count).
fn menu_text(m: u8, i: usize, counts: [usize; 3]) -> String {
    if m == 0 { SORTS[i].0.to_string() } else { format!("{} ({})", FILTERS[i], counts[i]) }
}

/// Insert #13551 item 3: the room left of the item text inside the menu frame: 6 pixels, the 3 pixel current-item
/// arrow, 2 pixels.
const MENU_ARROW: i32 = 6 + 3 + 2;

/// The menu box of menu m (0 sort, 1 filter): x, y, w, h and the item count. Insert #13517 item 4: one rule from
/// the menu's button (SORT, FILTER): left edge = button left - 3, bottom edge 4 pixels above the button. Insert
/// #13551 item 3: w = the double frame (3 pixels each side) + the arrow room + the widest item text (font 1) + 6
/// pixels; h = 9 rows per item + the frame and 2 pixels above and below the items.
pub fn menu_box(fonts: &[crate::panel::Font; 3], m: u8, counts: [usize; 3]) -> (i32, i32, i32, i32, usize) {
    let n = if m == 0 { SORTS.len() } else { FILTERS.len() };
    let tw = (0..n).map(|i| crate::panel::text_width(fonts, 1, menu_text(m, i, counts).as_bytes())).max().unwrap_or(0);
    let (bx, by) = BUTTON_XY[1 + m as usize];
    let h = n as i32 * 9 + 8;
    (bx - 3, by - 4 - h, 3 + MENU_ARROW + tw + 6 + 3, h, n)
}

/// The item of the open menu at (x, y), or None (a click there closes the menu).
pub fn menu_item(fonts: &[crate::panel::Font; 3], m: u8, counts: [usize; 3], x: i32, y: i32) -> Option<usize> {
    let (bx, by, bw, _, n) = menu_box(fonts, m, counts);
    let i = (y - by - 5).div_euclid(9);
    (bx <= x && x < bx + bw && (0..n as i32).contains(&i)).then_some(i as usize)
}

/// The face colour of the bottom button strip and the header bar (AIMSCRN.PCX rows 177..193: 2235 of the 3689
/// pixels inside the strip's inner frame are 0x2D).
pub const STRIP_FACE: u8 = 0x2D;

/// The colour of a menu item not under the mouse (draw_menu).
pub const MENU_ITEM: u8 = 0x28;

/// The open menu over the grid. Insert #13551 item 2: in the style of the bottom strip: an opaque fill of the strip's
/// face colour 0x2D inside the strip's double frame (strip_frame), so no face shows through. Insert #13517 item 3:
/// the item under the mouse is white (0xFF); the current one has a 3 pixel arrow in front. The other items: 0x2A (the
/// strip's label colour) gives only 2.1:1 contrast on 0x2D, too weak, so they take 0x28 (3.4:1, the original's
/// funds digits and merc name on this screen, 0x215BC).
pub fn draw_menu(p: &mut crate::panel::Painter, m: u8, v: &View, counts: [usize; 3], hover: Option<usize>) {
    let (x, y, w, h, n) = menu_box(&p.d.fonts, m, counts);
    fill(p, x, y, w, h, STRIP_FACE);
    strip_frame(p, x, y, x + w - 1, y + h - 1);
    for i in 0..n {
        let cur = if m == 0 { i == v.sort } else { i == v.filter };
        let (ty, c) = (y + 5 + 9 * i as i32, if hover == Some(i) { 0xFF } else { MENU_ITEM });
        if cur {
            for k in 0..3 {
                fill(p, x + 3 + 6 + k, ty + k, 1, 5 - 2 * k, c);
            }
        }
        p.text(menu_text(m, i, counts).as_bytes(), x + 3 + MENU_ARROW, ty, false, c, 3, 1);
    }
}

/// The dark body colours of the engraved A.I.M. title of AIMSCRN.PCX (x 16..63, y 5..12); its light edges are
/// 0x2B, 0x0D, 0xB3.
const TITLE_BODY: [u8; 6] = [0x2F, 0x30, 0x31, 0x32, 0xBF, 0x03];

/// Insert #13551 item 1: the left arrow in front of the title, engraved like the letters (AIMSCRN.PCX x 16..63): 4
/// columns x 10..13, column k over rows 8-k..9+k (rows 5..12 at the base, the letters' height); the top pixel of a
/// column dark 0x31 (the letters' top-left edge), the bottom pixel light 0x2B (their bottom edge), the base column's
/// inside light 0x0D (their right edge), the rest the dark body 0x2F.
pub fn title_arrow() -> Vec<(i32, i32, u8)> {
    let mut v = Vec::new();
    for k in 0..4 {
        let (top, bot) = (8 - k, 9 + k);
        for y in top..=bot {
            let c = if y == top { 0x31 } else if y == bot { 0x2B } else if k == 3 { 0x0D } else { 0x2F };
            v.push((10 + k, y, c));
        }
    }
    v
}

/// Insert #13517 item 6: the link back to the grid on the merc page = the engraved A.I.M. title (a site logo) with
/// the engraved left arrow in front (title_arrow). Under the mouse the dark body of the arrow and the title is white.
/// `scr` = AIMSCRN.PCX (the title at rest).
pub fn draw_title(p: &mut crate::panel::Painter, scr: &(usize, usize, Vec<u8>), lit: bool) {
    let (sw, _, px) = scr;
    for y in 5..13usize {
        for x in 16..64usize {
            let c = px.get(y * sw + x).copied().unwrap_or(0);
            p.put(x as i32, y as i32, if lit && TITLE_BODY.contains(&c) { 0xFF } else { c });
        }
    }
    for (x, y, c) in title_arrow() {
        p.put(x, y, if lit && TITLE_BODY.contains(&c) { 0xFF } else { c });
    }
}

/// The hit box of the title link (arrow and title).
pub fn on_title(x: i32, y: i32) -> bool {
    (8..65).contains(&x) && (4..14).contains(&y)
}

/// The merc area of the grid (a click there never reaches the merc page's face).
pub fn in_grid(x: i32, y: i32) -> bool {
    (3..234).contains(&x) && (13..172).contains(&y)
}

/// The button labels with the grid on (aim.rs labels draws EXIT INFO GEAR on the merc page).
pub const LABELS: [&str; 3] = ["EXIT", "SORT", "FILTER"];

/// Insert #13517 item 4: the font of a grid button label: font 1 (the merc page's labels) when the label leaves 2
/// pixels of space each side inside the button face (37 wide, a 2 pixel edge each side), else the small font 0.
pub fn label_font(fonts: &[crate::panel::Font; 3], label: &str) -> usize {
    if crate::panel::text_width(fonts, 1, label.as_bytes()) <= BUTTON_W - 2 * (2 + 2) { 1 } else { 0 }
}

/// Item 5: the pressed face of grid button b: the pressed button picture (element 0x13, as aim.rs pressed_button)
/// with the grid's label, so the merc page's words never show.
pub fn draw_pressed_face(p: &mut crate::panel::Painter, b: usize) {
    let (x, y) = BUTTON_XY[b];
    p.element(0x13, x, y, None);
    draw_label(p, b, true);
}

/// A grid button label: at rest centred at (x + 19, 183), fg 0x2A, shadow 3 (aim.rs labels); pressed (item 5) where
/// the original's pressed face puts its label (x + 18, 182), fg 0x2B, shadow 2. Font 0 one row lower (its glyphs are
/// one row shorter).
pub fn draw_label(p: &mut crate::panel::Painter, b: usize, pressed: bool) {
    let label = LABELS[b];
    let f = label_font(&p.d.fonts, label);
    let (bx, _) = BUTTON_XY[b];
    let (x, y, fg, sh) = if pressed { (bx + 18, 182, 0x2B, 2) } else { (bx + 19, 183, 0x2A, 3) };
    p.text(label.as_bytes(), x, y + (f == 0) as i32, true, fg, sh, f);
}

#[cfg(test)]
mod tests {
    use super::*;

    /// 60 profiles: wisdom 50 for 0..57 (58, 59 empty), salary 1000 - 10 p, health 60 + p % 7, nick "Mp".
    fn profiles() -> Vec<u8> {
        let mut b = vec![0u8; 60 * PROFILE];
        for p in 0..58 {
            let r = &mut b[p * PROFILE..(p + 1) * PROFILE];
            let n = format!("M{:02}", 57 - p);
            r[..n.len()].copy_from_slice(n.as_bytes());
            r[P_WISDOM] = 50;
            r[P_SALARY..P_SALARY + 2].copy_from_slice(&(1000 - 10 * p as u16).to_le_bytes());
            r[P_HEALTH] = 60 + (p % 7) as u8;
        }
        b
    }

    fn team(ps: &[u8]) -> Vec<u8> {
        let mut s = vec![0u8; 8 * SOLDIER];
        for (k, &p) in ps.iter().enumerate() {
            s[k * SOLDIER + 0x182] = 1;
            s[k * SOLDIER + 0x179] = p;
        }
        s
    }

    #[test]
    fn the_kind_comes_from_the_records_and_the_rule() {
        let mut pr = profiles();
        let s = team(&[3]);
        pr[5 * PROFILE + P_HEALTH] = 0;
        pr[6 * PROFILE + P_TEAM_STATUS] = 2;
        pr[7 * PROFILE + P_HIRE_ANYWAY] = 3;
        let r = |p: usize| match p { 7 | 8 => 8, 9 => 6, 10 => 12, 11 => 1, _ => 0 };
        let k: Vec<Hire> = (0..12).map(|p| hire_kind(&pr, &s, p, r)).collect();
        assert_eq!(k[0], Hire::Yes);
        assert_eq!(&k[3..12], &[Hire::Away, Hire::Yes, Hire::Away, Hire::Away, Hire::Yes, Hire::No, Hire::Offer,
                                Hire::Offer, Hire::No]);
    }

    #[test]
    fn sort_filter_and_pages_follow_the_mockup() {
        let pr = profiles();
        let mut kinds = [Hire::Yes; 60];
        kinds[0] = Hire::Away;
        kinds[1] = Hire::No;
        kinds[2] = Hire::Offer;
        let mut v = View::default();
        let l = list(&pr, &kinds, &v);
        assert_eq!(l.len(), 58, "58 and 59 have no wisdom");
        assert_eq!(&l[..2], &[57, 56], "salary low to high");
        v.pick_sort(2);
        assert!(v.desc);
        assert_eq!(list(&pr, &kinds, &v)[0], 0, "same sort again = high to low");
        v.pick_sort(1);
        assert!(!v.desc);
        assert_eq!(list(&pr, &kinds, &v)[0], 57, "name M00 first");
        v.pick_sort(3);
        assert!(v.desc, "a stat starts high to low");
        let l = list(&pr, &kinds, &v);
        assert_eq!(cell_value(&pr, l[0], 3), 66);
        assert!(l.windows(2).all(|w| cell_value(&pr, w[0], 3) > cell_value(&pr, w[1], 3)
            || (cell_value(&pr, w[0], 3) == cell_value(&pr, w[1], 3) && w[0] < w[1])));
        assert_eq!(counts(&pr, &kinds), [58, 57, 56]);
        v.pick_filter(2);
        assert!(!list(&pr, &kinds, &v).contains(&1) && list(&pr, &kinds, &v).contains(&2));
        assert_eq!(View::pages(58), 3);
        assert_eq!(View::pages(0), 1);
        v.flip(58, -1);
        assert_eq!(v.page, 2, "PREV on page 1 wraps to the last page");
        v.flip(58, 1);
        assert_eq!(v.page, 0);
    }

    #[test]
    fn cells_steps_and_index() {
        assert_eq!(cell_at(GX, GY, 0, 58), Some(0));
        assert_eq!(cell_at(GX + 5 * CW + 37, GY + 3 * CH + 37, 0, 58), Some(23));
        assert_eq!(cell_at(GX + 6 * CW, GY, 0, 58), None);
        assert_eq!(cell_at(GX + CW, GY + CH, 2, 58), Some(55));
        assert_eq!(cell_at(GX + 4 * CW, GY + 2 * CH, 2, 58), None, "page 3 holds 10 mercs");
        let l = [4, 9, 2];
        assert_eq!(step(&l, 9, 1), Some(2));
        assert_eq!(step(&l, 2, 1), Some(4));
        assert_eq!(step(&l, 4, -1), Some(2));
        assert_eq!(step(&l, 7, 1), Some(4));
        assert_eq!(step(&l, 7, -1), Some(2));
        assert_eq!(step(&[], 7, 1), None);
        assert!(on_title(10, 9) && on_title(60, 8) && !on_title(70, 8) && !on_title(30, 20));
        let mut v = View::default();
        let l: Vec<usize> = (0..58).collect();
        v.show(&l, 50);
        assert_eq!(v.page, 2);
    }
    /// The menus (Insert #13517 item 4 + #13551 item 3): both start 3 pixels left of their button and end 4 pixels
    /// above it; w = the frame + arrow room + the widest item + 6 pixels (a count in FILTER changes its width); the
    /// items sit 9 rows apart from 5 rows below the box top.
    #[test]
    fn menus_fit_their_text() {
        let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
        let (iso, exe) = (root.join("rom/JA1.gog"), root.join("build/game113b/JA.EXE"));
        if !iso.exists() || !exe.exists() {
            return;
        }
        let pd = crate::panel::panel_data(iso.to_str().unwrap(), exe.to_str().unwrap()).unwrap();
        let f = &pd.fonts;
        for (m, c) in [(0u8, [60, 57, 30]), (1, [60, 57, 30]), (1, [60, 9, 1])] {
            let (x, y, w, h, n) = menu_box(f, m, c);
            assert_eq!((x, y + h), (BUTTON_XY[1 + m as usize].0 - 3, 174), "menu {m}");
            let tw = (0..n).map(|i| crate::panel::text_width(f, 1, menu_text(m, i, c).as_bytes())).max().unwrap();
            assert_eq!(w, 3 + 6 + 3 + 2 + tw + 6 + 3, "menu {m}");
            assert!(w < 112, "menu {m} is {w} wide");
            assert_eq!(menu_item(f, m, c, x, y + 5), Some(0));
            assert_eq!(menu_item(f, m, c, x + w - 1, y + 5 + 9 * (n as i32 - 1)), Some(n - 1));
            assert_eq!(menu_item(f, m, c, x + w, y + 5), None);
            assert_eq!(menu_item(f, m, c, x, y + 5 + 9 * n as i32), None);
            assert_eq!(menu_item(f, m, c, x - 1, y + 5), None);
        }
    }

    /// Insert #13517 item 4: every grid button label (in the font that label_font picks) leaves at least 2 pixels of
    /// space each side inside its button face (37 wide with a 2 pixel edge each side).
    #[test]
    fn every_grid_label_fits_its_button() {
        let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
        let (iso, exe) = (root.join("rom/JA1.gog"), root.join("build/game113b/JA.EXE"));
        if !iso.exists() || !exe.exists() {
            return;
        }
        let pd = crate::panel::panel_data(iso.to_str().unwrap(), exe.to_str().unwrap()).unwrap();
        for l in LABELS {
            let w = crate::panel::text_width(&pd.fonts, label_font(&pd.fonts, l), l.as_bytes());
            assert!(w + 2 * (2 + 2) <= BUTTON_W, "{l}: {w} pixels in a {BUTTON_W} pixel button");
        }
    }

    /// The grid over AIMSCRN.PCX with the day-3 save: 24 faces on page 1, the sort header, a dark away face.
    /// JA1_GRID_DUMP=FILE writes the picture (64000 bytes) and the palette (768 bytes, 6 bit) to FILE.
    #[test]
    fn the_grid_draws_faces_over_the_marble() {
        let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
        let (iso, exe, save) = (root.join("rom/JA1.gog"), root.join("build/game113b/JA.EXE"), root.join("build/DAY_day3.SAV"));
        if !iso.exists() || !exe.exists() || !save.exists() {
            return;
        }
        let (iso, exe) = (iso.to_str().unwrap(), exe.to_str().unwrap());
        let w = crate::world::World::load(iso, exe, save.to_str().unwrap()).unwrap();
        let (c, pd) = (crate::common(iso).unwrap(), crate::panel::panel_data(iso, exe).unwrap());
        let main = crate::invdraw::inv_data(iso, exe).unwrap();
        let m = crate::panel::archive_member(&main.main, "AIMSCRN.PCX").unwrap();
        let scr = crate::panel::pcx(&m).unwrap();
        let kinds: [Hire; 60] = std::array::from_fn(|p| hire_kind(&w.profiles, &w.soldiers, p, |q| if q % 9 == 4 { 6 } else { 0 }));
        let mut img = vec![0u8; 64000];
        let mut table0 = c.remap[0];
        {
            let mut p = crate::panel::Painter { d: &pd, buf: &mut img, table0: &mut table0, table29: crate::panel::table29(&c) };
            crate::office::put(p.buf, &scr, 0, 0);
            draw(&mut p, &scr, &c.remap[30], &w.profiles, &w.soldiers, &kinds, &View::default(), Some(list(&w.profiles, &kinds, &View::default())[1]));
        }
        let l = list(&w.profiles, &kinds, &View::default());
        assert!(l.len() > PER, "{} mercs", l.len());
        assert!(kinds.iter().any(|&k| k == Hire::Away), "the day-3 team is away");
        // Cell 0's face: the FACES.PCX pixels of its merc (or darkened).
        let (fx, fy) = (GX as usize + 5, GY as usize + 1);
        let f = l[0];
        let same = (0..25).all(|r| (0..28).all(|k| {
            let v = pd.faces[(29 * (f / 11) + r) * 320 + 28 * (f % 11) + k];
            let got = img[(fy + r) * 320 + fx + k];
            got == v || got == c.remap[30][v as usize]
        }));
        assert!(same, "cell 0 shows the face of profile {}", f);
        assert_eq!(img[(fy - 1) * 320 + GX as usize + CW as usize + 4], 0x28, "hover frame on cell 1");
        if let Ok(path) = std::env::var("JA1_GRID_DUMP") {
            let m = crate::panel::archive_member(&main.main, "AIMSCRN.PCX").unwrap();
            let pal: Vec<u8> = m[m.len() - 768..].iter().map(|v| v >> 2).collect();
            std::fs::write(path, [img, pal].concat()).unwrap();
        }
    }
}
