//! The experience count of the morning stat changes (docs/spec/strategic.md "The experience count").

use crate::panel::{Painter, PW};

/// One thing the count does, in order: a Show (P's rows 0..145 as they are then), a wait in ms, the click sound.
#[derive(Clone, Debug, PartialEq)]
pub enum Ev {
    Show(Vec<u8>),
    Wait(u32),
    Click,
}

/// Rows 0..145 of P: the part a Show puts on the screen.
pub const SHOW_ROWS: usize = 146;

fn show(p: &Painter) -> Ev {
    Ev::Show(p.buf[..SHOW_ROWS * PW as usize].to_vec())
}

/// The step g of a change d.
pub fn step(d: i32) -> i32 {
    let a = (d as i16).unsigned_abs() as i32;
    let g = [(10000, 1000), (5000, 500), (1000, 100), (500, 50), (250, 25), (100, 10), (50, 5), (20, 2)]
        .iter()
        .find(|&&(lo, g)| a >= lo && a % g == 0)
        .map_or(1, |&(_, g)| g);
    if (d as i16) < 0 { -g } else { g }
}

/// The count of stat i from `v` by `d` for the column of slot byte `slot`, in P (`p`), with B = `bg` (BASSMAP.PCC,
/// 320 wide). `esc` says whether Escape is down (asked once per number); `fast` is the shared fast flag.
pub fn count(p: &mut Painter, bg: &[u8], slot: u8, i: usize, v: i32, d: i32, fast: &mut bool,
             esc: &mut dyn FnMut() -> bool) -> Vec<Ev> {
    let (bx, by) = (27 + 37 * slot as i32, 59 + 8 * i as i32);
    let d = d as i16 as i32;
    let t = v + d;
    let g = step(d);
    let mut evs = Vec::new();
    let mut n = v;
    while if g < 0 { n >= t } else { n <= t } {
        if esc() && !*fast {
            *fast = true;
        }
        let f = if n == v { 1 } else { 4 };
        let (w0, mut x) = if n < 100 {
            (12, bx + 2)
        } else if n < 1000 {
            (14, bx - 2)
        } else if n < 10000 {
            (16, bx)
        } else {
            (21, bx - 1)
        };
        let (mut w, mut y, mut h) = (w0, by, 8);
        for j in 0..f {
            w += 2;
            x -= 1;
            y -= 1;
            let save = rect(p, x, y, w, h);
            put_rect(p, &save, 0, 146, w, h);
            p.frame(x + 1, y, x + w0 + 1 + 2 * j, y + 7 + 2 * j, 0x79);
            evs.push(show(p));
            if n == v && d != 0 && !*fast {
                evs.push(Ev::Wait(800));
            }
            let back = rect(p, 0, 146, w, h);
            put_rect(p, &back, x, y, w, h);
            if j == 3 {
                for r in 0..8 {
                    for c in 0..18 {
                        let b = bg.get(((by + r) * 320 + bx + c) as usize).copied().unwrap_or(0);
                        p.put(bx + c, by + r, b);
                    }
                }
                crate::assign::number(p, n, 3, bx + 1, by + 1, 7, 0);
                crate::assign::number(p, n, 3, bx, by, 0xFF, 0);
                evs.push(show(p));
            }
            if !*fast {
                evs.push(Ev::Click);
                evs.push(Ev::Wait(30));
            }
            h += 2;
        }
        n += g;
    }
    evs
}

/// A w x h rectangle of P at (x, y), row by row (pixels outside P read as 0).
fn rect(p: &Painter, x: i32, y: i32, w: i32, h: i32) -> Vec<u8> {
    let mut v = Vec::with_capacity((w * h).max(0) as usize);
    for r in 0..h {
        for c in 0..w {
            v.push(p.get(x + c, y + r).unwrap_or(0));
        }
    }
    v
}

fn put_rect(p: &mut Painter, src: &[u8], x: i32, y: i32, w: i32, h: i32) {
    for r in 0..h {
        for c in 0..w {
            p.put(x + c, y + r, src[(r * w + c) as usize]);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn steps() {
        assert_eq!(step(3), 1);
        assert_eq!(step(-20), -2);
        assert_eq!(step(21), 1);
        assert_eq!(step(250), 25);
        assert_eq!(step(-10000), -1000);
        assert_eq!(step(0), 1);
    }

    /// Op 287 (the end of the count): the original shows the payroll picture P once more, rows 0..145 (the last
    /// "show" of xpcount_diff's original log = 320 x 146 bytes of P). The rows below stay as the screen has them.
    #[test]
    fn end_shows_the_panel_rows() {
        let mut v = XpView { panel: vec![7; 64000], bg: vec![], screen: vec![3; 64000], fast: false, timer: None, shows: 4, clicks: 2 };
        let mut got = vec![];
        let line = v.op("", "", 287, &[], &[], &mut || false, &mut |o| {
            if let Out::Screen(s) = o {
                got = s.to_vec();
            }
        });
        assert_eq!(line.as_deref(), Some("xp end shows 5 clicks 2 fast 0"));
        assert_eq!(got.len(), 64000);
        assert!(got[..SHOW_ROWS * 320].iter().all(|&b| b == 7));
        assert!(got[SHOW_ROWS * 320..].iter().all(|&b| b == 3));
    }

    /// Op 286 waits only what is left of the stat pause (25 ms fast): 10 ms gone -> at most 15 ms more.
    #[test]
    fn pause_waits_the_rest() {
        let t = std::time::Instant::now().checked_sub(std::time::Duration::from_millis(10)).unwrap();
        let mut v = XpView { panel: vec![], bg: vec![], screen: vec![], fast: true, timer: Some(t), shows: 0, clicks: 0 };
        let mut w = None;
        v.op("", "", 286, &[], &[], &mut || false, &mut |o| {
            if let Out::Wait(n) = o {
                w = Some(n);
            }
        });
        let w = w.expect("a wait");
        assert!(w <= 15, "{w}");
    }

    /// Op 284 copies the three columns x..x+3 (x = 37 * slot + 44) of rows 20..45 from P to the screen, no more.
    #[test]
    fn health_bar_copies_three_columns() {
        let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
        let (iso, exe) = (root.join("rom/JA1.gog"), root.join("build/game113b/JA.EXE"));
        if !iso.exists() || !exe.exists() {
            return;
        }
        let (iso, exe) = (iso.to_str().unwrap(), exe.to_str().unwrap());
        let mut v = XpView { panel: vec![7; 64000], bg: vec![], screen: vec![3; 64000], fast: false, timer: None, shows: 0, clicks: 0 };
        let mut rec = vec![0u8; 0x400];
        rec[0x200 + crate::assign::SLOT] = 1;
        v.op(iso, exe, 284, &[1], &rec, &mut || false, &mut |_| {});
        let x = 81usize;
        for r in 20..45usize {
            for i in x..x + 3 {
                assert_eq!(v.screen[r * 320 + i], v.panel[r * 320 + i], "row {r} col {i}");
            }
            assert_eq!(v.screen[r * 320 + x + 3], 3);
            assert_eq!(v.screen[r * 320 + x - 1], 3);
        }
        assert!(v.screen[20 * 320..45 * 320].iter().any(|&b| b != 3));
    }
}

/// What the stat changes put out for the front-end: the screen after a change, a wait, the click sound.
pub enum Out<'a> {
    Screen(&'a [u8]),
    Wait(u32),
    Click,
    /// Op 285: the message box (the screen with the box, the text); the front-end waits for a key, a press or 25 s.
    Message(&'a [u8], &'a str),
}

/// The text of op 285 xp_salary_msg for nickname `nick` (strategic.md "Morning stat changes" 6b).
pub fn salary_text(nick: &str, up: bool, level: i32, salary: i32) -> String {
    let (a, b) = if up {
        ("'s experience level has risen to",
         "The A.I.M. collective bargaining agreement entitles the merc to an automatic salary increase to")
    } else {
        ("'s experience level has fallen to",
         "The A.I.M. collective bargaining agreement entitles you to automatically reduce the merc's salary to")
    };
    format!("{}{} {}!  {} ${} per day, effective tomorrow.", nick, a, level, b, salary)
}

/// The state of one run of the morning stat changes (ops 282..287): P, B, the screen as the front-end shows it, the
/// fast flag and the stat timer.
pub struct XpView {
    pub panel: Vec<u8>,
    pub bg: Vec<u8>,
    pub screen: Vec<u8>,
    pub fast: bool,
    timer: Option<std::time::Instant>,
    pub shows: usize,
    pub clicks: usize,
}

impl XpView {
    /// P = the payroll picture of `soldiers` (9 records), B = BASSMAP.PCC, the screen = `screen` (320 x 200).
    pub fn new(iso: &str, exe: &str, soldiers: &[u8], faces: &[u8], stat_changed: &[u8], screen: &[u8])
               -> Result<XpView, i32> {
        let (c, d, inv) = (crate::common(iso)?, crate::panel::panel_data(iso, exe)?, crate::invdraw::inv_data(iso, exe)?);
        let mut panel = vec![0u8; 64000];
        let mut bg = vec![0u8; 64000];
        let mut table0 = c.remap[0];
        let mut p = Painter { d: &d, buf: &mut panel, table0: &mut table0, table29: crate::panel::table29(&c) };
        crate::payroll::picture(&mut p, &inv, soldiers, faces, stat_changed);
        let mut table0 = c.remap[0];
        let mut p = Painter { d: &d, buf: &mut bg, table0: &mut table0, table29: crate::panel::table29(&c) };
        crate::invdraw::load_pcx(&mut p, &inv, "BASSMAP.PCC", 0, 0);
        bg.truncate(320 * SHOW_ROWS);
        Ok(XpView { panel, bg, screen: screen.to_vec(), fast: false, timer: None, shows: 0, clicks: 0 })
    }

    /// Host op `op` (282..287) with its arguments; `soldiers` = the team records now; `esc` = Escape pressed;
    /// `out` gets the screens, waits and clicks. Returns the script line of ops 283 and 287.
    pub fn op(&mut self, iso: &str, exe: &str, op: i32, a: &[i32], soldiers: &[u8], esc: &mut dyn FnMut() -> bool,
              out: &mut dyn FnMut(Out)) -> Option<String> {
        let arg = |i: usize| a.get(i).copied().unwrap_or(0);
        let rec = |k: i32| soldiers.get(k as usize * 0x200..(k as usize + 1) * 0x200);
        match op {
            282 => {
                if esc() && !self.fast {
                    self.fast = true;
                }
                self.timer = Some(std::time::Instant::now());
                None
            }
            283 => {
                let (k, stat, v, d) = (arg(0), arg(1), arg(2), arg(3));
                let s = rec(k)?;
                let (Ok(c), Ok(pd)) = (crate::common(iso), crate::panel::panel_data(iso, exe)) else { return None };
                let v = if stat == 0 { v & 0xFFFF } else { v };
                let mut table0 = c.remap[0];
                let mut p = Painter { d: &pd, buf: &mut self.panel, table0: &mut table0, table29: crate::panel::table29(&c) };
                let evs = count(&mut p, &self.bg, s[crate::assign::SLOT], stat.clamp(0, 9) as usize, v, d, &mut self.fast, esc);
                let (n0, c0) = (self.shows, self.clicks);
                for e in evs {
                    match e {
                        Ev::Show(top) => {
                            self.screen[..top.len()].copy_from_slice(&top);
                            self.shows += 1;
                            out(Out::Screen(&self.screen));
                        }
                        Ev::Wait(ms) => out(Out::Wait(ms)),
                        Ev::Click => {
                            self.clicks += 1;
                            out(Out::Click);
                        }
                    }
                }
                Some(format!("xp count {} {} {} {} shows {} clicks {}", k, stat, v, d as i16, self.shows - n0,
                             self.clicks - c0))
            }
            284 => {
                let s = rec(arg(0))?;
                let (Ok(c), Ok(pd)) = (crate::common(iso), crate::panel::panel_data(iso, exe)) else { return None };
                let x = 37 * s[crate::assign::SLOT] as i32 + 44;
                let mut table0 = c.remap[0];
                let mut p = Painter { d: &pd, buf: &mut self.panel, table0: &mut table0, table29: crate::panel::table29(&c) };
                crate::assign::health_bar(&mut p, s, x, 20, true);
                for r in 20..45 {
                    for i in x.max(0)..(x + 3).min(320) {
                        let o = (r * 320 + i) as usize;
                        self.screen[o] = self.panel[o];
                    }
                }
                out(Out::Screen(&self.screen));
                None
            }
            285 => {
                let s = rec(arg(0))?;
                let nick = &s[0x16E..0x16E + 9];
                let nick = String::from_utf8_lossy(&nick[..nick.iter().position(|&b| b == 0).unwrap_or(9)]).into_owned();
                let text = salary_text(&nick, arg(1) != 0, arg(2) as i8 as i32, arg(3) & 0xFFFF);
                let (Ok(c), Ok(pd)) = (crate::common(iso), crate::panel::panel_data(iso, exe)) else { return None };
                let mut boxed = self.screen.clone();
                let mut table0 = c.remap[0];
                let mut p = Painter { d: &pd, buf: &mut boxed, table0: &mut table0, table29: crate::panel::table29(&c) };
                crate::msgbox::draw(&mut p, text.as_bytes(), &c.remap[crate::msgbox::MAP_TABLE],
                                    crate::msgbox::frame_colour(crate::msgbox::MAP_TABLE));
                out(Out::Message(&boxed, &text));
                let n = SHOW_ROWS * 320;
                self.screen[..n].copy_from_slice(&self.panel[..n]);
                self.shows += 1;
                out(Out::Screen(&self.screen));
                Some(format!("xp message {}", text))
            }
            286 => {
                let pause = if self.fast { 25 } else { 120 };
                if let Some(t) = self.timer {
                    let el = t.elapsed().as_millis() as u32;
                    if el < pause {
                        out(Out::Wait(pause - el));
                    }
                }
                None
            }
            287 => {
                let n = SHOW_ROWS * 320;
                self.screen[..n].copy_from_slice(&self.panel[..n]);
                self.shows += 1;
                out(Out::Screen(&self.screen));
                Some(format!("xp end shows {} clicks {} fast {}", self.shows, self.clicks, self.fast as i32))
            }
            _ => None,
        }
    }
}
