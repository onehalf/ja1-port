//! The battle box (docs/spec/strategic.md "The battle box"): "ENEMY ATTACKING SECTOR n!" with the enemy, guard and
//! tapper counts over the tactical view, shown by host op 334 battle_box during a sector battle.
use crate::panel::{Painter, PW};

/// The box area: x, y, w, h on the screen (darkened).
const AREA: (i32, i32, i32, i32) = (52, 37, 216, 80);
/// The part of the box that reaches the screen: x, y, w, h.
pub const SHOWN: (i32, i32, i32, i32) = (63, 42, 195, 70);
const FONT: usize = 2;
const MODE: i32 = 300;
const HEADER: &[u8] = b"Enemies       Guards      Tappers";

/// The colour of a count: 0x16 for the side that just lost one (`kind` 0 enemies, 1 guards, 2 tappers), else 0xFF.
fn colour(kind: i32, side: i32) -> u8 {
    if kind == side {
        0x16
    } else {
        0xFF
    }
}

/// Draws the box for sector `s` (0-based), the counts and `kind` on `p` (the screen as the tactical view shows it)
/// with darkening table `dark`; only the part SHOWN changes.
#[allow(clippy::too_many_arguments)]
pub fn draw(p: &mut Painter, dark: &[u8; 256], s: i32, enemies: u8, guards: u8, tappers: u8, kind: i32) {
    let mut work = p.buf.to_vec();
    let (ax, ay, aw, ah) = AREA;
    for y in ay..ay + ah {
        for x in ax..ax + aw {
            if let Some(v) = work.get_mut((y * PW + x) as usize) {
                *v = dark[*v as usize];
            }
        }
    }
    {
        let mut q = Painter { d: p.d, buf: &mut work, table0: p.table0, table29: p.table29 };
        q.frame(ax + 0x0C, ay + 6, ax + 0xCD, ay + 0x4A, 1);
        q.frame(ax + 0x0B, ay + 5, ax + 0xCC, ay + 0x49, 0x68);
        let title = format!("ENEMY ATTACKING SECTOR {}!", (s as u8 as i32) + 1);
        q.text_mode(title.as_bytes(), ax + 0x6C, ay + 0x12, true, 0xFF, 1, FONT, MODE);
        q.text_mode(HEADER, ax + 0x6C, ay + 0x28, true, 0xFF, 1, FONT, MODE);
        for (v, x, side) in [(enemies, 0x28, 0), (guards, 0x62, 1), (tappers, 0x9B, 2)] {
            let mut t = v.to_string().into_bytes();
            while t.len() < 2 {
                t.insert(0, b'^');
            }
            q.text_mode(&t, ax + x, ay + 0x32, false, colour(kind, side), 1, FONT, MODE);
        }
    }
    let (sx, sy, sw, sh) = SHOWN;
    for y in sy..sy + sh {
        let a = (y * PW + sx) as usize;
        p.buf[a..a + sw as usize].copy_from_slice(&work[a..a + sw as usize]);
    }
}

/// `draw` on a tactical screen of tileset `tileset` (darkening table 28 of that tileset).
#[allow(clippy::too_many_arguments)]
pub fn draw_tactical(iso: &str, exe: &str, tileset: u8, s: i32, counts: (u8, u8, u8), kind: i32,
                     screen: &mut [u8]) -> Result<(), i32> {
    let c = crate::common(iso)?;
    let d = crate::panel::panel_data(iso, exe)?;
    let t = crate::tileset_data(iso, tileset)?;
    let mut table0 = c.remap[0];
    let mut p = Painter { d: &d, buf: screen, table0: &mut table0, table29: crate::panel::table29(&c) };
    draw(&mut p, &t.table28, s, counts.0, counts.1, counts.2, kind);
    Ok(())
}
