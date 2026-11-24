//! The question box (docs/spec/panel.md "Question box"): a yes / no question with a picture, a text and the
//! buttons NO and OK.
use crate::invdraw::{load_pcx, InvData};
use crate::panel::{archive_member, pcx, Painter, PW};

/// The box on the screen: x, y, w, h (also the rectangle saved before and put back after).
pub const BOX: (i32, i32, i32, i32) = (52, 37, 216, 80);

/// Where the mouse goes when it is outside the box.
pub const MOUSE: (i32, i32) = (237, 102);

/// Buttons 5 (NO) and 6 (OK): screen x, y and label.
pub const BUTTONS: [(i32, i32, &[u8]); 2] = [(142, 92, b"NO"), (208, 92, b"OK")];

const WRAP: i32 = 129;

/// YesNoPrompt? 0x28D88 (question and text-only box): with Subtitles on, a mouse outside the box moves to `MOUSE`.
/// With Subtitles off the mouse stays where it is.
pub fn prompt_warp(sub: bool, mouse: (i32, i32)) -> Option<(i32, i32)> {
    if sub {
        crate::confirm::open_warp(mouse)
    } else {
        None
    }
}

/// YesNoPrompt? 0x28F52 with a stack argument (the A.I.M. quote box 0x2900C): at the end the mouse goes back to the
/// place before the move when it is less than 10 pixels from `after`, its place after the move, on both axes.
pub fn prompt_back(after: (i32, i32), now: (i32, i32)) -> bool {
    (now.0 - after.0).abs() < 10 && (now.1 - after.1).abs() < 10
}

/// Step 2 + 3: build the box with picture `pic` (a MAIN.DAT member, None = the frame only) and copy it to the
/// screen of `p` at (52, 37). `item` = (item picture, x, y, colour) in the box (the discard question).
pub fn draw(p: &mut Painter, main: &InvData, s: &[u8], pic: Option<&str>, item: Option<(u8, i32, i32, u8)>) {
    draw_panel(p, main, s, pic, item, "PANEL_P.PCC");
}

/// `draw` with the panel picture `panel` (`PANEL_B.PCC` in the office and on the A.I.M. screen).
pub fn draw_panel(p: &mut Painter, main: &InvData, s: &[u8], pic: Option<&str>, item: Option<(u8, i32, i32, u8)>,
                  panel: &str) {
    build(p, main, s, pic, item.as_slice(), panel, true);
}

/// MergeItemsPrompt 0x4AA26 (`MERGE_ASK`): the two items in the box, (type's picture, x, y, colour).
pub const MERGE_ASK: (&[u8], &str) = (b"Attempt to PERMANENTLY merge these two items?", "MERGE.PCC");

/// One item picture of the box: (picture, x, y, colour).
pub type BoxItem = (u8, i32, i32, u8);

/// The items of the merge question: type `a` at (20, 16), type `b` at (20, 50), both colour 10.
pub fn merge_items(item_types: &[u8], a: u8, b: u8) -> [BoxItem; 2] {
    let pic = |t: u8| item_types.get(12 * t as usize + 4).copied().unwrap_or(0);
    [(pic(a), 20, 16, 10), (pic(b), 20, 50, 10)]
}

/// The question box with both item pictures of QuestionBox? 0x27939 + 0x27975.
pub fn draw_items(p: &mut Painter, main: &InvData, s: &[u8], pic: Option<&str>, items: &[BoxItem], panel: &str) {
    build(p, main, s, pic, items, panel, true);
}

/// The merge question over the screen `base`: three screens, with no button, NO and OK pressed.
pub fn merge_screens(iso: &str, exe: &str, base: &[u8], items: &[BoxItem; 2]) -> Result<Vec<Vec<u8>>, i32> {
    let (c, d, inv) = (crate::common(iso)?, crate::panel::panel_data(iso, exe)?, crate::invdraw::inv_data(iso, exe)?);
    let mut out = Vec::new();
    for pressed in [None, Some(0), Some(1)] {
        let mut img = base.to_vec();
        let mut table0 = c.remap[0];
        let mut p = Painter { d: &d, buf: &mut img, table0: &mut table0, table29: crate::panel::table29(&c) };
        draw_items(&mut p, &inv, MERGE_ASK.0, Some(MERGE_ASK.1), items, "PANEL_P.PCC");
        if let Some(k) = pressed {
            button(&mut p, k, true);
        }
        out.push(img);
    }
    Ok(out)
}

/// The window's wait for the merge question: (screen, items, island map palette) -> yes.
pub type MergeWait = Box<dyn FnMut(&[u8], [BoxItem; 2], bool) -> bool>;

thread_local! {
    static MERGE_WAIT: std::cell::RefCell<Option<MergeWait>> = const { std::cell::RefCell::new(None) };
}

/// The window sets the wait of the merge question; None takes it away.
pub fn set_merge_wait(f: Option<MergeWait>) {
    MERGE_WAIT.with(|m| *m.borrow_mut() = f);
}

/// Runs the merge question in the window; None = no window (a script or a test).
pub fn merge_wait(base: &[u8], items: [BoxItem; 2], map: bool) -> Option<bool> {
    let f = MERGE_WAIT.with(|m| m.borrow_mut().take());
    let mut f = f?;
    let yes = f(base, items, map);
    MERGE_WAIT.with(|m| *m.borrow_mut() = Some(f));
    Some(yes)
}

/// panel.md "The travel animation": the box after yes, `PANEL_P.PCC` with the picture frame block and text `s`, no
/// buttons and no picture (the go row's frame 0 goes in afterwards).
pub fn draw_go(p: &mut Painter, main: &InvData, s: &[u8]) {
    draw_go_on(p, main, s, "PANEL_P.PCC");
}

/// `draw_go` on panel picture `panel` (`PANEL_B.PCC` in the office and on A.I.M.).
pub fn draw_go_on(p: &mut Painter, main: &InvData, s: &[u8], panel: &str) {
    build(p, main, s, None, &[], panel, false);
}

fn build(p: &mut Painter, main: &InvData, s: &[u8], pic: Option<&str>, items: &[BoxItem], panel: &str,
         buttons: bool) {
    let s = &s[..s.iter().position(|&b| b == 0).unwrap_or(s.len())];
    let (bx, by, bw, bh) = BOX;
    let mut scratch = vec![0u8; 64000];
    let mut table0 = *p.table0;
    {
        let mut q = Painter { d: p.d, buf: &mut scratch, table0: &mut table0, table29: p.table29 };
        load_pcx(&mut q, main, panel, bx, by);
        for y in 0..68 {
            for x in 0..64 {
                let v = q.buf[((by + 6 + y) * PW + bx + 69 + x) as usize];
                q.buf[((by + 6 + y) * PW + bx + 6 + x) as usize] = v;
            }
        }
        if let Some(n) = pic {
            load_pcx(&mut q, main, n, bx + 12, by + 13);
        }
        for &(ip, ix, iy, ic) in items {
            crate::invdraw::picture(&mut q, main, ip, bx + ix, by + iy, ic);
        }
        let (ls, _) = crate::msgbox::lines_wrap(&q.d.fonts[2].widths, s, WRAP);
        for (i, l) in ls.iter().enumerate() {
            q.text(l, bx + 140, by + 13 + 11 * i as i32, true, 0xED, 8, 2);
        }
        if buttons {
            for k in 0..2 {
                button(&mut q, k, false);
            }
        }
    }
    for y in by..by + bh {
        let o = (y * PW + bx) as usize;
        p.buf[o..o + bw as usize].copy_from_slice(&scratch[o..o + bw as usize]);
    }
}

/// The question box over the screen `base` (320 x 200) with the panel picture `PANEL_P.PCC`: three screens, with
/// no button, NO and OK pressed (step 4).
pub fn screens(iso: &str, exe: &str, s: &[u8], pic: Option<&str>, base: &[u8]) -> Result<Vec<Vec<u8>>, i32> {
    let (c, d, inv) = (crate::common(iso)?, crate::panel::panel_data(iso, exe)?, crate::invdraw::inv_data(iso, exe)?);
    let mut out = Vec::new();
    for pressed in [None, Some(0), Some(1)] {
        let mut img = base.to_vec();
        let mut table0 = c.remap[0];
        let mut p = Painter { d: &d, buf: &mut img, table0: &mut table0, table29: crate::panel::table29(&c) };
        draw(&mut p, &inv, s, pic, None);
        if let Some(k) = pressed {
            button(&mut p, k, true);
        }
        out.push(img);
    }
    Ok(out)
}

/// panel.md "The travel animation": the box after yes over the screen `base` (320 x 200): the go row's text and its
/// frame 0.
pub fn go_screen(iso: &str, exe: &str, base: &[u8], go: &crate::sanim::Anim) -> Result<Vec<u8>, i32> {
    let (c, d, inv) = (crate::common(iso)?, crate::panel::panel_data(iso, exe)?, crate::invdraw::inv_data(iso, exe)?);
    let mut img = base.to_vec();
    let mut table0 = c.remap[0];
    let mut p = Painter { d: &d, buf: &mut img, table0: &mut table0, table29: crate::panel::table29(&c) };
    draw_go(&mut p, &inv, &go.text);
    go.put(&mut img, 0);
    Ok(img)
}

/// The merc-face question box (`draw_face`) over the screen `base` (320 x 200): three screens, with no button, NO
/// and OK pressed.
pub fn face_screens(iso: &str, exe: &str, s: &[u8], profile: u8, nick: &[u8], panel: &str, base: &[u8])
                    -> Result<Vec<Vec<u8>>, i32> {
    let (c, d, inv) = (crate::common(iso)?, crate::panel::panel_data(iso, exe)?, crate::invdraw::inv_data(iso, exe)?);
    let mut out = Vec::new();
    for pressed in [None, Some(0), Some(1)] {
        let mut img = base.to_vec();
        let mut table0 = c.remap[0];
        let mut p = Painter { d: &d, buf: &mut img, table0: &mut table0, table29: crate::panel::table29(&c) };
        draw_face(&mut p, &inv, s, profile, nick, panel);
        if let Some(k) = pressed {
            button(&mut p, k, true);
        }
        out.push(img);
    }
    Ok(out)
}

/// The tactical ABORT / RESTART questions (panel.md "Abort and restart questions"): text and picture.
pub const ABORT: (&[u8], &str) = (b"\xB1 ABORT this day?", "!MARK.PCC");
pub const RESTART: (&[u8], &str) = (b"\xB1 RESTART this day?", "Q_MARK.PCC");
/// Key A / the abort button, 0x11DC0: None = the day goes on; else the answer to the second question (Y = restart).
pub fn ask_abort(mut ask: impl FnMut((&'static [u8], &'static str)) -> bool) -> Option<bool> {
    if !ask(ABORT) {
        return None;
    }
    Some(ask(RESTART))
}

/// Alt-X on the tactical screen (ExitToDosPrompt).
pub const EXIT: (&[u8], &str) = (b"Exit to DOS?", "Q_MARK.PCC");

/// panel.md "Exit to DOS" step 4: the quit line k (0..22, random(23)), an 80-byte record of the exe's data at 0x9AF0B,
/// up to its zero byte.
pub fn quit_line(exe: &str, k: i32) -> Vec<u8> {
    let at = 0x9AF0B + 0x470A4 + 80 * k.clamp(0, 22) as usize;
    let b = std::fs::read(exe).unwrap_or_default();
    b.get(at..at + 80).unwrap_or(&[]).iter().copied().take_while(|&c| c != 0).collect()
}

/// panel.md "Exit to DOS" step 3: with no `QUICK.SAV` in `dir`, the day files are deleted.
pub fn exit_deletes(dir: &std::path::Path) -> bool {
    if dir.join("QUICK.SAV").exists() {
        return false;
    }
    crate::quicksave::delete_day_files(dir);
    true
}

/// ExitToDosPrompt 0x38084 after yes to "Exit to DOS?". `tactical` (argument 1): `quick_save` runs (the original's QuickSave
/// with its own question), no file goes. Else (argument 0): with no `QUICK.SAV` the day files go. Then the quit line: a
/// random draw (23) in both cases. Result: the day files went, the line.
pub fn exit_finish(tactical: bool, dir: Option<&std::path::Path>, exe: &str, rng: &mut crate::host::Rng, quick_save: impl FnOnce()) -> (bool, Vec<u8>) {
    let mut deleted = false;
    if tactical {
        quick_save();
    } else if let Some(d) = dir {
        deleted = exit_deletes(d);
    }
    (deleted, quit_line(exe, rng.random(23)))
}

/// panel.md "Merc-face question box", steps 1 and 2: the box with the face of `profile` (MAIN.DAT member named by
/// the number), the nickname `nick` and the text `s`, copied to the screen of `p` at (52, 37). `panel` = the panel
/// picture (`PANEL_P.PCC` on the island map, `PANEL_B.PCC` in the office).
pub fn draw_face(p: &mut Painter, main: &InvData, s: &[u8], profile: u8, nick: &[u8], panel: &str) {
    face_box(p, main, s, profile, nick, true, panel);
}

/// panel.md "Merc-face question box" with ECX 0 (a merc speaks a quote with Subtitles on): the panel picture, the
/// face and the nickname `nick` as in the question; the text only at (75, 7) (lines of the message box rule, wrap
/// 129, line height 11 − 2), fg 0xED, shadow 8, font 2; no buttons.
pub fn draw_face_text(p: &mut Painter, main: &InvData, s: &[u8], profile: u8, nick: &[u8], panel: &str) {
    face_box(p, main, s, profile, nick, false, panel);
}

fn face_box(p: &mut Painter, main: &InvData, s: &[u8], profile: u8, nick: &[u8], ask: bool, panel: &str) {
    let cut = |t: &[u8]| t[..t.iter().position(|&b| b == 0).unwrap_or(t.len())].to_vec();
    let (s, nick) = (cut(s), cut(nick));
    let (bx, by, bw, bh) = BOX;
    let mut scratch = vec![0u8; 64000];
    let mut table0 = *p.table0;
    {
        let mut q = Painter { d: p.d, buf: &mut scratch, table0: &mut table0, table29: p.table29 };
        load_pcx(&mut q, main, panel, bx, by);
        // panel.md "Face animation": the box's face point is screen (60, 44) (the face sheet of the same member).
        crate::balloon::set_last_face(crate::faceanim::FaceSpot {
            sheet: crate::balloon::face_sheet(main, &profile.to_string()), x: bx + 8, y: by + 7, b: 0 });
        // SPEC GAP: a face member that is missing or not a PCX leaves the panel picture there.
        if let Ok(m) = archive_member(&main.main, &profile.to_string()) {
            if let Ok((w, h, px)) = pcx(&m) {
                for r in 0..50.min(h) {
                    for c in 0..56.min(w) {
                        q.put(bx + 8 + c as i32, by + 7 + r as i32, px[r * w + c]);
                    }
                }
            }
        }
        let (ls, _) = crate::msgbox::lines_wrap(&q.d.fonts[2].widths, &s, WRAP);
        q.text(&nick, bx + 36, by + 60, true, 0x2A, 3, 2);
        if ask {
            for (i, l) in ls.iter().enumerate() {
                q.text(l, bx + 140, by + 13 + 10 * i as i32, true, 0x28, 8, 2);
            }
            for k in 0..2 {
                button(&mut q, k, false);
            }
        } else {
            for (i, l) in ls.iter().enumerate() {
                q.text(l, bx + 75, by + 7 + 9 * i as i32, false, 0xED, 8, 2);
            }
        }
    }
    for y in by..by + bh {
        let o = (y * PW + bx) as usize;
        p.buf[o..o + bw as usize].copy_from_slice(&scratch[o..o + bw as usize]);
    }
}

/// Button k (0 = NO, 1 = OK) up or pressed at its screen position.
pub fn button(p: &mut Painter, k: usize, pressed: bool) {
    let (x, y, label) = BUTTONS[k];
    let d = pressed as i32;
    p.element(if pressed { 0x13 } else { 0x12 }, x, y, None);
    p.text(label, x + 19 - d, y + 5 - d, true, 0x2A + d as u8, 3 - d as u8, 1);
}

/// The button (0 = NO, 1 = OK) under the mouse at (x, y): x .. x + 37, y .. y + 15. NO is tested first.
pub fn button_at(x: i32, y: i32) -> Option<usize> {
    (0..2).find(|&k| {
        let (bx, by, _) = BUTTONS[k];
        bx <= x && x <= bx + 37 && by <= y && y <= by + 15
    })
}

/// The answer of a key (its character, upper case): Some(true) = yes (Enter, O, Y), Some(false) = no (Escape,
/// N), None = nothing.
pub fn key_answer(c: u8) -> Option<bool> {
    match c.to_ascii_uppercase() {
        0x0D | b'O' | b'Y' => Some(true),
        0x1B | b'N' => Some(false),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn hits_and_keys() {
        assert_eq!(button_at(142, 92), Some(0));
        assert_eq!(button_at(179, 107), Some(0));
        assert_eq!(button_at(180, 100), None);
        assert_eq!(button_at(245, 100), Some(1));
        assert_eq!(button_at(208, 108), None);
        assert_eq!((key_answer(b'y'), key_answer(13), key_answer(27), key_answer(b'n'), key_answer(b'x')),
                   (Some(true), Some(true), Some(false), Some(false), None));
    }
}
