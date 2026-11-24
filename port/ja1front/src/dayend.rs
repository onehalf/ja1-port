//! strategic.md "The day-end screen": the wait loop of DayEndScreen? 0x65BD4 (0x65C57..0x65F4B), one poll at a time, as
//! the original runs it: MousePoll, one key, then OK button 15 (replay screen) or 16 (payroll screen) and, on the
//! payroll screen, the merc faces. The mouse is a level: a button that is down over a face or an OK button acts in
//! that poll, also when it went down elsewhere. The window (main.rs) and the diff test (ja1f_dayend_run) are hosts.

/// What the loop asks of its host, in the original's call order.
pub trait Host {
    /// MousePoll? 0x678E8. False: the host stops the wait (the tape of a test ended).
    fn poll(&mut self) -> bool;
    /// The mouse after the last poll (x, y, buttons: 1 left, 2 right). A question box can move it during a pass.
    fn mouse(&self) -> (i32, i32, u8);
    /// KeyPressed 0x5616C + GetKey 0x560F4.
    fn key(&mut self) -> Option<u8>;
    /// The second byte of an extended key (GetKey).
    fn key2(&mut self) -> u8;
    /// ExitToDosPrompt 0x38084 with argument 0. True: the program ends.
    fn exit_prompt(&mut self) -> bool;
    /// SpecialNpcSpeech? 0x29D54.
    fn say_again(&mut self);
    /// ToggleMessage 0x3F994 ("Text ", Subtitles).
    fn toggle_text(&mut self);
    /// ButtonPressSound (0) and ButtonReleaseSound (1).
    fn button_sound(&mut self, which: u8);
    /// The face of OK button `b` in PressPanelButton? 0x35D44.
    fn ok_face(&mut self, b: u8, pressed: bool);
    /// PressPanelButton?: the left button comes up.
    fn wait_left_up(&mut self);
    /// Soldier k `inCurSector` (+0x182).
    fn present(&self, k: usize) -> bool;
    /// ShowTooltip 0x23264 number k + 1, or HideTooltip 0x23420.
    fn tip(&mut self, k: Option<usize>);
    /// The question "Fire NICK?" of soldier k. True: yes.
    fn ask_fire(&mut self, k: usize) -> bool;
    /// Subtitles 0x1163BD: on, the question box stays on the screen during FireMerc's quote (0x65E1E).
    fn subtitles(&self) -> bool;
    /// RestoreSaveArea0 0x28698 (0x25): the question box goes.
    fn close_question(&mut self);
    /// Yes: FireMerc 0x5097C, LoadPanels, TeamCompact? 0x50750.
    fn fire(&mut self, k: usize);
    /// DrawBassMap 0x4420C + DrawTeamSlotMarks? 0x4508C.
    fn redraw(&mut self);
}

/// The hit area of a button test is the place + the pressed element's width and height (ButtonAt? 0x67CD8, DL = 1).
pub const OK_W: i32 = 25;
pub const OK_H: i32 = 15;

/// OK button `b` place (table 0x93458).
pub fn ok_at(b: u8) -> (i32, i32) {
    if b == 15 { (266, 174) } else { (279, 166) }
}

/// The face rectangles of soldier k (MouseInRect? 0x67C0C, inclusive): a button down, no button.
pub fn face_rect(k: usize, down: bool) -> (i32, i32, i32, i32) {
    let k = k as i32;
    if down { (11 + 37 * k, 20, 47 + 37 * k, 46) } else { (12 + 37 * k, 20, 42 + 37 * k, 46) }
}

fn inside(r: (i32, i32, i32, i32), x: i32, y: i32) -> bool {
    x >= r.0 && x <= r.2 && y >= r.1 && y <= r.3
}

/// The text of the fire question (sprintf "%s%s?" with "Fire " and the nickname).
pub fn fire_text(nick: &[u8]) -> Vec<u8> {
    let mut t = b"Fire ".to_vec();
    t.extend_from_slice(nick);
    t.push(b'?');
    t
}

/// FireMerc's arguments for soldier k: slot byte +0x17A, the balloon at x = 11 + 37 k (the pass's own x), y = 20.
pub fn fire_args(soldiers: &[u8], k: usize) -> (i32, i32, i32) {
    (soldiers[k * 0x200 + 0x17A] as i32, 11 + 37 * k as i32, 20)
}

/// DrawTeamSlotMarks? 0x4508C: the slots that get element 0x67: k = 0.. while soldier k is present, k < 8, profile < 60.
pub fn slot_marks(soldiers: &[u8]) -> Vec<i32> {
    (0..8usize).take_while(|&k| soldiers[k * 0x200 + 0x182] != 0)
        .filter(|&k| soldiers[k * 0x200 + 0x179] < 60).map(|k| k as i32).collect()
}

/// The result of a wait.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum End {
    /// The OK button came up.
    Ok,
    /// Yes to "Exit to DOS?".
    Exit,
    /// The host stopped the wait.
    Stopped,
}

/// One wait: 15 = the replay screen, 16 = the payroll screen (its faces work too).
pub fn wait(h: &mut impl Host, b: u8) -> End {
    loop {
        if !h.poll() {
            return End::Stopped;
        }
        // One key per poll. ToUpper? on the sign-extended byte; only 0 + 0x2D, L and T do something.
        if let Some(c) = h.key() {
            match c.to_ascii_uppercase() {
                0 => {
                    if h.key2() == 0x2D && h.exit_prompt() {
                        return End::Exit;
                    }
                }
                b'L' => h.say_again(),
                b'T' => h.toggle_text(),
                _ => {}
            }
        }
        // ButtonAt? 0x67CD8 reads the mouse again: a box of the key step can change it.
        let (x, y, buttons) = h.mouse();
        let (x0, y0) = ok_at(b);
        if buttons & 1 != 0 && x >= x0 && x <= x0 + OK_W && y >= y0 && y <= y0 + OK_H {
            h.button_sound(0);
            h.ok_face(b, true);
            h.wait_left_up();
            h.ok_face(b, false);
            h.button_sound(1);
            return End::Ok;
        }
        if b != 16 {
            continue;
        }
        if buttons & 3 != 0 {
            // The pass goes on after a question, and the mouse can be elsewhere by then.
            for k in 0..8 {
                let (mx, my, _) = h.mouse();
                if inside(face_rect(k, true), mx, my) && h.present(k) {
                    h.tip(None);
                    if h.ask_fire(k) {
                        if !h.subtitles() {
                            h.close_question();
                        }
                        h.fire(k);
                    } else {
                        h.close_question();
                    }
                    h.redraw();
                }
            }
        } else {
            let hit = (0..8).find(|&k| inside(face_rect(k, false), x, y) && h.present(k));
            h.tip(hit);
        }
    }
}
