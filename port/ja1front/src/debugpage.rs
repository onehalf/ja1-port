//! cursor.md "Debug keys (level 4)": the four debug pages of one soldier (Ctrl+P, Shift+left click) and the
//! opponent lists (Ctrl+O).

use crate::panel::Painter;

/// One row value: a field of the record by type and offset, or a value from outside the record.
#[derive(Clone, Copy)]
enum V {
    U8(usize),
    I8(usize),
    I16(usize),
    U16(usize),
    Zero,
    CallTile,
    CallLevel,
    Armour,
    AtWaypoint,
    Enemies,
    AnimWord,
    ScriptWord,
}

use V::*;

const PAGES: [[(&str, V); 17]; 4] = [
    [
        ("active", U8(0x182)),
        ("in sector", I8(0x138)),
        ("dead And Removed", I8(0x188)),
        ("visible", I8(0x9C)),
        ("stopped", U8(0x1B8)),
        ("moved", I8(0x1B0)),
        ("forced to Stop", I8(0x197)),
        ("new Situation", U8(0x1C1)),
        ("action", U8(0x1D1)),
        ("action Data", I16(0x132)),
        ("action in Progr.", I8(0x1D6)),
        ("action Timeout", U16(0xF0)),
        ("alert Status", I8(0xBB)),
        ("noise Gridno", I16(0x122)),
        ("noise Volume", U8(0x1C0)),
        ("Pub Noise Gridno", CallTile),
        ("Pub Noise Volume", CallLevel),
    ],
    [
        ("service", U8(0x1B9)),
        ("serv grid", I16(0xD6)),
        ("serv cnt", I8(0x1CA)),
        ("busy Being Hit", I8(0x195)),
        ("bypass To Green", U8(0x131)),
        ("made Roll By", U8(0x1FC)),
        ("has Keys", I8(0xB9)),
        ("gassed", U8(0x1DB)),
        ("underFire", I8(0x1C4)),
        ("in the Way", U8(0x1D7)),
        ("where I Was", I16(0x12E)),
        ("tiles Moved", I8(0x1F2)),
        ("last Known Loc", I16(0x108)),
        ("just Arrived", I8(0x1AB)),
        ("---", Zero),
        ("---", Zero),
        ("---", Zero),
    ],
    [
        ("guynum", U8(0x17A)),
        ("health", I8(0x183)),
        ("breath", I8(0x18A)),
        ("protect %", Armour),
        ("actionPts", I8(0x1A5)),
        ("orders", U8(0x1D4)),
        ("attitude", U8(0x1D3)),
        ("reacheddest", AtWaypoint),
        ("nextdest", I16(0x12A)),
        ("desdest", I16(0x120)),
        ("nextact", U8(0x1A6)),
        ("actgrid", I16(0x134)),
        ("destination", I16(0x124)),
        ("destx", I16(0x126)),
        ("desty", I16(0x128)),
        ("x", I16(0x116)),
        ("y", I16(0x118)),
    ],
    [
        ("# opponents", I8(0x1D5)),
        ("cur oppnum", U8(0x1D8)),
        ("target   ", I16(0xDC)),
        ("override", U8(0x1C2)),
        ("moved", I8(0x1B0)),
        ("# enemies", Enemies),
        ("artillery", I8(0x1C9)),
        ("anitype", AnimWord),
        ("frame", ScriptWord),
        ("firing", U8(0x1BF)),
        ("OLD oppnum", U8(0x1F7)),
        ("action pts", I8(0x1A5)),
        ("DestFirst", I8(0x1F6)),
        ("desdir", I8(0x1A2)),
        ("aiming", U8(0x1BE)),
        ("ammo", I8(0x01)),
        ("oldgridno", I16(0x1F8)),
    ],
];

/// The values from outside the record: the latest call of each side (tile, level: `g_callM`, `g_callE`), the
/// "enemies" byte of the current sector and the soldier's script word.
#[derive(Clone, Copy, Default)]
pub struct Outside {
    pub call_m: (i16, u8),
    pub call_e: (i16, u8),
    pub enemies: u8,
    pub script_word: u16,
}

pub struct Page {
    /// Pages 1 and 2 redraw the map view whatever the view mode; pages 3 and 4 the current screen.
    pub map_view: bool,
    pub colon_x: i32,
    pub number_x: i32,
    pub rows: Vec<(&'static str, i32)>,
}

fn u16_at(r: &[u8], o: usize) -> u16 {
    // An offset past the record (a +0x1F3 above 10, not seen in play) reads 0 here.
    u16::from_le_bytes([r.get(o).copied().unwrap_or(0), r.get(o + 1).copied().unwrap_or(0)])
}

/// AtWaypoint (S): 0 while a walk step is under way (service < 5, animation word >= 4, position off the step target).
fn at_waypoint(r: &[u8]) -> i32 {
    let anim = u16_at(r, 0x1E8 + 2 * r[0x1F3] as usize);
    let off = (0..3).any(|k| u16_at(r, 0x114 + 2 * k) != u16_at(r, 0x124 + 2 * k));
    !(r[0x1B9] < 5 && anim >= 4 && off) as i32
}

/// The four pages of soldier record `r` (512 bytes).
pub fn pages(r: &[u8], o: &Outside) -> Vec<Page> {
    let call = if r[0x17A] < 40 { o.call_m } else { o.call_e };
    PAGES
        .iter()
        .enumerate()
        .map(|(p, rows)| Page {
            map_view: p < 2,
            colon_x: if p < 2 { 148 } else { 118 },
            number_x: if p < 2 { 165 } else { 125 },
            rows: rows
                .iter()
                .map(|&(label, v)| {
                    let n = match v {
                        U8(a) => r[a] as i32,
                        I8(a) => r[a] as i8 as i32,
                        I16(a) => u16_at(r, a) as i16 as i32,
                        U16(a) => u16_at(r, a) as i32,
                        Zero => 0,
                        CallTile => call.0 as i32,
                        CallLevel => call.1 as i32,
                        Armour => crate::invdraw::armour(r),
                        AtWaypoint => at_waypoint(r),
                        Enemies => o.enemies as i32,
                        AnimWord => u16_at(r, 0x1E8 + 2 * r[0x1F3] as usize) as i32,
                        ScriptWord => o.script_word as i32,
                    };
                    (label, n)
                })
                .collect(),
        })
        .collect()
}

/// The page's texts in draw order: (x, y, text): the 17 labels, the 17 ":" texts, the 17 numbers (at least 4
/// characters, '^' on the left).
pub fn texts(page: &Page) -> Vec<(i32, i32, Vec<u8>)> {
    let y = |k: usize| 15 + 10 * k as i32;
    let mut out: Vec<(i32, i32, Vec<u8>)> = page.rows.iter().enumerate().map(|(k, (l, _))| (50, y(k), l.as_bytes().to_vec())).collect();
    out.extend((0..page.rows.len()).map(|k| (page.colon_x, y(k), b":".to_vec())));
    out.extend(page.rows.iter().enumerate().map(|(k, &(_, n))| {
        let mut s = n.to_string().into_bytes();
        while s.len() < 4 {
            s.insert(0, b'^');
        }
        (page.number_x, y(k), s)
    }));
    out
}

/// Draws the page's texts over the screen (font 1, colour 0xFF, shadow 8).
pub fn draw(p: &mut Painter, page: &Page) {
    for (x, y, s) in texts(page) {
        p.text(&s, x, y, false, 0xFF, 8, 1);
    }
}

/// The pages of soldier `slot` in world `w`: the side calls, the current sector's "enemies" byte and the script
/// word from the world.
pub fn pages_of(w: &crate::world::World, slot: usize) -> Vec<Page> {
    let c = &w.calls;
    let i16_at = |o: usize| i16::from_le_bytes([c[o], c[o + 1]]);
    let o = Outside {
        call_m: (i16_at(0), c[2]),
        call_e: (i16_at(4), c[6]),
        enemies: w.sector_state[w.cur_sector as usize * 9 + 1],
        script_word: crate::enter::script_words(w).get(slot).copied().unwrap_or(0),
    };
    pages(&w.soldiers[slot * 512..(slot + 1) * 512], &o)
}

/// `page` over `screen` (320 x 200).
pub fn draw_on(iso: &str, exe: &str, page: &Page, screen: &mut [u8]) -> Result<(), i32> {
    let c = crate::common(iso)?;
    let d = crate::panel::panel_data(iso, exe)?;
    let mut table0 = c.remap[0];
    let mut p = Painter { d: &d, buf: screen, table0: &mut table0, table29: crate::panel::table29(&c) };
    draw(&mut p, page);
    Ok(())
}

/// The opponent lists of soldier `slot` in world `w`.
pub fn opp_lists_of(w: &crate::world::World, slot: usize) -> Vec<String> {
    opp_lists(&w.soldiers[slot * 512..(slot + 1) * 512], &w.knowledge, &w.remember_m, &w.remember_e)
}

/// The opponent lists (Ctrl+O) of record `r`: `knowledge` = g_known (48 x i8), `remember_m` (16 x 8),
/// `remember_e` (8 x 40).
pub fn opp_lists(r: &[u8], knowledge: &[u8], remember_m: &[u8], remember_e: &[u8]) -> Vec<String> {
    let s = r[0x17A] as usize;
    let list = |head: &str, b: &mut dyn Iterator<Item = u8>| {
        let mut t = head.to_string();
        for v in b {
            t += &format!(" {}", v as i8);
        }
        t
    };
    let n = if s < 40 { 8 } else { 40 };
    let public = if s < 40 { 40..48 } else { 0..40 };
    let seen: Vec<u8> = if s < 16 {
        remember_m[8 * s..8 * s + 8].to_vec()
    } else if s >= 40 {
        remember_e[40 * (s - 40)..40 * (s - 40) + 40].to_vec()
    } else {
        Vec::new()
    };
    vec![
        format!("OPPCNT = {}", r[0x1D5] as i8),
        list("PERSONAL:", &mut r[0x13A..0x13A + n].iter().copied()),
        list("PUBLIC:", &mut public.map(|i| knowledge[i])),
        list("SEEN:", &mut seen.into_iter()),
    ]
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn number_padding_and_rows() {
        let mut r = vec![0u8; 512];
        r[0x17A] = 41;
        r[0x183] = 0xFB; // health -5
        let o = Outside { call_e: (300, 2), ..Default::default() };
        let p = pages(&r, &o);
        assert_eq!(p.len(), 4);
        assert_eq!(p[0].rows[15], ("Pub Noise Gridno", 300));
        let t = texts(&p[2]);
        assert_eq!(t.len(), 51);
        assert_eq!(t[34 + 1], (125, 25, b"^^-5".to_vec()));
        assert_eq!(p[2].rows[7].1, 1);
    }

    #[test]
    fn opp_lists_counts() {
        let mut r = vec![0u8; 512];
        r[0x17A] = 20;
        let l = opp_lists(&r, &[0; 48], &[0; 128], &[0; 320]);
        assert_eq!(l[3], "SEEN:");
        assert_eq!(l[1].split(' ').count(), 9);
    }
}
