//! The A.I.M. screen (docs/spec/strategic.md "The A.I.M. screen"): the page of one merc, the team faces, the
//! funds, browsing, the interview and the hire, the removal of a merc hired in this visit, and leaving with the
//! helicopter charge. The logic runs in the port library (port_api.md "A.I.M. entry points"). The original screen
//! loop is AimScreen 0x2011C: its opening block is diff-tested (aim_diff --port open), its decision routines
//! (refusal, hire, cancel, browse) carry their own sections, and gate_aim.py proves the screen as a player sees it.
use std::collections::VecDeque;
use std::ffi::c_void;
use std::sync::Arc;

use crate::assign::Tooltip;
use crate::data::Common;
use crate::host::{Ja1Host, Ja1World, Rng};
use crate::invdraw::{self, InvData};
use crate::panel::{self, Painter, PanelData, PW};
use crate::question;
use crate::world::World;

extern "C" {
    fn ja1_aim_open(w: *mut Ja1World, h: *const Ja1Host) -> i32;
    fn ja1_aim_browse(w: *mut Ja1World, h: *const Ja1Host, q: i32, d: i32) -> i32;
    fn ja1_aim_refusal(w: *mut Ja1World, h: *const Ja1Host, p: i32) -> i32;
    fn ja1_aim_hire(w: *mut Ja1World, h: *const Ja1Host, p: i32) -> i32;
    fn ja1_aim_cancel_pending(w: *mut Ja1World, h: *const Ja1Host);
    fn ja1_aim_pay(w: *mut Ja1World, h: *const Ja1Host) -> i32;
    fn ja1_fire_merc_core(w: *mut Ja1World, h: *const Ja1Host, slot: i32, x: i32, y: i32);
    fn ja1_team_compact(w: *mut Ja1World, h: *const Ja1Host, redraw: i32);
    fn ja1_calc_ledger_all(w: *mut Ja1World, h: *const Ja1Host);
    fn ja1_performance_rating(w: *mut Ja1World, h: *const Ja1Host) -> i32;
}

pub const SOLDIER: usize = 0x200;
const PROFILE: usize = 0x5A;
/// The darkening table of the screen (g_remapTables[30]: the face darkening and the message boxes).
const DARK: usize = 30;

pub const FUNDS_WARNING: &str = "WARNING! You do not have enough funds to authorize helicopter transportation.";
pub const TEAM_FULL: &str = "Your team is complete. If you wish to hire another member, you must get rid of somebody.";
pub const NO_FUNDS: &str = "Save your mouse finger - you have insufficient funds.";
pub const TERMINATE: &str = "Terminate communication with A.I.M.?";
pub const CANT_AFFORD: &str = "You can't afford the helicopter charge...";
/// panel.md "The travel animation", Callers: the archive and the wait / yes rows of the helicopter charge and the
/// terminate question (no row none).
const SANIM_ARC: &str = "SANIM.DAT";
const HELI_ROWS: (i32, i32) = (0, 1);
const TERMINATE_ROWS: (i32, i32) = (2, 3);

/// The mercs hired in this visit (AimScreen 0x20F38: record byte +0x19D = 1).
fn pending_count(soldiers: &[u8]) -> usize {
    (0..8).filter(|&k| soldiers[k * SOLDIER + 0x19D] == 1).count()
}

/// AimScreen 0x20F52..0x20F6A, the exit branch: "cannot afford" first?, then the first question. Nobody hired: no
/// charge. Balance below the charge: the message, and yes removes the mercs of this visit.
fn leave_first(pending: usize, balance: i32, heli: i32) -> (bool, Ask) {
    if pending == 0 {
        (false, Ask::Terminate { cancel: false })
    } else if balance < heli {
        (true, Ask::Terminate { cancel: true })
    } else {
        (false, Ask::Heli)
    }
}

/// AimScreen 0x21066: no to the charge asks "Terminate" with the mercs of this visit at stake.
const ASK_AFTER_HELI_NO: Ask = Ask::Terminate { cancel: true };

/// The exit branch as a list of steps for `aimleave_diff`, from the same rules as `Aim::leave` and `Aim::answer`, with
/// the answers in `answers`: 1 = "cannot afford", 2 = ask terminate, 3 = ask charge, 4 = pay, 5 = cancel the mercs of
/// this visit, 6 = leave, 7 = stay.
pub fn leave_route(soldiers: &[u8], bal: i32, difficulty: u8, answers: &[bool]) -> Vec<i32> {
    let (cant_afford, mut ask) = leave_first(pending_count(soldiers), bal, heli_charge(difficulty));
    let mut out = vec![];
    if cant_afford {
        out.push(1);
    }
    let mut answers = answers.iter();
    loop {
        out.push(if ask == Ask::Heli { 3 } else { 2 });
        let yes = *answers.next().unwrap_or(&false);
        match (ask, yes) {
            (Ask::Heli, true) => {
                out.extend([4, 6]);
                return out;
            }
            (Ask::Heli, false) => ask = ASK_AFTER_HELI_NO,
            (Ask::Terminate { cancel }, true) => {
                if cancel {
                    out.push(5);
                }
                out.push(6);
                return out;
            }
            _ => {
                out.push(7);
                return out;
            }
        }
    }
}

/// The helicopter charge H by difficulty 0 / 1 / 2.
pub fn heli_charge(difficulty: u8) -> i32 {
    [600, 800, 1000][difficulty.min(2) as usize]
}

/// Buttons EXIT, INFO, GEAR (37 x 15 hit areas: x .. x + 37, y .. y + 15) and the two arrows (25 x 18).
const BUTTONS: [(i32, i32, i32, i32); 5] = [(11, 178, 37, 15), (59, 178, 37, 15), (108, 178, 37, 15),
    (158, 177, 24, 17), (198, 177, 24, 17)];

/// What shows over the screen until a press or a key.
enum Over {
    /// A message box (msgbox rule); `left` = the text starts at x 66 instead of centred lines.
    Msg { text: Vec<u8>, left: bool },
    /// Key L: the merc-face box with the text only, until a key or a click.
    Quote { profile: u8, nick: Vec<u8>, text: Vec<u8> },
    /// P says quote: the face talks (panel.md "Face animation", the wait of "Speech balloon" step 7); `start` = the
    /// wait's start (ms, set at its first poll), `text_ms` = 80 ms per text byte, `voiced` = a voice plays. `quote`
    /// = Subtitles on: (profile, nickname, text) of the text-only merc-face box, its face at face point (59, 43);
    /// None = Subtitles off: P's face on the page at face point (8, 20), mode 0, no box.
    Talk { wait: crate::faceanim::SpeechWait, start: Option<i64>, quote: Option<(u8, Vec<u8>, Vec<u8>)> },
    /// A yes / no question.
    Ask(Ask),
    /// The yes row of question `ask` plays in the box (panel.md "The travel animation"); at its end the answer acts.
    Go(Ask),
    /// The A.I.M. statistics page (its picture; the A.I.M. palette while it shows).
    Info { img: Vec<u8>, aim_pal: [u8; 768] },
    /// The salary panel of team record k (strategic.md "The salary panel") while the right button is held.
    Salary { k: usize, values: [i32; 10] },
    /// panel.md "Item info box" from the gear view (strategic.md "The gear view item click"): the screen with the box
    /// or the picture (None: Item Info off, no box), what was drawn, the wait step of step 7 (0 = both buttons up,
    /// 1 = a button down, 2 = both up), the voice has a name sample, Item Info on, the kept mouse position and the
    /// mouse position after step 6.
    Item { img: Option<Vec<u8>>, shown: crate::iteminfo::Shown, step: u8, voice: bool, info: bool, kept: (i32, i32),
           at: (i32, i32) },
    /// AddFunds? 0x514A4 mode 1: the shown balance counts down. Nothing else runs: the original waits inside.
    Count(FundsCount),
    /// The screen ends (after the boxes before it).
    Leave,
}

/// One count-down of AddFunds? 0x514A4 mode 1.
#[derive(Clone, Copy, Debug)]
pub struct FundsCount {
    shown: i32,
    left: i32,
    step: i32,
    /// When the next step is due. BusyWait? 0x67250 is a calibrated loop, not the BIOS clock: real time here.
    next: Option<std::time::Instant>,
    /// The face click with the old-debt answer (mode 3) goes on after the count for this profile.
    then_profile: Option<usize>,
    /// For the window's log line: the first step and the number of steps.
    first: Option<std::time::Instant>,
    steps: u32,
}

impl FundsCount {
    /// 0x514B0..0x514D0: the step is 10 when amount / 10 as a signed 16-bit number (cwde) is 30 or less, else 100.
    pub fn new(shown: i32, amount: i32, then_profile: Option<usize>) -> FundsCount {
        let step = if (amount / 10) as i16 as i32 <= 30 { 10 } else { 100 };
        FundsCount { shown, left: amount, step, next: None, then_profile, first: None, steps: 0 }
    }

    /// One redraw: true = a 30 ms wait follows, false = the last one, with no wait.
    pub fn step(&mut self) -> bool {
        if self.left > self.step {
            self.shown -= self.step;
            self.left -= self.step;
            true
        } else {
            self.shown -= self.left;
            self.left = 0;
            false
        }
    }
}

/// 30 ms: the wait after each step but the last (0x51482, BusyWait? EAX 0x1E).
const COUNT_WAIT: std::time::Duration = std::time::Duration::from_millis(30);

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
enum Ask {
    /// "Terminate communication with A.I.M.?"; `cancel`: yes removes the mercs hired in this visit.
    Terminate { cancel: bool },
    /// "There is a $H helicopter charge. Authorize payment?"
    Heli,
    /// "Fire <nickname>?" for team record k.
    Fire(usize),
    /// Alt-X: "Exit to DOS?" (panel.md "Exit to DOS").
    Exit,
}

struct AimHost {
    rng: Rng,
    stash: Vec<[u8; 3]>,
    /// The host ops 328 fire_quote and 337 aim_remark of the call (op, arguments), for the screen to act on.
    said: Vec<(i32, Vec<i32>)>,
}

unsafe extern "C" fn cb_random(ctx: *mut c_void, n: i32) -> i32 {
    (*(ctx as *mut AimHost)).rng.random(n)
}

unsafe extern "C" fn cb_call(ctx: *mut c_void, op: i32, args: *const i32, nargs: i32, _out: *mut u8, _size: i32) -> u32 {
    let a = if nargs > 0 && !args.is_null() { std::slice::from_raw_parts(args, nargs as usize) } else { &[] };
    let arg = |i: usize| a.get(i).copied().unwrap_or(0);
    match op {
        264 => crate::play::stash_put(&mut (*(ctx as *mut AimHost)).stash, arg(0) as u8, arg(1) as u8, arg(2) as u8),
        // fire_quote (strategic.md "Firing a merc") and aim_remark (the named merc's name sample, "Refusal reason").
        328 | 337 => {
            (*(ctx as *mut AimHost)).said.push((op, a.to_vec()));
            0
        }
        // SPEC GAP front-end: aim_removed (338): no 500 ms wait between the removals.
        _ => 0,
    }
}

/// The pictures and texts of the screen.
struct AimData {
    scr: (usize, usize, Vec<u8>),
    deceased: Option<(usize, usize, Vec<u8>)>,
    /// EDT/BIOQUOTE.EDT (320-byte records) and EDT/AIMQUOTE.EDT (200-byte records, 14 per merc).
    bio: Vec<u8>,
    quotes: Vec<u8>,
}

/// LoadBioQuoteEdt 0x234E0: the 320 bytes of profile `pi` in EDT/BIOQUOTE.EDT.
pub(crate) fn bio_record(bio: &[u8], pi: usize) -> &[u8] {
    bio.get(pi * 320..pi * 320 + 320).unwrap_or(&[])
}

pub(crate) fn edt(iso: &str, exe: &str, name: &str) -> Vec<u8> {
    // OpenCDFile 0x38304: the game folder first, then the CD.
    let dir = std::path::Path::new(exe).parent().map(|p| p.to_path_buf()).unwrap_or_default();
    if let Ok(b) = std::fs::read(dir.join("EDT").join(name)) {
        return b;
    }
    let Ok(mut i) = crate::iso::Iso::open(iso) else { return Vec::new() };
    i.read(&format!("EDT/{}", name)).unwrap_or_default()
}

/// A record's text: up to the first 0, every byte but the space taken − 1.
pub(crate) fn decode(rec: &[u8]) -> Vec<u8> {
    let t = &rec[..rec.iter().position(|&b| b == 0).unwrap_or(rec.len())];
    t.iter().map(|&b| if b == b' ' { b } else { b.wrapping_sub(1) }).collect()
}

/// AimScreen 0x2011C, the sites 0x205B1..0x2078D: quote q of profile p as the box shows it, and the member of a refusal.
/// PlayQuote 0x24F08 reads the record; refusal reasons 6, 7 and 13 write the member's +1 encoded nickname to g_textBuf and
/// run StrReplace 0x252B0 with its bug (issue #2 row 44) on the raw record. The other quotes show the shared buffer.
pub fn quote_text(quotes: &[u8], last_quote: &mut [u8; 6], profiles: &[u8], text_buf: &mut [u8], pi: usize, q: i32) -> (Vec<u8>, Option<u8>) {
    // world_load.md "The last quote": PlayQuote (speaker p, category 0, row 14 p, quote q).
    let row = (14 * pi as i16).to_le_bytes();
    let qq = (q.max(0) as i16).to_le_bytes();
    *last_quote = [pi as u8, 0, row[0], row[1], qq[0], qq[1]];
    let raw = crate::balloon::shared_raw(quotes, (14 * pi + q.max(0) as usize) * 200);
    let pr = &profiles[pi * PROFILE..(pi + 1) * PROFILE];
    let m = match q + 1 {
        6 => Some(pr[0x4C]),
        7 => Some(pr[0x3B]),
        13 => Some(pr[0x4D]),
        _ => None,
    };
    let Some(m) = m.filter(|&m| m < 64) else {
        return (decode(&raw), None);
    };
    let nick = cstr(&profiles[m as usize * PROFILE..m as usize * PROFILE + 10]);
    crate::play::encode_name(text_buf, &nick);
    let rep: Vec<u8> = nick.iter().map(|&b| b.wrapping_add(1)).collect();
    (decode(&crate::balloon::str_replace(&raw, &rep, b"NFNCFS")), Some(m))
}

fn cstr(b: &[u8]) -> Vec<u8> {
    b[..b.iter().position(|&c| c == 0).unwrap_or(b.len())].to_vec()
}

/// strategic.md "The A.I.M. statistics page": the labels and their rows.
const STATS_LEFT: [(&str, i32); 12] = [("Completed Days", 33), ("Mercenary Sorties", 43), ("Success Points", 53), ("# Hirings", 73),
    ("# Fatalities", 83), ("# Terminations", 93), ("# Resignations", 103), ("# Non-Payments", 113), ("Success Rate", 133),
    ("Fatality Rate", 143), ("Turnover Rate", 153), ("Non-Payment Rate", 163)];
const STATS_RIGHT: [(&str, i32); 12] = [("Current Team Size", 33), ("Avg. Health", 43), ("Avg. Agility", 53), ("Avg. Marksmanship", 63),
    ("Avg. Wisdom", 73), ("Avg. Salary", 93), ("Minimum Team Cost", 103), ("Maximum Team Cost", 113), ("Current Balance", 133),
    ("Estimated Costs", 143), ("Estimated Income", 153), ("Estimated Balance", 163)];

/// strategic.md "The A.I.M. statistics page" steps 1..5: the page (320 x 200) and its palette from team records 0..7
/// (`soldiers`), the recalculated `accounts` (0xA8 bytes) and the performance rating score.
pub fn stats_page(main: &[u8], pd: &panel::PanelData, c: &crate::Common, soldiers: &[u8], accounts: &[u8], rating: i32)
                  -> Option<(Vec<u8>, [u8; 768])> {
    let m = panel::archive_member(main, "STATBGND.PCX").ok()?;
    let (pw, ph, px) = panel::pcx(&m).ok()?;
    let mut img = vec![0u8; 64000];
    for r in 0..ph.min(200) {
        let n = pw.min(320);
        img[r * 320..r * 320 + n].copy_from_slice(px.get(r * pw..r * pw + n)?);
    }
    let tail = m.get(m.len().checked_sub(768)?..)?;
    let mut pal = [0u8; 768];
    for (i, v) in tail.iter().enumerate() {
        pal[i] = v >> 2;
    }
    let a = |o: usize| accounts.get(o).copied().unwrap_or(0) as i32;
    let a32 = |o: usize| i32::from_le_bytes([a(o) as u8, a(o + 1) as u8, a(o + 2) as u8, a(o + 3) as u8]);
    let day = a32(0x04);
    let rates = if a(0x9D) != 0 && day > 0 {
        [a(0x9A).wrapping_mul(100).wrapping_div(day.wrapping_mul(2)), aim_rate(accounts, 0), aim_rate(accounts, 1), aim_rate(accounts, 2)]
    } else {
        [0; 4]
    };
    let (mut n, mut sums, mut cost_s, mut cost_m) = (0i32, [0i32; 4], 0i32, 0i32);
    for k in 0..8 {
        let r = soldiers.get(k * 512..k * 512 + 512)?;
        if r[0x182] == 0 {
            break;
        }
        n += 1;
        for (j, o) in [0x184, 0x18B, 0x191, 0x18D].into_iter().enumerate() {
            sums[j] += r[o] as i8 as i32;
        }
        let sal = u16::from_le_bytes([r[0x110], r[0x111]]) as i32;
        cost_s += sal;
        cost_m += (sal + 1) / 2;
    }
    let avg = |v: i32| if n > 0 { v / n } else { 0 };
    let left = [day, a(0x9F), a(0x9A), a(0x9D), a32(0x00), a(0x9C), a(0xA0), a(0xA1), rates[0], rates[1], rates[2], rates[3]];
    let right = [n, avg(sums[0]), avg(sums[1]), avg(sums[2]), avg(sums[3]), avg(cost_s), cost_m, cost_s, a32(0x24), a32(0x64),
                 a32(0x20), a32(0x28)];
    let mut table0 = c.remap[0];
    let mut p = Painter { d: pd, buf: &mut img, table0: &mut table0, table29: panel::table29(c) };
    let num = |p: &mut Painter, v: i32, d: usize, x: i32, y: i32| {
        let mut t = v.to_string().into_bytes();
        while t.len() < d {
            t.insert(0, b'^');
        }
        p.text(&t, x, y, false, 0x1D, 9, 1);
    };
    for (i, (label, y)) in STATS_LEFT.iter().enumerate() {
        p.text(label.as_bytes(), 9, *y, false, 0x29, 7, 1);
        num(&mut p, left[i], 4, 125, *y);
    }
    for (i, (label, y)) in STATS_RIGHT.iter().enumerate() {
        p.text(label.as_bytes(), 171, *y, false, 0x29, 7, 1);
        num(&mut p, right[i], 6, 274, *y);
    }
    p.text(b"Overall Rating:", 107, 185, false, 0x29, 7, 1);
    num(&mut p, rating & 0xFF, 3, 195, 185);
    p.text(b"%", 213, 185, false, 0x1D, 9, 1);
    Some((img, pal))
}

/// strategic.md "AIM rate" without "one more" (kind 0 fatality, 1 turnover, 2 non-payment) from the accounts.
fn aim_rate(acc: &[u8], kind: i32) -> i32 {
    let b = |o: usize| acc.get(o).copied().unwrap_or(0) as i32;
    let d = b(0x04);
    let o = if kind == 0 { d } else { b(0x9D) };
    let mut t = (d / 2 + o) & 255;
    if t == 0 {
        return 0;
    }
    t = t.max(10);
    let n = match kind {
        0 => b(0x00),
        1 => (b(0xA0) + b(0x9C)) & 255,
        _ => b(0xA1),
    };
    let r = (n * 100 / t) & 255;
    r.min(100)
}

/// strategic.md "Button labels": the view V (0 = bio, 1 = gear) is a program-wide value; it is bio at the program
/// start and each visit opens with the view of the last visit.
static AIM_VIEW: std::sync::atomic::AtomicU8 = std::sync::atomic::AtomicU8::new(0);

/// The A.I.M. screen's state while it is open.
pub struct Aim {
    d: AimData,
    pd: Arc<PanelData>,
    c: Arc<Common>,
    main: Arc<InvData>,
    iso: String,
    pub rng: Rng,
    /// The screen without the boxes (with the tooltip).
    base: Vec<u8>,
    pub pal: [u8; 768],
    /// Mode M of the page, the interview counts, view V (0 = bio, 1 = gear), X of mode 3.
    mode: u8,
    asked: [u8; 60],
    view: u8,
    transfer: i32,
    tip: Tooltip,
    over: VecDeque<Over>,
    buttons: i32,
    /// The screen button (0..4) whose pressed face shows until the left button is up, and the keys typed meanwhile.
    held: Option<usize>,
    /// The press of `held`: the mouse goes back there at the release.
    hold: Option<crate::pointer::ButtonHold>,
    /// ButtonAt 0x35F98 on a question box: the pressed OK or NO until the left button is up; then the answer.
    ask_down: Option<usize>,
    /// KeyOrClick 0x67E44 of the row after the answer (0x277F6); made at the row's first poll from the start flags as
    /// the last KeyOrClick left them (`invextra::latch`: ConfirmDialog sets none).
    go_wait: Option<crate::invextra::KeyOrClick>,
    /// KeyOrClick 0x67E44 of a message box (ShowMessage? 0x27B98 via StatusMessage? / AimAlreadyHiredMsg?): made at the
    /// box's first poll with the flags from the button level then (ShowMessage sets them at its start).
    msg_wait: Option<crate::invextra::KeyOrClick>,
    held_keys: Vec<u8>,
    /// The face timer t of the opening (the first eye time te of panel.md "Face animation", A.I.M. screen).
    pub face_timer: i32,
    /// The window's face blink (panel.md "Face animation", A.I.M. screen; only when `timed`): the eye state and the
    /// face sheet of the profile it shows.
    blink: Option<crate::faceanim::AimBlink>,
    blink_sheet: Option<(usize, Vec<u8>)>,
    /// The eye frame now on the screen (0 = the face's own eyes).
    blink_shown: i32,
    heli: i32,
    /// The profile and nickname of the fire question.
    fire_who: (u8, Vec<u8>),
    /// The host ops of the last library calls that the screen shows (op 328 fire_quote).
    said: Vec<(i32, Vec<i32>)>,
    /// The window moves the mouse there (the item info box, panel.md step 6 and 7).
    pub warp: Option<(i32, i32)>,
    /// The quote box: the place of the mouse before and after the move of YesNoPrompt? 0x28D88.
    talk_mouse: Option<((i32, i32), (i32, i32))>,
    /// The window sets it: the right button held on the face browses again each 200 ms of the game clock
    /// (crate::dostime). Off in a script (a line has no time): only a press browses.
    pub repeat: bool,
    /// The clock at the last browse of the held right button (the original's TimerStart).
    rep_at: Option<i64>,
    exe: String,
    /// The window sets it: the question box's animation steps by its frame time. Off in a script (a line has no
    /// time): the box keeps the row's frame 0, and a go row ends only at a press or a key.
    pub timed: bool,
    /// The clock time at which the box in front closes by itself (`HIRED_MS`).
    msg_close: Option<i64>,
    /// The row playing in the question box: its number, the row, the next frame and the time of the last frame
    /// step (None = the first step comes at once).
    sanim: Option<(i32, crate::sanim::Anim, usize, Option<std::time::Instant>)>,
}

/// What one poll did: script lines and the end of the screen.
#[derive(Default)]
pub struct Poll {
    pub lines: Vec<String>,
    pub leave: bool,
    /// A fired merc's quote balloon (strategic.md "Firing a merc", host op 328): the op's arguments (slot, x, y,
    /// profile) and the screen under the balloon.
    pub fire: Option<(Vec<i32>, Vec<u8>)>,
    /// panel.md "Exit to DOS" (argument 0): yes to Alt-X's question; the quit line. The caller deletes the day
    /// files (step 3) and ends the program.
    pub quit: Option<Vec<u8>>,
}

impl Aim {
    /// The statistics page is open (strategic.md "The A.I.M. statistics page"; the window fades between them).
    pub fn info_open(&self) -> bool {
        matches!(self.over.front(), Some(Over::Info { .. }))
    }

    /// Opening steps 1..4: the randoms, the picture, the page of the next profile from P − 1 up, the funds warning.
    /// `seed` = the game's random seed (written back by the caller from `rng`).
    pub fn new(iso: &str, exe: &str, w: &mut World, seed: u32) -> Result<(Aim, Vec<String>), i32> {
        // music.md "Who plays what": A.I.M. plays theme 3 of MENUS.
        crate::music::set_exe(exe);
        crate::music::screen("MENUS", 3);
        let mut a = Self::loaded(iso, exe, w, seed)?;
        a.face_timer = a.call(w, |cw, h| unsafe { ja1_aim_open(cw, h) });
        let mut lines = vec!["aim open".to_string()];
        lines.extend(a.show(w));
        if balance(w) < heli_charge(w.difficulty) {
            a.over.push_back(Over::Msg { text: FUNDS_WARNING.as_bytes().to_vec(), left: false });
            lines.push("aim msg funds warning".into());
        }
        Ok((a, lines))
    }

    /// ShowAimScreen 0x1FEB0 after the files: the page of the next profile from P - 1 up (AimBrowse), on the whole
    /// picture.
    fn show(&mut self, w: &mut World) -> Vec<String> {
        let p = w.accounts.get(0xA4).copied().unwrap_or(0) as i32;
        self.browse(w, p - 1, 1)
    }

    /// The files of the screen, and the fields at their start values.
    fn loaded(iso: &str, exe: &str, w: &World, seed: u32) -> Result<Aim, i32> {
        let (c, pd) = (crate::common(iso)?, panel::panel_data(iso, exe)?);
        let main = invdraw::inv_data(iso, exe)?;
        let m = panel::archive_member(&main.main, "AIMSCRN.PCX").map_err(|_| crate::E_FILE)?;
        let scr = panel::pcx(&m).map_err(|_| crate::E_FILE)?;
        let tail = m.get(m.len().saturating_sub(768)..).filter(|t| t.len() == 768).ok_or(crate::E_FILE)?;
        let mut pal = [0u8; 768];
        for (i, v) in tail.iter().enumerate() {
            pal[i] = v >> 2;
        }
        let deceased = panel::archive_member(&main.main, "DECEASED.PCC").ok().and_then(|m| panel::pcx(&m).ok());
        let d = AimData { scr, deceased, bio: edt(iso, exe, "BIOQUOTE.EDT"), quotes: edt(iso, exe, "AIMQUOTE.EDT") };
        Ok(Aim { d, pd, c, main, iso: iso.to_string(), rng: Rng::new(seed), base: vec![0; 64000], pal,
                          mode: 0, asked: [0; 60], view: AIM_VIEW.load(std::sync::atomic::Ordering::Relaxed), transfer: 0, tip: Tooltip::default(),
                          over: VecDeque::new(), buttons: 0, held: None, hold: None, ask_down: None, go_wait: None, msg_wait: None, held_keys: vec![], face_timer: 0, heli: heli_charge(w.difficulty),
                          msg_close: None, fire_who: (0, vec![]), said: Vec::new(), warp: None, talk_mouse: None, repeat: false, rep_at: None, exe: exe.to_string(), timed: false,
                          sanim: None, blink: None, blink_sheet: None, blink_shown: 0 })
    }

    /// A library call with this screen's host (random = the game's numbers, the item store of `w`).
    fn call<R>(&mut self, w: &mut World, f: impl FnOnce(*mut Ja1World, *const Ja1Host) -> R) -> R {
        let mut host = AimHost { rng: Rng { seed: self.rng.seed, draws: self.rng.draws }, stash: std::mem::take(&mut w.stash_records),
                                 said: Vec::new() };
        let mut cw = Ja1World::view(w);
        let h = Ja1Host { ctx: &mut host as *mut AimHost as *mut c_void, random: cb_random, call: cb_call, host_ops: 0 };
        let r = f(&mut cw, &h);
        cw.store(w);
        w.stash_records = host.stash;
        self.rng = host.rng;
        for (op, a) in host.said {
            if op == 337 {
                // "Refusal reason": the speaker's sample of the named merc's name, the middle of the quote's voice.
                let (iso, exe) = (self.iso.clone(), self.exe.clone());
                let (p, m) = (a.first().copied().unwrap_or(0) as u8, a.get(1).copied().unwrap_or(0) as u8);
                crate::sound::screen(|s| {
                    if !s.has_files() {
                        *s = crate::sound::Sound::with_files(&iso, &exe);
                    }
                    s.name_voice(p, m);
                });
            } else {
                self.said.push((op, a));
            }
        }
        r
    }

    fn p(&self, w: &World) -> usize {
        w.accounts.get(0xA4).copied().unwrap_or(0) as usize
    }

    /// Browsing from q by d; the page of the new P (mode by the profile's state).
    fn browse(&mut self, w: &mut World, q: i32, d: i32) -> Vec<String> {
        let p = self.call(w, |cw, h| unsafe { ja1_aim_browse(cw, h, q, d) });
        if p < 0 {
            return vec!["aim browse none".into()];
        }
        let pr = &w.profiles[p as usize * PROFILE..];
        self.mode = page_mode(pr);
        self.redraw(w);
        vec![format!("aim page {} mode {}", p, self.mode)]
    }

    /// The whole screen again: AIMSCRN.PCX, the funds, the team faces, the page of P. The tooltip is gone.
    fn redraw(&mut self, w: &World) {
        let mut img = vec![0u8; 64000];
        let mut table0 = self.c.remap[0];
        {
            let mut p = Painter { d: &self.pd, buf: &mut img, table0: &mut table0, table29: panel::table29(&self.c) };
            crate::office::put(p.buf, &self.d.scr, 0, 0);
            let shown = match self.over.front() {
                Some(Over::Count(c)) => c.shown,
                _ => balance(w),
            };
            self.funds(&mut p, shown);
            self.labels(&mut p);
            self.team(&mut p, w);
            self.page(&mut p, w);
        }
        self.base = img;
        self.tip = Tooltip::default();
        self.blink_shown = 0;
    }

    /// Funds: the balance as 5 digits at x 2 of a 32 x 7 box, fg 0x28, shadow 0x30, font 2, its rows 2..8 at (280,
    /// 185): so the digits start at x 282 (the maintainer play test #29 D).
    fn funds(&self, p: &mut Painter, value: i32) {
        funds_digits(p, value);
    }

    /// AimDrawFunds? 0x513E4: the 32 x 7 funds box at (280, 185) of the shown screen, clean, with `value`.
    fn funds_box(&mut self, value: i32) {
        let mut img = std::mem::take(&mut self.base);
        let mut table0 = self.c.remap[0];
        let mut p = Painter { d: &self.pd, buf: &mut img, table0: &mut table0, table29: panel::table29(&self.c) };
        funds_box_draw(&mut p, &self.d.scr, value);
        self.base = img;
    }

    /// The running count-down at the front of the queue: every step that is due. Without the window's clock (`timed`
    /// off) all of them at once. Returns true when the count ended in this call.
    fn count_pass(&mut self, w: &mut World, out: &mut Poll) -> bool {
        let timed = self.timed;
        let Some(Over::Count(c)) = self.over.front_mut() else { return false };
        let mut c = *c;
        let mut more = true;
        while more && (!timed || c.next.is_none_or(|t| std::time::Instant::now() >= t)) {
            more = c.step();
            c.steps += 1;
            let due = c.next.unwrap_or_else(std::time::Instant::now);
            c.first.get_or_insert(due);
            c.next = Some(due + COUNT_WAIT);
            self.funds_box(c.shown);
        }
        if more {
            self.over[0] = Over::Count(c);
            return false;
        }
        self.over.pop_front();
        out.lines.push(if timed {
            format!("aim count {} steps {} ms", c.steps, c.first.map_or(0, |t| t.elapsed().as_millis()))
        } else {
            format!("aim count {} steps", c.steps)
        });
        if let Some(pi) = c.then_profile {
            self.old_debt_paid(w, pi, out);
        }
        true
    }

    /// The face click, M = 3, after the count-down: the debt is gone, then the refusal reason of P.
    fn old_debt_paid(&mut self, w: &mut World, pi: usize, out: &mut Poll) {
        w.profiles[pi * PROFILE + 0x3E] = 0;
        w.profiles[pi * PROFILE + 0x3F] = 0;
        let r = self.call(w, |cw, h| unsafe { ja1_aim_refusal(cw, h, pi as i32) });
        if r == 0 {
            self.hire(w, 10, out);
        } else {
            self.quote(w, 13, out);
            w.profiles[pi * PROFILE + 0x3D] = 1;
            // AimShowMercBio? 0x21648 (0x2092A): the page mode from the profile (aimface_diff).
            self.mode = page_mode(&w.profiles[pi * PROFILE..]);
            self.redraw(w);
        }
    }

    /// The labels of the buttons EXIT, INFO and GEAR / BIO (the other view): centred at (x + 19, 183), fg 0x2A,
    /// shadow 3, font 1.
    fn labels(&self, p: &mut Painter) {
        let gear: &[u8] = if self.view == 0 { b"GEAR" } else { b"BIO" };
        for (x, label) in [(11, &b"EXIT"[..]), (59, b"INFO"), (108, gear)] {
            p.text(label, x + 19, 183, true, 0x2A, 3, 1);
        }
    }

    /// Team faces of records 0..7 with `inCurSector` ≠ 0.
    fn team(&self, p: &mut Painter, w: &World) {
        for k in 0..8 {
            let s = &w.soldiers[k * SOLDIER..(k + 1) * SOLDIER];
            let (x, y) = face_xy(k as usize);
            if s[0x182] != 0 {
                let mut face = vec![0u8; 1024];
                if panel::load_face(&self.iso, &self.pd, s[0x179] as i32, 0, &mut face).is_ok() {
                    for r in 0..25 {
                        for c in 0..28 {
                            p.put(x + c, y + r, face[(2 + r as usize) * 32 + 2 + c as usize]);
                        }
                    }
                }
            }
            // 0x21198: the name strip under the face, for an empty slot too.
            p.element(0x18, x, y + 29, None);
            if s[0x182] != 0 {
                p.text(&cstr(&s[0x16E..0x177]), x + 14, y + 30, true, 0x2A, 0xFE, 0);
                if s[0x19D] != 0 {
                    p.element(0x11, x, y, None);
                }
            }
        }
    }

    /// The page of P: face, stats, nickname, text panel.
    fn page(&self, p: &mut Painter, w: &World) {
        page_draw(p, &self.c, &self.d, &self.main, self.p(w), &w.profiles, &w.soldiers, self.view, self.mode);
    }

    /// One poll: the mouse at (x, y), the buttons held (bit 0 left, bit 1 right) and the keys typed before it (a 0
    /// byte before the scan code of an extended key). A button counts when it goes down.
    pub fn poll(&mut self, w: &mut World, x: i32, y: i32, buttons: i32, keys: &[u8]) -> Poll {
        let out = self.poll_in(w, x, y, buttons, keys);
        // ShowMessage? 0x27B98 sets KeyOrClick's start flags from the button level when the box opens (in this poll).
        if matches!(self.over.front(), Some(Over::Msg { .. })) && self.msg_wait.is_none() {
            self.msg_wait = Some(crate::invextra::KeyOrClick::new(self.buttons as u8 & 3));
            crate::invextra::set_latch(self.buttons as u8 & 3);
        }
        out
    }

    fn poll_in(&mut self, w: &mut World, x: i32, y: i32, buttons: i32, keys: &[u8]) -> Poll {
        let mut out = Poll::default();
        let press = buttons & !self.buttons;
        self.buttons = buttons;
        // strategic.md "Button faces": the pressed face shows until the left button is up; then the release sound
        // and the button's action. Keys typed meanwhile wait.
        let mut keys = keys;
        let held_keys;
        let held_keys2;
        if let Some(b) = self.held {
            self.held_keys.extend_from_slice(keys);
            if buttons & 1 != 0 {
                return out;
            }
            self.held = None;
            self.warp = self.hold.take().and_then(|h| h.step(0));
            crate::sound::screen_button(&self.iso, &self.exe, 1);
            out.lines.push(format!("aim button {} up", b));
            self.button_action(w, b, &mut out);
            held_keys = std::mem::take(&mut self.held_keys);
            keys = &held_keys;
        }
        // Keys typed while a question's button was down stayed in the queue.
        let held_keys3;
        if self.held.is_none() && self.ask_down.is_none() && !self.held_keys.is_empty()
            && !matches!(self.over.front(), Some(Over::Count(_))) {
            let mut all = std::mem::take(&mut self.held_keys);
            all.extend_from_slice(keys);
            held_keys3 = all;
            keys = &held_keys3;
        }
        // AddFunds? 0x514A4: the original stays inside this wait; typed keys stay in the queue for later.
        if matches!(self.over.front(), Some(Over::Count(_))) {
            self.held_keys.extend_from_slice(keys);
            if !self.count_pass(w, &mut out) {
                return out;
            }
            held_keys2 = std::mem::take(&mut self.held_keys);
            keys = &held_keys2;
        }
        if !self.over.is_empty() {
            self.box_input(w, x, y, press, keys, &mut out);
            self.sanim_step(w, x, y, &mut out);
            self.leave_check(&mut out);
            return out;
        }
        let mut i = 0;
        while i < keys.len() && self.over.is_empty() {
            let ext = keys[i] == 0;
            if ext {
                i += 1;
            }
            let k = keys.get(i).copied().unwrap_or(0);
            i += 1;
            let p = self.p(w) as i32;
            match (ext, k) {
                (false, 27) => self.leave(w, &mut out),
                (true, 0x4B | 0x48 | 0x49) => out.lines.extend(self.browse(w, p, -1)),
                (true, 0x4D | 0x50 | 0x51) => out.lines.extend(self.browse(w, p, 1)),
                (false, b'v' | b'V') => {
                    self.over.push_back(Over::Msg { text: crate::data::exe_text(&self.exe, crate::data::VERSION_AIM).as_bytes().to_vec(), left: false });
                    out.lines.push("aim msg version".into());
                }
                // panel.md "Say it again (key L)": the last quote's text in the text-only merc-face box
                // (PANEL_B.PCC: the office opened before A.I.M.) until a key or a click.
                (false, b'l' | b'L') => match crate::balloon::again_text(&self.iso, &self.exe, w) {
                    Some((profile, nick, text)) => {
                        let q = &w.last_quote;
                        out.lines.push(format!("aim again {} cat {} row {} quote {}", q[0], q[1],
                                               i16::from_le_bytes([q[2], q[3]]), i16::from_le_bytes([q[4], q[5]])));
                        self.over.push_back(Over::Quote { profile, nick, text });
                        self.warp = question::prompt_warp(crate::ctlpanel::get(w, 5) != 0, (x, y));
                    }
                    None => out.lines.push(format!("aim again none {}", w.last_quote[0])),
                },
                // strategic.md "Key T": the Subtitles switch flips, then the message "Text  on" / "Text  off".
                (false, b't' | b'T') => {
                    let v = crate::ctlpanel::get(w, 5) ^ 1;
                    crate::ctlpanel::set(w, 5, v);
                    let text = if v != 0 { "Text  on" } else { "Text  off" };
                    self.over.push_back(Over::Msg { text: text.as_bytes().to_vec(), left: false });
                    out.lines.push(format!("aim msg {}", text));
                }
                // strategic.md "Key Alt-X": the quit question (Exit to DOS, argument 0).
                (true, 0x2D) => {
                    self.over.push_back(Over::Ask(Ask::Exit));
                    // QuestionBox? 0x279B1 (ExitToDosPrompt): the return flag is 0.
                    crate::pointer::mouse_save((x, y));
                    self.warp = crate::confirm::open_warp((x, y));
                    if let Some(at) = self.warp {
                        out.lines.push(format!("aim question mouse {} {}", at.0, at.1));
                    }
                    out.lines.push("aim ask exit".into());
                }
                _ => {}
            }
        }
        if self.over.is_empty() {
            if self.timed {
                self.blink_step(w, x, y, &mut out);
            }
            self.hover(w, x, y);
            // The right button (the left one up) on the face: browse +1, then again each 200 ms (the original
            // waits until the clock reads 200 ms more or the button is up, and the next pass browses again while it
            // is still down).
            let again = browse_again(self.repeat, buttons, self.rep_at, crate::dostime::now_ms());
            if buttons & 2 == 0 {
                self.rep_at = None;
            }
            if (press & 2 != 0 || again) && buttons & 1 == 0 && on_face(x, y) {
                let p = self.p(w) as i32;
                self.rep_at = Some(crate::dostime::now_ms());
                out.lines.extend(self.browse(w, p, 1));
            } else if let (2, Some(k)) = (buttons & 3, team_face_at(w, x, y)) {
                // strategic.md "The salary panel": shown while the right button is held.
                let values = salary_values(&w.soldiers[k * SOLDIER..]);
                self.over.push_back(Over::Salary { k, values });
                out.lines.push(format!("aim salary {} {:?}", k, values));
            } else if self.view == 1 && buttons & 3 != 0 && self.item_click(w, x, y, &mut out) {
            } else if press & 1 != 0 {
                self.click(w, x, y, &mut out);
            }
        }
        self.sanim_step(w, x, y, &mut out);
        self.leave_check(&mut out);
        out
    }

    /// AimScreen 0x2111C: the music volume goes to 0 over 1200 ms, then the 24 step fade of the picture, then
    /// `stop_theme` (0x2112F; music.md "Who plays what": the office plays theme 1 again).
    pub fn leave_music() {
        crate::music::with(|m| m.set_volume(0, 1200));
    }

    fn leave_check(&mut self, out: &mut Poll) {
        if matches!(self.over.front(), Some(Over::Leave)) {
            self.over.clear();
            out.leave = true;
            // The host fades: `leave_music` before the picture fade, then the stop.
            out.lines.push("aim leave".into());
        }
    }

    /// A box shows: a press or a key closes a message or a quote; a question waits for its answer.
    fn box_input(&mut self, w: &mut World, x: i32, y: i32, press: i32, keys: &[u8], out: &mut Poll) {
        match self.over.front() {
            Some(Over::Ask(a)) => {
                let a = *a;
                let ans = if let Some(k) = self.ask_down {
                    // PressPanelButton 0x36040: no key, no row and no answer until the left button is up; the mouse
                    // goes back, the release sound comes, and the answer acts.
                    self.held_keys.extend_from_slice(keys);
                    if self.buttons & 1 != 0 {
                        return;
                    }
                    self.ask_down = None;
                    self.warp = self.hold.take().and_then(|h| h.step(0));
                    crate::sound::screen_button(&self.iso, &self.exe, 1);
                    Some(k == 1)
                } else if let Some(k) = (press & 1 != 0).then(|| question::button_at(x, y)).flatten() {
                    self.ask_down = Some(k);
                    self.hold = Some(crate::pointer::ButtonHold::press((x, y)));
                    crate::sound::screen_button(&self.iso, &self.exe, 0);
                    return;
                } else {
                    keys.iter().find_map(|&k| question::key_answer(k))
                };
                if let Some(yes) = ans {
                    self.over.pop_front();
                    out.lines.push(format!("aim answer {}", if yes { "yes" } else { "no" }));
                    self.answer(w, a, yes, out);
                }
            }
            // strategic.md "The A.I.M. statistics page" step 7: a key byte 13 or the left button down ends it.
            Some(Over::Info { aim_pal, .. }) => {
                if keys.contains(&13) || self.buttons & 1 != 0 {
                    self.pal = *aim_pal;
                    self.over.pop_front();
                    self.redraw(w);
                    out.lines.push("aim info end".into());
                }
            }
            Some(Over::Salary { .. }) => {
                if self.buttons & 2 == 0 {
                    self.over.pop_front();
                    out.lines.push("aim salary end".into());
                }
            }
            Some(Over::Go(g)) => {
                // 0x277F6: KeyOrClick ends the row: a key, or a button when it comes up; no frame step until then.
                let g = *g;
                // Keys in its release wait are lost: every yes row ends in Over::Leave, and InitOfficeScreen? flushes them.
                let wait = self.go_wait.get_or_insert_with(|| crate::invextra::KeyOrClick::new(crate::invextra::latch()));
                if wait.step(self.buttons as u8, !keys.is_empty()) {
                    self.go_wait = None;
                    self.over.pop_front();
                    self.go_end(w, g, out);
                }
            }
            Some(Over::Item { .. }) => self.item_wait(x, y, out),
            Some(Over::Talk { .. }) => self.talk_wait(w, x, y, press, keys, out),
            Some(Over::Leave) | Some(Over::Count(_)) | None => {}
            Some(Over::Msg { .. }) => {
                // ShowMessage? 0x27B98: KeyOrClick ends it (a key, or a button that was up at the start when it comes
                // up; no timer read during that release wait); a box with a time limit also closes when the time ends.
                let level = self.buttons as u8 & 3;
                let wait = self.msg_wait.get_or_insert_with(|| crate::invextra::KeyOrClick::new(crate::invextra::latch()));
                // Its release wait (the MousePoll loop) reads no key: keys stay in the queue for the screen after it.
                if wait.in_release() {
                    self.held_keys.extend_from_slice(keys);
                }
                let done = wait.step(level, !keys.is_empty());
                let late = !done && !wait.in_release() && self.timed
                    && self.msg_close.is_some_and(|t| crate::dostime::now_ms() as i64 >= t);
                if done || late {
                    self.msg_wait = None;
                    self.msg_close = None;
                    self.over.pop_front();
                    out.lines.push("aim box closed".into());
                }
            }
            Some(_) => {
                // A quote box: a press or a key closes it; a box with a time limit also closes when the time ends.
                let late = self.timed && self.msg_close.is_some_and(|t| crate::dostime::now_ms() as i64 >= t);
                if press != 0 || !keys.is_empty() || late {
                    self.msg_close = None;
                    if matches!(self.over.pop_front(), Some(Over::Quote { .. })) {
                        crate::sound::screen(|s| s.speech_stop());
                    }
                    out.lines.push("aim box closed".into());
                }
            }
        }
    }

    /// panel.md "Speech balloon" step 7 for the talking A.I.M. face: Enter, Escape, Space or a click end the wait (a
    /// click outside the face rectangle only after 1000 ms), and so do the voice's end or, without a voice, the text
    /// time. Without the window's clock (`timed` off) only a key or a press ends it. At the end the plain face.
    fn talk_wait(&mut self, w: &World, x: i32, y: i32, press: i32, keys: &[u8], out: &mut Poll) {
        let timed = self.timed;
        let Some(Over::Talk { wait, start, quote }) = self.over.front_mut() else { return };
        let t = crate::dostime::now_ms() as i64;
        // YesNoPrompt? 0x2900C (Subtitles on): the mouse moves into the box as the box opens, and comes back at the end.
        if start.is_none() && quote.is_some() {
            crate::pointer::mouse_save((x, y)); // 0x28D83: the stack argument of the quote box
            let moved = question::prompt_warp(true, (x, y));
            if let Some(at) = moved {
                self.warp = Some(at);
                out.lines.push(format!("aim prompt mouse {} {}", at.0, at.1));
            }
            self.talk_mouse = Some(((x, y), moved.unwrap_or((x, y))));
        }
        let now = t - *start.get_or_insert(t);
        let lip = crate::sound::screen(|s| s.speech_lip());
        // Without the window's clock (a script) a click anywhere ends the wait.
        let input = crate::faceanim::WaitInput {
            now: if timed { now } else { now.max(1001) },
            x,
            y,
            left: press & 1 != 0,
            right: press & 2 != 0,
            key: keys.iter().any(|k| crate::faceanim::wait_key_ends(*k)),
            playing: lip.is_some(),
            played_us: lip.as_ref().map_or(0, |l| l.1),
            silent: None,
        };
        if wait.pass(&input).end.is_some() {
            let (n, boxed) = (wait.face.redraws, quote.is_some());
            self.over.pop_front();
            crate::sound::screen(|s| s.speech_stop());
            if boxed {
                if let Some((before, after)) = self.talk_mouse.take() {
                    if question::prompt_back(after, (x, y)) {
                        self.warp = Some(before);
                        out.lines.push(format!("aim prompt back {} {}", before.0, before.1));
                    }
                }
                out.lines.push(format!("aim quote talk end {}", n));
                out.lines.push("aim box closed".into());
                return;
            }
            // "When the wait ends the plain face (frames 0) is drawn once more."
            self.eyes(w, 0);
            out.lines.push(format!("aim talk end {}", n));
        }
    }

    /// panel.md "Face animation", A.I.M. screen: only for an available merc (mode 0), the eyes blink while the mouse
    /// is off the face; on the face the face's own eyes show.
    fn blink_step(&mut self, w: &World, x: i32, y: i32, out: &mut Poll) {
        if self.mode != 0 {
            return;
        }
        let now = crate::dostime::now_ms();
        let te = self.face_timer as i64;
        let b = self.blink.get_or_insert_with(|| crate::faceanim::AimBlink::new(te, now, crate::faceanim::AnimRng::new(now as u32)));
        // On the face rect the eyes stay as they are (the original's face branch draws no eye frame).
        if on_face(x, y) || !b.pass(now) {
            return;
        }
        let f = b.e.min(3);
        self.eyes(w, f);
        out.lines.push(format!("aim blink {}", f));
    }

    /// Eye frame `f` of the shown merc's face to the screen at (14, 26) (FaceEyeFrame, panel.md "Face animation").
    fn eyes(&mut self, w: &World, f: i32) {
        let pi = self.p(w);
        if self.blink_sheet.as_ref().map(|s| s.0) != Some(pi) {
            self.blink_sheet = Some((pi, crate::balloon::face_sheet(&self.main, &pi.to_string())));
        }
        let sheet = &self.blink_sheet.as_ref().unwrap().1;
        crate::faceanim::draw_eyes(sheet, f, &mut self.base, PW as usize, 14, 26);
        self.blink_shown = f;
    }

    /// strategic.md "The gear view item click": a button held on the n-th kit item of the gear view (rect
    /// (4 + 44 (n mod 5), 95 + 38 (n / 5)) .. (+42, +36)); the answer = an item was hit.
    fn item_click(&mut self, w: &mut World, x: i32, y: i32, out: &mut Poll) -> bool {
        let pi = self.p(w);
        let kit: Vec<u8> = w.profiles[pi * PROFILE + 0x50..pi * PROFILE + 0x5A].iter().copied().filter(|&t| t != 0).collect();
        let Some(n) = (0..kit.len()).find(|&n| {
            let (ix, iy) = (4 + 44 * (n as i32 % 5), 95 + 38 * (n as i32 / 5));
            (ix..=ix + 42).contains(&x) && (iy..=iy + 36).contains(&y)
        }) else {
            return false;
        };
        self.eyes(w, 0);
        // Only a merc who is not on the team, or only hired in this visit (b19D ≠ 0), shows the box.
        let on = (0..8).any(|k| w.soldiers[k * SOLDIER + 0x182] != 0 && w.soldiers[k * SOLDIER + 0x179] == pi as u8);
        let pending = hired_this_visit(&w.soldiers, pi as u8) != 0;
        if on && !pending {
            return true;
        }
        let t = kit[n];
        let it = &self.main.item_types;
        let count = if it.get(t as usize * 12 + 6).copied() == Some(1) { it.get(t as usize * 12 + 5).copied().unwrap_or(0) as i8 } else { 1 };
        let info = crate::ctlpanel::get(w, 6) != 0;
        let i = crate::iteminfo::Info { profile: pi as u8, t, count, value: 20, numbers: false, opt_info: info,
                                        opt_sub: crate::ctlpanel::get(w, 5) != 0 };
        let mut img = self.picture();
        let mut table0 = self.c.remap[0];
        let shown = {
            let mut p = Painter { d: &self.pd, buf: &mut img, table0: &mut table0, table29: panel::table29(&self.c) };
            crate::iteminfo::draw(&mut p, &self.main, i)
        };
        let it = self.main.item_types.clone();
        let (voice, names) = crate::sound::screen(|snd| {
            let v = crate::iteminfo::voice(&i, &it, false, &|arc, m| snd.inv_has(arc, m));
            let names = snd.item_voice(v.as_deref().unwrap_or(&[]));
            (v.is_some(), names)
        });
        out.lines.push(format!("aim item {} {} {} {:?}", pi, t, count, shown));
        out.lines.push(format!("aim item voice {}", if voice { names.join(" ") } else { "none".into() }));
        // Step 6, m = 1 (0x20DF1): the mouse place goes to MouseSavePos2?; outside the box the mouse moves to (237, 102).
        let mut at = (x, y);
        if shown == crate::iteminfo::Shown::Box {
            if let Some(m) = crate::iteminfo::mouse_open(true, (x, y)) {
                at = m;
                self.warp = Some(at);
            }
        }
        let img = (shown != crate::iteminfo::Shown::Nothing).then_some(img);
        self.over.push_back(Over::Item { img, shown, step: 0, voice, info, kept: (x, y), at });
        true
    }

    /// panel.md "Item info box" step 7 for the gear view box.
    fn item_wait(&mut self, x: i32, y: i32, out: &mut Poll) {
        let held = self.buttons & 3 != 0;
        let Some(Over::Item { img, shown, step, voice, info, kept, at }) = self.over.front_mut() else { return };
        use crate::iteminfo::Shown;
        if *step == 0 && !held {
            *step = 1;
        }
        if *step == 1 {
            let voice_done = *shown == Shown::Nothing && *voice && !crate::sound::screen(|s| s.speaking());
            if !held && !voice_done {
                return;
            }
            if *shown == Shown::Box && (x - at.0).abs() < 10 && (y - at.1).abs() < 10 {
                self.warp = Some(*kept);
            }
            crate::sound::screen(|s| s.stop_all());
            if !*info {
                *step = 3;
            } else {
                *step = 2;
            }
        }
        if *step == 2 && !held {
            *step = 3;
        }
        if *step != 3 {
            return;
        }
        // A picture item stays on the screen (the screen does not draw the page again); the box goes.
        if *shown == Shown::Picture {
            if let Some(img) = img.take() {
                self.base = img;
            }
        }
        self.over.pop_front();
        out.lines.push("aim item end".into());
    }

    /// The tooltip of P's face (number 9) or of a team face (k + 1); elsewhere none.
    fn hover(&mut self, w: &World, x: i32, y: i32) {
        let mut table0 = self.c.remap[0];
        let mut base = std::mem::take(&mut self.base);
        {
            let mut p = Painter { d: &self.pd, buf: &mut base, table0: &mut table0, table29: panel::table29(&self.c) };
            if on_face(x, y) {
                let pi = self.p(w);
                let x_text = format!("${}", self.transfer).into_bytes();
                let t: [&[u8]; 3] = match self.mode {
                    0 if self.asked.get(pi).copied().unwrap_or(0) != 0 => [b"repeat", b"interview", b""],
                    0 => [b"hire", b"", b""],
                    2 => [b"Hire at", b"double", b"salary"],
                    3 => [b"transfer", &x_text, b"now"],
                    _ => [b"", b"", b""],
                };
                self.tip.show(&mut p, 9, (9, 21, 65, 71), t, 1);
            } else if let Some(k) = team_face_at(w, x, y) {
                let (fx, fy) = face_xy(k);
                let t = team_tip(&w.soldiers, k);
                self.tip.show(&mut p, k as u8 + 1, (fx, fy, fx + 28, fy + 25), [t, b"", b""], 0);
            } else {
                self.tip.hide(&mut p);
            }
        }
        self.base = base;
    }

    fn click(&mut self, w: &mut World, x: i32, y: i32, out: &mut Poll) {
        if on_face(x, y) {
            if balance(w) < 1 {
                return;
            }
            let pi = self.p(w);
            match self.mode {
                1 => {
                    let (text, left) = hired_text(&w.profiles[pi * PROFILE..(pi + 1) * PROFILE]);
                    self.over.push_back(Over::Msg { text, left });
                    self.msg_close = Some(crate::dostime::now_ms() as i64 + HIRED_MS as i64);
                    out.lines.push(format!("aim msg status {}", pi));
                }
                2 => {
                    let o = pi * PROFILE;
                    w.profiles[o + 0x4C] = 100;
                    let sal = u16::from_le_bytes([w.profiles[o + 0x28], w.profiles[o + 0x29]]).wrapping_mul(2);
                    w.profiles[o + 0x28..o + 0x2A].copy_from_slice(&sal.to_le_bytes());
                    self.hire(w, 10, out);
                }
                3 => {
                    let old = balance(w);
                    let b = old - self.transfer;
                    w.accounts[0x24..0x28].copy_from_slice(&b.to_le_bytes());
                    if self.transfer > 0 {
                        self.over.push_front(Over::Count(FundsCount::new(old, self.transfer, Some(pi))));
                    } else {
                        self.old_debt_paid(w, pi, out);
                    }
                }
                0 => self.interview(w, out),
                _ => {}
            }
            return;
        }
        if let Some(k) = team_face_at(w, x, y) {
            let s = &w.soldiers[k * SOLDIER..(k + 1) * SOLDIER];
            if s[0x19D] != 0 {
                let (fx, fy) = face_xy(k);
                self.call(w, |cw, h| unsafe {
                    ja1_fire_merc_core(cw, h, k as i32, fx, fy);
                    ja1_team_compact(cw, h, 0);
                });
                out.lines.push(format!("aim remove {}", k));
                let pi = self.p(w);
                let pr = &w.profiles[pi * PROFILE..];
                self.mode = page_mode(pr);
                self.redraw(w);
            } else {
                self.fire_who = (s[0x179], cstr(&s[0x16E..0x177]));
                self.over.push_back(Over::Ask(Ask::Fire(k)));
                self.warp = question::prompt_warp(crate::ctlpanel::get(w, 5) != 0, (x, y));
                out.lines.push(format!("aim ask fire {}", k));
                out.lines.push(format!("aim text {}", String::from_utf8_lossy(&self.ask_text(Ask::Fire(k)).0)));
            }
            return;
        }
        let Some(b) = BUTTONS.iter().position(|&(bx, by, bw, bh)| bx <= x && x <= bx + bw && by <= y && y <= by + bh) else {
            return;
        };
        crate::sound::screen_button(&self.iso, &self.exe, 0);
        self.held = Some(b);
        self.hold = Some(crate::pointer::ButtonHold::press((x, y)));
        out.lines.push(format!("aim button {} down", b));
    }

    /// The action of screen button b (0 EXIT, 1 INFO, 2 GEAR / BIO, 3 and 4 the arrows) after its release.
    fn button_action(&mut self, w: &mut World, b: usize, out: &mut Poll) {
        let p = self.p(w) as i32;
        match b {
            0 => self.leave(w, out),
            1 => self.info(w, out),
            2 => {
                self.view ^= 1;
                AIM_VIEW.store(self.view, std::sync::atomic::Ordering::Relaxed);
                self.redraw(w);
                out.lines.push(format!("aim view {}", self.view));
            }
            3 => out.lines.extend(self.browse(w, p, -1)),
            _ => out.lines.extend(self.browse(w, p, 1)),
        }
    }

    /// strategic.md "The A.I.M. statistics page": the page over the whole screen with its own palette.
    fn info(&mut self, w: &mut World, out: &mut Poll) {
        let Some((img, pal)) = self.info_page(w) else {
            out.lines.push("aim info missing".into());
            return;
        };
        let aim_pal = std::mem::replace(&mut self.pal, pal);
        self.over.push_back(Over::Info { img, aim_pal });
        out.lines.push("aim info".into());
    }

    fn info_page(&mut self, w: &mut World) -> Option<(Vec<u8>, [u8; 768])> {
        // Steps 2 and 3 read nothing that the recalculation changes.
        self.call(w, |cw, h| unsafe { ja1_calc_ledger_all(cw, h) });
        let rating = self.call(w, |cw, h| unsafe { ja1_performance_rating(cw, h) });
        stats_page(&self.main.main, &self.pd, &self.c, &w.soldiers, &w.accounts, rating)
    }

    /// The interview of P (mode 0).
    fn interview(&mut self, w: &mut World, out: &mut Poll) {
        if team_size(w) > 7 {
            self.over.push_back(Over::Msg { text: TEAM_FULL.as_bytes().to_vec(), left: false });
            out.lines.push("aim msg team full".into());
            return;
        }
        let pi = self.p(w);
        let r = self.call(w, |cw, h| unsafe { ja1_aim_refusal(cw, h, pi as i32) });
        out.lines.push(format!("aim interview {} reason {}", pi, r));
        let pr = pi * PROFILE;
        if r == 0 {
            self.hire(w, 10, out);
        } else {
            self.quote(w, r - 1, out);
            if r == 6 {
                self.mode = 2;
            }
            if r == 8 && w.profiles[pr + 0x2F] == 3 {
                // SPEC GAP (check): the quote of this hire; the port shows only the refusal quote.
                self.hire_only(w, out);
            }
            if r == 12 {
                let x = 3 * u16::from_le_bytes([w.profiles[pr + 0x3E], w.profiles[pr + 0x3F]]) as i32;
                if balance(w) < x {
                    self.over.push_back(Over::Msg { text: NO_FUNDS.as_bytes().to_vec(), left: false });
                    out.lines.push("aim msg no funds".into());
                } else {
                    self.mode = 3;
                    self.transfer = x;
                }
            }
        }
        if let Some(n) = self.asked.get_mut(pi) {
            *n = n.wrapping_add(1);
        }
        self.tip = Tooltip::default();
        self.redraw(w);
    }

    /// P says quote q (AIMQUOTE.EDT row 14 P + q; "MEMBER" = the nickname of the merc the refusal names).
    fn quote(&mut self, w: &mut World, q: i32, out: &mut Poll) {
        let pi = self.p(w);
        let (text, m) = quote_text(&self.d.quotes, &mut w.last_quote, &w.profiles, &mut w.text_buf, pi, q);
        if let Some(m) = m {
            // strategic.md "Refusal reason": m is stored in the game state byte (save byte 21276).
            w.quote_member = m;
        }
        let pr = &w.profiles[pi * PROFILE..(pi + 1) * PROFILE];
        let nick = cstr(&pr[..10]).to_vec();
        let sub = crate::ctlpanel::get(w, 5) != 0;
        if sub {
            out.lines.push(format!("aim quote {} {}", pi, q));
        }
        // speech.md "Quote categories": the merc's voice (archive AIM\p, member <p>AIM<q>.8 or its halves) on the
        // speech channel (the maintainer play test #14 D); the box's end stops it
        let (iso, exe) = (self.iso.clone(), self.exe.clone());
        let n = crate::sound::screen(|s| {
            if !s.has_files() {
                *s = crate::sound::Sound::with_files(&iso, &exe);
            }
            let n = s.started.len();
            s.cat_quote("AIM", pi as u8, q.max(0));
            s.started.get(n).cloned()
        });
        out.lines.push(format!("aim voice {}", n.as_deref().map_or("none", |v| v.trim_start_matches("quote:"))));
        out.lines.push(format!("aim speech bytes {}", crate::sound::screen(|s| s.speech_bytes())));
        // strategic.md "P says quote q": without Subtitles the quote box 0x28FB8 runs SpeechWait at (8, 20) (the
        // face at (9, 21)) with no box; with Subtitles the text-only merc-face box runs the same wait at (59, 43) (the
        // box's face at (60, 44), the maintainer play test #29 A). Either way the face blinks and talks with the voice.
        let lip = crate::sound::screen(|s| s.speech_lip());
        let plan = talk_plan(sub, pi as u8, text.len());
        let spot = crate::faceanim::FaceSpot { sheet: crate::balloon::face_sheet(&self.main, &plan.face), x: plan.point.0 + 1, y: plan.point.1 + 1, b: 0 };
        let seed = crate::dostime::now_ms() as u32 ^ 0x5EED;
        let face = crate::faceanim::WaitFace::new(spot, lip.as_ref().map(|l| l.0.as_slice()), seed);
        let quote = if sub { Some((pi as u8, nick, text)) } else { None };
        // Subtitles (the text-only box) or a script (no window clock): only the player ends the wait.
        let rules = crate::faceanim::WaitRules { text_ms: plan.text_ms, voiced: lip.is_some(), hold: plan.hold, subtitles: sub || !self.timed, has_text: false };
        let wait = crate::faceanim::SpeechWait::new(face, rules);
        self.over.push_back(Over::Talk { wait, start: None, quote });
        if !sub {
            out.lines.push(format!("aim talk {} {}", pi, q));
        }
    }

    /// Hire of P with quote q.
    fn hire(&mut self, w: &mut World, q: i32, out: &mut Poll) {
        self.quote(w, q, out);
        self.hire_only(w, out);
    }

    fn hire_only(&mut self, w: &mut World, out: &mut Poll) {
        let pi = self.p(w) as i32;
        let k = self.call(w, |cw, h| unsafe { ja1_aim_hire(cw, h, pi) });
        out.lines.push(format!("aim hire {} slot {}", pi, k));
        if k >= 0 {
            self.mode = 1;
        }
        self.redraw(w);
    }

    /// Leaving (EXIT, Escape): the terminate question, or the helicopter charge.
    fn leave(&mut self, w: &mut World, out: &mut Poll) {
        let (cant_afford, ask) = leave_first(pending_count(&w.soldiers), balance(w), self.heli);
        if cant_afford {
            self.over.push_back(Over::Msg { text: CANT_AFFORD.as_bytes().to_vec(), left: false });
            out.lines.push("aim msg cannot afford".into());
        }
        self.over.push_back(Over::Ask(ask));
        out.lines.push(match ask {
            Ask::Heli => format!("aim ask heli {}", self.heli),
            _ => "aim ask terminate".into(),
        });
        out.lines.push(format!("aim text {}", String::from_utf8_lossy(&self.ask_text(ask).0)));
    }

    /// panel.md "The travel animation": the row of the question in front plays (the wait row starts when its
    /// question comes to the front); a go row ends after its last frame step.
    fn sanim_step(&mut self, w: &mut World, x: i32, y: i32, out: &mut Poll) {
        if self.ask_down.is_some() || self.go_wait.as_ref().is_some_and(|g| g.in_release()) {
            return;
        }
        let wait = match self.over.front() {
            Some(Over::Ask(Ask::Heli)) => Some(HELI_ROWS.0),
            Some(Over::Ask(Ask::Terminate { .. })) => Some(TERMINATE_ROWS.0),
            Some(Over::Go(_)) => None,
            _ => {
                self.sanim_stop();
                return;
            }
        };
        if let Some(r) = wait {
            if self.sanim.as_ref().map(|x| x.0) != Some(r) {
                self.sanim = crate::sanim::load(&self.iso, &self.exe, SANIM_ARC, r, None).ok().map(|a| (r, a, 1, None));
                // ConfirmDialog 0x2764B: a mouse outside the box moves to (237, 102); flags 1, 0, 0: it stays there.
                crate::pointer::mouse_save((x, y)); // ConfirmDialog 0x27646
                if let Some(at) = crate::confirm::open_warp((x, y)) {
                    self.warp = Some(at);
                }
                if let Some((_, a, _, _)) = &self.sanim {
                    if a.sound != -1 {
                        crate::sound::screen_sfx(&self.iso, &self.exe, (a.sound as i32, 0x2B11, 20, a.sound_loop as i32));
                    }
                }
            }
        }
        let timed = self.timed;
        let Some((_, a, k, at)) = &mut self.sanim else { return };
        if at.is_some_and(|t| !timed || !frame_due(t.elapsed().as_millis() as u32, a.ms)) {
            return;
        }
        let first = at.is_none();
        *at = Some(std::time::Instant::now());
        let mut k0 = if first { 0 } else { *k };
        if k0 >= a.shown() {
            if wait.is_none() {
                if let Some(Over::Go(g)) = self.over.pop_front() {
                    self.go_end(w, g, out);
                }
                return;
            }
            k0 = 0;
        }
        *k = k0 + 1;
    }

    /// The question box ends (its rows): every sound stops (the original's 0x5CE08).
    fn sanim_stop(&mut self) {
        if self.sanim.take().is_some() {
            crate::sound::screen(|s| s.stop_all());
        }
    }

    /// The go row of question `a` ended (its last frame, a press or a key): the box goes, then the answer yes acts.
    fn go_end(&mut self, w: &mut World, a: Ask, out: &mut Poll) {
        self.sanim_stop();
        out.lines.push("aim go end".into());
        self.answer(w, a, true, out);
    }

    fn answer(&mut self, w: &mut World, a: Ask, yes: bool, out: &mut Poll) {
        if !yes {
            // panel.md "The travel animation": no row: the box ends, every sound stops.
            self.sanim_stop();
        }
        match (a, yes) {
            (Ask::Terminate { .. } | Ask::Heli, true) if self.sanim.is_some() => {
                // panel.md "The travel animation": the yes row first; the answer acts at its end (go_end).
                let row = if a == Ask::Heli { HELI_ROWS.1 } else { TERMINATE_ROWS.1 };
                self.sanim = crate::sanim::load(&self.iso, &self.exe, SANIM_ARC, row, None).ok().map(|g| (row, g, 1, None));
                match &self.sanim {
                    Some((_, g, _, _)) => {
                        if g.sound != -1 {
                            crate::sound::screen_sfx(&self.iso, &self.exe, (g.sound as i32, 0x2B11, 20, g.sound_loop as i32));
                        }
                        self.over.push_front(Over::Go(a));
                    }
                    None => self.go_end(w, a, out),
                }
            }
            (Ask::Terminate { cancel }, true) => {
                if cancel {
                    self.call(w, |cw, h| unsafe { ja1_aim_cancel_pending(cw, h) });
                    out.lines.push("aim cancel pending".into());
                    self.redraw(w);
                }
                self.over.push_back(Over::Leave);
            }
            (Ask::Heli, true) => {
                let old = balance(w);
                let n = self.call(w, |cw, h| unsafe { ja1_aim_pay(cw, h) });
                out.lines.push(format!("aim pay {} funds {}", n, balance(w)));
                if old > balance(w) {
                    self.over.push_front(Over::Count(FundsCount::new(old, old - balance(w), None)));
                }
                self.redraw(w);
                self.over.push_back(Over::Leave);
            }
            (Ask::Exit, true) => {
                let line = crate::question::quit_line(&self.exe, self.rng.random(23));
                out.lines.push(format!("aim quit {}", String::from_utf8_lossy(&line)));
                out.quit = Some(line);
            }
            (Ask::Heli, false) => {
                self.over.push_back(Over::Ask(ASK_AFTER_HELI_NO));
                out.lines.push("aim ask terminate".into());
            }
            (Ask::Fire(k), true) => {
                // "Team face k": the tooltip goes, then the full FireMerc at the face point (the quote balloon
                // there); not the assignment screen's steps 5 and 8 (AimScreen 0x20B95..0x20C1C).
                let (fx, fy) = face_xy(k);
                let mut table0 = self.c.remap[0];
                let mut base = std::mem::take(&mut self.base);
                self.tip.hide(&mut Painter { d: &self.pd, buf: &mut base, table0: &mut table0, table29: panel::table29(&self.c) });
                self.base = base;
                self.said.clear();
                self.call(w, |cw, h| unsafe {
                    ja1_fire_merc_core(cw, h, k as i32, fx, fy);
                    ja1_team_compact(cw, h, 0);
                });
                if let Some(i) = self.said.iter().position(|(op, _)| *op == 328) {
                    let a = self.said.remove(i).1;
                    out.lines.push(format!("aim fire quote {} {} {} {}", a[0], a[1], a[2], a[3]));
                    out.fire = Some((a, self.base.clone()));
                }
                out.lines.push(format!("aim fire {}", k));
                let pi = self.p(w);
                let pr = &w.profiles[pi * PROFILE..];
                self.mode = page_mode(pr);
                self.redraw(w);
            }
            _ => {}
        }
    }

    /// The picture on the screen: the base and the first box.
    pub fn picture(&self) -> Vec<u8> {
        let mut img = self.base.clone();
        let mut table0 = self.c.remap[0];
        let mut p = Painter { d: &self.pd, buf: &mut img, table0: &mut table0, table29: panel::table29(&self.c) };
        match self.over.front() {
            Some(Over::Info { img, .. }) | Some(Over::Item { img: Some(img), .. }) => return img.clone(),
            Some(Over::Salary { k, values }) => salary_panel(&mut p, &self.main, *k, values),
            None if self.held.is_some() => pressed_button(&mut p, self.held.unwrap_or(0), self.view),
            Some(Over::Msg { text, left: false }) => {
                crate::msgbox::draw(&mut p, text, &self.c.remap[DARK], crate::msgbox::frame_colour(DARK))
            }
            Some(Over::Msg { text, left: true }) => draw_left(&mut p, text, &self.c.remap[DARK]),
            Some(Over::Quote { profile, nick, text }) => {
                question::draw_face_text(&mut p, &self.main, text, *profile, nick, "PANEL_B.PCC")
            }
            Some(Over::Talk { wait, quote, .. }) => {
                let face = &wait.face;
                if let Some((profile, nick, text)) = quote {
                    question::draw_face_text(&mut p, &self.main, text, *profile, nick, "PANEL_B.PCC");
                }
                face.spot.paint(p.buf, face.anim.e, face.anim.m)
            }
            Some(Over::Ask(a)) => {
                let s = self.ask_text(*a);
                match a {
                    Ask::Fire(_) => {}
                    Ask::Exit => question::draw_panel(&mut p, &self.main, &s.0, Some(question::EXIT.1), None, "PANEL_B.PCC"),
                    _ => question::draw_panel(&mut p, &self.main, &s.0, None, None, "PANEL_B.PCC"),
                }
                if let (Ask::Heli | Ask::Terminate { .. }, Some((_, an, k, _))) = (a, &self.sanim) {
                    an.put(p.buf, k.saturating_sub(1));
                }
                if let Some(k) = self.ask_down {
                    question::button(&mut p, k, true);
                }
                if let (Ask::Fire(_), Some((pr, nick))) = (a, s.1) {
                    question::draw_face(&mut p, &self.main, &s.0, pr, &nick, "PANEL_B.PCC");
                }
            }
            Some(Over::Go(_)) => {
                if let Some((_, an, k, _)) = &self.sanim {
                    question::draw_go_on(&mut p, &self.main, &an.text, "PANEL_B.PCC");
                    an.put(p.buf, k.saturating_sub(1));
                }
            }
            _ => {}
        }
        img
    }

    fn ask_text(&self, a: Ask) -> (Vec<u8>, Option<(u8, Vec<u8>)>) {
        match a {
            Ask::Terminate { .. } => (TERMINATE.as_bytes().to_vec(), None),
            Ask::Exit => (question::EXIT.0.to_vec(), None),
            Ask::Heli => (format!("There is a ${} helicopter charge. Authorize payment?", self.heli).into_bytes(), None),
            Ask::Fire(_) => {
                let (pr, nick) = self.fire_who.clone();
                let mut s = b"Fire ".to_vec();
                s.extend(&nick);
                s.push(b'?');
                (s, Some((pr, nick)))
            }
        }
    }
}

/// AimShowMercBio? 0x21648 with AimDrawStats 0x212D8, AimTextPanel 0x214E8, AimDrawMercItems? 0x2186C and
/// AimDrawTextPanel 0x215BC: the page of profile `pi` over the clean screen. `view` 0 = the biography, else the gear.
#[allow(clippy::too_many_arguments)]
fn page_draw(p: &mut Painter, c: &crate::Common, d: &AimData, main: &invdraw::InvData, pi: usize, profiles: &[u8],
             soldiers: &[u8], view: u8, mode: u8) {
    let pr = &profiles[pi * PROFILE..(pi + 1) * PROFILE];
    if let Ok(m) = panel::archive_member(&main.main, &pi.to_string()) {
        if let Ok((fw, fh, px)) = panel::pcx(&m) {
            for r in 0..50.min(fh) {
                for c in 0..56.min(fw) {
                    p.put(9 + c as i32, 21 + r as i32, px[r * fw + c]);
                }
            }
        }
    }
    let dark = |p: &mut Painter, t: &[u8; 256]| {
        for r in 21..71 {
            for c in 9..65 {
                let o = (r * PW + c) as usize;
                p.buf[o] = t[p.buf[o] as usize];
            }
        }
    };
    if pr[0x2A] == 0 {
        dark(p, &c.remap[DARK]);
        if let Some(d) = &d.deceased {
            // SPEC GAP (check): DECEASED.PCC at the face's place, pixel 0 kept.
            for r in 0..d.1 {
                for c in 0..d.0 {
                    let v = d.2[r * d.0 + c];
                    if v != 0 {
                        p.put(9 + c as i32, 21 + r as i32, v);
                    }
                }
            }
        }
    } else if mode != 0 && pr[0x4B] != 0 {
        dark(p, &c.remap[DARK]);
        p.element(0x3C, 13, 21, None);
    }
    p.text(&cstr(&pr[0x0A..0x28]), 81, 17, false, 0xFF, 0x32, 2);
    let n = |p: &mut Painter, v: i32, d: usize, x: i32, y: i32| invdraw::number(p, v, d, x, y, 0x27, 0x32, 2);
    n(p, u16::from_le_bytes([pr[0x28], pr[0x29]]) as i32, 5, 119, 29);
    for (i, off) in [0x2A, 0x2C, 0x42, 0x2E].iter().enumerate() {
        n(p, pr[*off] as i32, 4, 125, 38 + 9 * i as i32);
    }
    for (i, off) in [0x40, 0x43, 0x41, 0x2D, 0x44].iter().enumerate() {
        n(p, pr[*off] as i32, 3, 204, 29 + 9 * i as i32);
    }
    // ORIGINAL BUG (#2): 0x215BC restores save area 5, but ShowAimScreen saves area 4 after area 5, and their back
    // buffer rects share a row. So the first 32 pixels of row 78 show the screen's row 192 from x 280.
    let (sw, row) = (d.scr.0, 192 * d.scr.0);
    if d.scr.2.len() >= row + sw {
        p.buf[78 * PW as usize + 16..78 * PW as usize + 48].copy_from_slice(&d.scr.2[row + 280..row + 312]);
    }
    p.text(&cstr(&pr[0..10]), 38, 76, true, 0x28, 0x32, 2);    // 0x215BC: colour 0x28
    if view == 0 {
        if pr[0x2A] != 0 {
            let rec = bio_record(&d.bio, pi);
            let text = decode(rec);
            let (ls, _) = crate::msgbox::lines_wrap(&p.d.fonts[2].widths, &text, 217);
            for (i, l) in ls.iter().enumerate() {
                p.text(l, 8, 93 + 9 * i as i32, false, 0x29, 0xFE, 2);
            }
        }
    } else {
        // The text panel ends at y 169 (0x215BC copies 78 rows from y 92): a name's third line of the second row is cut.
        let below = p.buf[170 * PW as usize..176 * PW as usize].to_vec();
        let on = team_slot_in(soldiers, pi as u8);
        if on.map(|k| soldiers[k * SOLDIER + 0x19D] != 0).unwrap_or(true) {
            let mut i = 0;
            for &t in pr[0x50..0x5A].iter().filter(|&&t| t != 0) {
                let (col, row) = (i % 5, i / 5);
                let (x, y) = (8 + 44 * col, 95 + 38 * row);
                let pic = main.item_types.get(t as usize * 12 + 4).copied().unwrap_or(0);
                // the maintainer play test #29 C: colour 0x17 darkens with table 30 (the item's shadow)
                invdraw::picture_shadow(p, main, pic, x, y, &c.remap[30]);
                // the maintainer play test #29 B: the short name of the item type (JA.EXE table, 15 bytes), font 0, line
                // height 8 - 1
                let rec = main.item_names.get(t as usize * 15..t as usize * 15 + 15).unwrap_or(&[]);
                let name = rec[..rec.iter().position(|&b| b == 0).unwrap_or(rec.len())].to_vec();
                let (ls, _) = crate::msgbox::lines_wrap(&p.d.fonts[0].widths, &name, 39);    // 0x2186C: box width 40
                for (k, l) in ls.iter().enumerate() {
                    p.text(l, x + 20, 92 + 22 + 38 * row + 7 * k as i32, true, 0x2A, 0xFE, 0);
                }
                i += 1;
            }
        }
        p.buf[170 * PW as usize..176 * PW as usize].copy_from_slice(&below);
    }
}

/// The digits of the funds box (see `Aim::funds`), clipped to the box.
fn funds_digits(p: &mut Painter, value: i32) {
    let keep: Vec<u8> = p.buf.to_vec();
    invdraw::number(p, value, 5, 282, 183, 0x28, 0x30, 2);
    for y in 183..195 {
        for x in (280..340.min(PW)).filter(|x| !(185..192).contains(&y) || *x >= 312) {
            let o = (y * PW + x) as usize;
            p.buf[o] = keep[o];
        }
    }
}

/// The clean box from the screen picture `scr`, then the digits.
fn funds_box_draw(p: &mut Painter, scr: &(usize, usize, Vec<u8>), value: i32) {
    let mut clean = vec![0u8; 64000];
    crate::office::put(&mut clean, scr, 0, 0);
    for y in 185..192usize {
        p.buf[y * 320 + 280..y * 320 + 312].copy_from_slice(&clean[y * 320 + 280..y * 320 + 312]);
    }
    funds_digits(p, value);
}

/// funds_diff: the count-down of `amount` from `balance`: per step the balance, 1 when a wait follows, and when `boxes`
/// is not empty the 40 x 12 screen part at (280, 183) round the 32 x 7 funds box (FUNDS_PART bytes per step: the digit rows
/// above, below and right of the box must stay as they were). `boxes[..FUNDS_PART]` holds that part of the screen before
/// the call. Returns the steps, at most `max`.
pub const FUNDS_PART: usize = 40 * 12;

pub fn count_test(iso: &str, exe: &str, balance: i32, amount: i32, values: &mut [i32], waits: &mut [u8], boxes: &mut [u8]) -> Result<usize, i32> {
    let (c, pd, main) = (crate::common(iso)?, panel::panel_data(iso, exe)?, invdraw::inv_data(iso, exe)?);
    let m = panel::archive_member(&main.main, "AIMSCRN.PCX").map_err(|_| crate::E_FILE)?;
    let scr = panel::pcx(&m).map_err(|_| crate::E_FILE)?;
    let mut img = vec![0u8; 64000];
    if !boxes.is_empty() {
        for y in 0..12 {
            img[(183 + y) * 320 + 280..(183 + y) * 320 + 320].copy_from_slice(&boxes[y * 40..(y + 1) * 40]);
        }
    }
    let mut table0 = c.remap[0];
    let mut count = FundsCount::new(balance, amount, None);
    let mut n = 0;
    let mut more = amount > 0;
    while more && n < values.len() {
        more = count.step();
        values[n] = count.shown;
        waits[n] = more as u8;
        if !boxes.is_empty() {
            let mut p = Painter { d: &pd, buf: &mut img, table0: &mut table0, table29: panel::table29(&c) };
            funds_box_draw(&mut p, &scr, count.shown);
            for y in 0..12 {
                let o = n * FUNDS_PART + y * 40;
                boxes[o..o + 40].copy_from_slice(&img[(183 + y) * 320 + 280..(183 + y) * 320 + 320]);
            }
        }
        n += 1;
    }
    Ok(n)
}

/// aimpage_diff: AIMSCRN.PCX with the page of profile `pi` on `screen`. Mode = the rule of 0x21648. Returns the mode.
/// aimshow_diff: ShowAimScreen 0x1FEB0 on this world and view. Writes the picture and the palette; returns the page
/// mode, and profile P in bits 8 up.
pub fn show_test(iso: &str, exe: &str, w: &mut World, view: u8, screen: &mut [u8], pal: &mut [u8]) -> Result<i32, i32> {
    let mut a = Aim::loaded(iso, exe, w, 1)?;
    a.view = view;
    a.show(w);
    screen.copy_from_slice(&a.picture());
    pal.copy_from_slice(&a.pal);
    Ok(a.mode as i32 | (a.p(w) as i32) << 8)
}

pub fn page_test(iso: &str, exe: &str, profiles: &[u8], soldiers: &[u8], pi: usize, view: u8, screen: &mut [u8]) -> Result<u8, i32> {
    let (c, pd, main) = (crate::common(iso)?, panel::panel_data(iso, exe)?, invdraw::inv_data(iso, exe)?);
    let m = panel::archive_member(&main.main, "AIMSCRN.PCX").map_err(|_| crate::E_FILE)?;
    let scr = panel::pcx(&m).map_err(|_| crate::E_FILE)?;
    let deceased = panel::archive_member(&main.main, "DECEASED.PCC").ok().and_then(|m| panel::pcx(&m).ok());
    let d = AimData { scr, deceased, bio: edt(iso, exe, "BIOQUOTE.EDT"), quotes: Vec::new() };
    let pr = &profiles[pi * PROFILE..(pi + 1) * PROFILE];
    let mode = page_mode(pr);
    let mut table0 = c.remap[0];
    let mut p = Painter { d: &pd, buf: screen, table0: &mut table0, table29: panel::table29(&c) };
    crate::office::put(p.buf, &d.scr, 0, 0);
    page_draw(&mut p, &c, &d, &main, pi, profiles, soldiers, view, mode);
    Ok(mode)
}

thread_local! {
    static FACE_TEST: std::cell::RefCell<Option<(World, Aim)>> = const { std::cell::RefCell::new(None) };
}

/// aimface_diff: a left click on P's face (AimScreen 0x204B3 .. 0x20E31) with page mode `mode`, after the interview
/// set X = 3 × debt (mode 3). `profiles`, `soldiers`, `accounts` (0x229FA0, P at +0xA4) go in; profiles and accounts
/// come back. Timed off; a funds count runs to its end. Returns the page mode after the click and the script lines.
pub fn face_click_test(iso: &str, exe: &str, profiles: &mut [u8], soldiers: &[u8], accounts: &mut [u8], mode: u8,
                       sub: u8) -> Result<(u8, Vec<String>), i32> {
    FACE_TEST.with(|cell| {
        let mut cell = cell.borrow_mut();
        if cell.is_none() {
            let mut w = World::new_game(iso, exe, 0, 0).map_err(|_| crate::E_FILE)?;
            let (a, _) = Aim::new(iso, exe, &mut w, 1)?;
            *cell = Some((w, a));
        }
        let (w, a) = cell.as_mut().unwrap();
        let n = w.profiles.len().min(profiles.len());
        w.profiles[..n].copy_from_slice(&profiles[..n]);
        let n = w.soldiers.len().min(soldiers.len());
        w.soldiers[..n].copy_from_slice(&soldiers[..n]);
        let n = w.accounts.len().min(accounts.len());
        w.accounts[..n].copy_from_slice(&accounts[..n]);
        crate::ctlpanel::set(w, 5, sub);
        a.timed = false;
        a.over.clear();
        a.tip = Tooltip::default();
        a.mode = mode;
        let pi = a.p(w);
        a.transfer = 3 * u16::from_le_bytes([w.profiles[pi * PROFILE + 0x3E], w.profiles[pi * PROFILE + 0x3F]]) as i32;
        let mut out = Poll::default();
        a.click(w, 30, 40, &mut out);
        while a.count_pass(w, &mut out) {}
        let n = w.profiles.len().min(profiles.len());
        profiles[..n].copy_from_slice(&w.profiles[..n]);
        let n = w.accounts.len().min(accounts.len());
        accounts[..n].copy_from_slice(&w.accounts[..n]);
        Ok((a.mode, out.lines))
    })
}

/// 0x21648: the page's mode byte 0x95F6C: 4 = dead, 0 = free to hire, 1 = away.
fn page_mode(pr: &[u8]) -> u8 {
    if pr[0x2A] == 0 { 4 } else if pr[0x4B] == 0 { 0 } else { 1 }
}

/// 0x50B04: how many of the 8 team records are in the game (+0x182) with profile `p` (+0x179) and hired only in this
/// visit (+0x19D).
pub fn hired_this_visit(soldiers: &[u8], p: u8) -> u8 {
    (0..8).filter(|k| {
        let r = &soldiers[k * SOLDIER..(k + 1) * SOLDIER];
        r[0x182] != 0 && r[0x179] == p && r[0x19D] != 0
    }).count() as u8
}

/// The mouse on P's face (9..64, 21..70).
fn on_face(x: i32, y: i32) -> bool {
    (9..=64).contains(&x) && (21..=70).contains(&y)
}

/// The place of team face k.
/// strategic.md "The salary panel": PANEL_S.PCC with ten numbers at the place of team record k.
/// strategic.md "Button faces": the pressed face of screen button b (view = the view before the press).
pub(crate) fn pressed_button(p: &mut Painter, b: usize, view: u8) {
    let (x, y, ..) = BUTTONS[b];
    match b {
        0..=2 => {
            let label: &[u8] = match (b, view) {
                (0, _) => b"EXIT",
                (1, _) => b"INFO",
                (_, 0) => b"BIO",
                _ => b"GEAR",
            };
            p.element(0x13, x, y, None);
            p.text(label, x + 18, y + 4, true, 0x2B, 2, 1);
        }
        _ => p.element(if b == 3 { 0x16 } else { 0x17 }, x, y, None),
    }
}

/// The ten numbers of the salary panel of soldier record `s`: salary, hp, the stats +0x18B .. +0x192.
pub(crate) fn salary_values(s: &[u8]) -> [i32; 10] {
    let mut values = [0i32; 10];
    values[0] = u16::from_le_bytes([s[0x110], s[0x111]]) as i32;
    values[1] = s[0x183] as i8 as i32;
    for (i, v) in values[2..].iter_mut().enumerate() {
        *v = s[0x18B + i] as i8 as i32;
    }
    values
}

pub(crate) fn salary_panel(p: &mut Painter, main: &InvData, k: usize, values: &[i32; 10]) {
    let x = if k % 2 == 0 { 170 } else { 214 };
    let y = (40 * (k as i32 / 2) - 13).clamp(5, 99);
    let mut scratch = vec![0u8; 64000];
    let mut table0 = *p.table0;
    {
        let mut q = Painter { d: p.d, buf: &mut scratch, table0: &mut table0, table29: p.table29 };
        crate::invdraw::load_pcx(&mut q, main, "PANEL_S.PCC", 0, 0);
        for (i, &v) in values.iter().enumerate() {
            crate::invdraw::number(&mut q, v, 5, 55, 8 + 8 * i as i32, 0x27, 0x32, 2);
        }
    }
    for r in 0..99 {
        for c in 0..96 {
            p.put(x + c, y + r, scratch[r as usize * 320 + c as usize]);
        }
    }
}

/// The right button browse repeats while only the right button stays down, 200 ms or more after the last browse.
fn browse_again(repeat: bool, buttons: i32, rep_at: Option<i64>, now: i64) -> bool {
    repeat && buttons & 3 == 2 && rep_at.is_some_and(|t| now - t >= 200)
}

/// A question box row frame stays for its frame time `ms`: the next frame is due when `elapsed` reaches it.
fn frame_due(elapsed: u32, ms: u32) -> bool {
    elapsed >= ms
}

/// The hover word over team face `k`: "remove" when byte 0x19D of its soldier record is set, else "fire".
fn team_tip(soldiers: &[u8], k: usize) -> &'static [u8] {
    if soldiers[k * SOLDIER + 0x19D] != 0 {
        b"remove"
    } else {
        b"fire"
    }
}

pub(crate) fn face_xy(k: usize) -> (i32, i32) {
    (if k % 2 == 0 { 240 } else { 284 }, 21 + 40 * (k as i32 / 2))
}

/// The team face under the mouse ((x, y) .. (x + 27, y + 24) of a record with `inCurSector` ≠ 0).
fn team_face_at(w: &World, x: i32, y: i32) -> Option<usize> {
    (0..8).find(|&k| {
        let (fx, fy) = face_xy(k);
        w.soldiers[k * SOLDIER + 0x182] != 0 && (fx..=fx + 27).contains(&x) && (fy..=fy + 24).contains(&y)
    })
}

/// AimAlreadyHiredMsg? 0x2306C: the box closes after this time when no key or click comes first.
pub const HIRED_MS: u32 = 4000;

/// What the quote box 0x28FB8 does for a quote of `len` text bytes.
pub struct TalkPlan {
    /// Subtitles on: YesNoPrompt 0x28C84 with the text only. Off: no box, SpeechWait 0x28034 on the page.
    pub boxed: bool,
    /// The MAIN.DAT member of the face: the profile number in decimal.
    pub face: String,
    /// The wait's point; the face goes at this point + (1, 1).
    pub point: (i32, i32),
    pub text_ms: i64,
    pub hold: i64,
}

/// 0x28FB8 (EAX text, DL profile, BL 0, ECX 8, stack 20; all 5 callers pass these numbers).
pub fn talk_plan(sub: bool, profile: u8, len: usize) -> TalkPlan {
    TalkPlan {
        boxed: sub,
        face: profile.to_string(),
        // Subtitles: the wait inside the box (question_diff).
        point: if sub { (question::BOX.0 + 7, question::BOX.1 + 6) } else { (8, 20) },
        text_ms: 80 * len as i64,
        hold: 0,
    }
}

/// "<nickname> is <text>." of a merc that is not available (AimAlreadyHiredMsg? 0x2306C; `pr` = the profile record);
/// `true` = the text starts at x 66 (on assignment). The original reads byte +0x4B as signed.
pub fn hired_text(pr: &[u8]) -> (Vec<u8>, bool) {
    let mut s = cstr(&pr[0..10]);
    s.extend_from_slice(b" is ");
    let t = pr[0x4B] as i8;
    let left = t < 50;
    match t {
        50 => s.extend_from_slice(b"waiting to join your team."),
        51.. => s.extend_from_slice(b"already on your team."),
        2..=49 => {
            s.extend_from_slice(b"currently on assignment and is expected to return in \xB3\x1D ");
            s.extend_from_slice(t.to_string().as_bytes());
            s.extend_from_slice(b" \xB3 days.");
        }
        _ => s.extend_from_slice(b"currently on assignment, but is scheduled to return \xB3\x1D tomorrow."),
    }
    (s, left)
}

/// Balance (accounts +0x24).
fn balance(w: &World) -> i32 {
    w.accounts.get(0x24..0x28).map(|b| i32::from_le_bytes([b[0], b[1], b[2], b[3]])).unwrap_or(0)
}

/// The team record (0..7, `inCurSector` ≠ 0) of profile p.
fn team_slot(w: &World, p: u8) -> Option<usize> {
    team_slot_in(&w.soldiers, p)
}

fn team_slot_in(soldiers: &[u8], p: u8) -> Option<usize> {
    (0..8).find(|&k| soldiers[k * SOLDIER + 0x182] != 0 && soldiers[k * SOLDIER + 0x179] == p)
}

/// Team size: the leading run of records with `inCurSector` ≠ 0.
fn team_size(w: &World) -> usize {
    (0..8).take_while(|&k| w.soldiers[k * SOLDIER + 0x182] != 0).count()
}

/// The message box with the text starting at x 66 (AimAlreadyHiredMsg): the box rows of the message box with the
/// box at x 58, the control word 0xB3 c (text colour c; 0xB3 alone: back to 0xFF).
fn draw_left(p: &mut Painter, s: &[u8], dark: &[u8; 256]) {
    let widths = p.d.fonts[2].widths;
    let wd = |b: u8| widths.get((b as usize).wrapping_sub(0x20)).copied().unwrap_or(0) as i32;
    let t: i32 = s.iter().map(|&b| wd(b)).sum();
    let w = if t > 190 { 206 } else { t + 16 };
    let x0 = 58;
    let (ls, rows) = crate::msgbox::lines(&widths, s);
    let darken = |p: &mut Painter, y: i32, h: i32| {
        for r in y..y + h {
            for c in x0..x0 + w {
                if let Some(v) = p.get(c, r) {
                    p.put(c, r, dark[v as usize]);
                }
            }
        }
    };
    darken(p, 58, 5);
    let mut fg = 0xFFu8;
    for (i, l) in ls.iter().enumerate() {
        let y = 63 + 11 * i as i32;
        darken(p, y, 11);
        let mut x = 66;
        for word in l.split(|&b| b == b' ') {
            if word.first() == Some(&0xB3) {
                fg = word.get(1).copied().unwrap_or(0xFF);
                if word.len() < 2 {
                    fg = 0xFF;
                }
                continue;
            }
            p.text(word, x, y, false, fg, 8, 2);
            x += word.iter().map(|&b| wd(b)).sum::<i32>() + wd(b' ');
        }
    }
    let yb = 63 + 11 * rows;
    darken(p, yb, 5);
    p.frame(x0, 58, x0 + w + 1, yb + 6, 1);
    p.frame(x0 - 1, 57, x0 + w, yb + 5, crate::msgbox::frame_colour(DARK));
}

#[cfg(test)]
mod count_tests {
    use super::*;

    /// Ledger rows ja1front:3020/3021 (AimQuoteBox? 0x28FB8; YesNoPrompt? 0x28C84 at 0x28F37 puts the face at
    /// (0x3B, 0x2B) = BOX + (7, 6)): with Subtitles the face goes into the box, else at (8, 20); text 80 ms a byte.
    #[test]
    fn talk_plan_face_points() {
        assert_eq!(talk_plan(true, 3, 10).point, (0x3B, 0x2B));
        assert_eq!(talk_plan(false, 3, 10).point, (8, 20));
        assert_eq!(talk_plan(false, 3, 10).text_ms, 800);
    }

    /// AddFunds? 0x514A4 draws the funds box (AimDrawFunds? 0x513E4) at each count step, the last step too (funds_diff
    /// checks the steps and the screen part against the original). After a count with no follow-up (the helicopter
    /// charge, "Authorize payment?" yes), no page redraw comes: the box must already show the new balance.
    /// Ledger row c2aea50:ja1front:3107 (strategic.md "P says quote q"): while a merc talks on the A.I.M. screen the
    /// face shows (at (9, 21) with no box, or in the text-only merc-face box with Subtitles on, over the page's face).
    #[test]
    fn the_talking_face_shows_on_the_aim_screen() {
        let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
        let (iso, exe) = (root.join("rom/JA1.gog"), root.join("build/game113b/JA.EXE"));
        if !iso.exists() || !exe.exists() {
            return;
        }
        let (iso, exe) = (iso.to_str().unwrap(), exe.to_str().unwrap());
        // Aim::quote sets up the screen sound: no other test may read it meanwhile.
        let _g = crate::sound::SCREEN_TEST_LOCK.lock().unwrap_or_else(|e| e.into_inner());
        for sub in [0u8, 1] {
            let mut w = World::new_game(iso, exe, 0, 0).unwrap();
            crate::ctlpanel::set(&mut w, 5, sub);
            let (mut a, _) = Aim::new(iso, exe, &mut w, 1).unwrap();
            a.timed = false;
            a.redraw(&w);
            a.over.clear();
            let mut out = Poll::default();
            a.quote(&mut w, 0, &mut out);
            assert!(matches!(a.over.front(), Some(Over::Talk { .. })));
            // The animated face: (9, 21) without Subtitles, the box's face (60, 44) with them.
            let Some(Over::Talk { wait, .. }) = a.over.front() else { unreachable!() };
            assert_eq!((wait.face.spot.x, wait.face.spot.y), if sub == 0 { (9, 21) } else { (60, 44) });
            let img = a.picture();
            // Without Subtitles the first frame is the page's own face (eyes and mouth at rest); the box shows at once.
            assert_eq!(img != a.base, sub == 1, "subtitles {sub}");
        }
    }

    /// Ledger row d057aca:ja1front:2654 (panel.md "Face animation", A.I.M. screen): the eyes blink only while the
    /// mouse is off the face, one frame per eye time; on the face no eye frame is drawn.
    #[test]
    fn the_aim_face_blinks_only_off_the_face_once_per_eye_time() {
        let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
        let (iso, exe) = (root.join("rom/JA1.gog"), root.join("build/game113b/JA.EXE"));
        if !iso.exists() || !exe.exists() {
            return;
        }
        let (iso, exe) = (iso.to_str().unwrap(), exe.to_str().unwrap());
        let mut w = World::new_game(iso, exe, 0, 0).unwrap();
        let (mut a, _) = Aim::new(iso, exe, &mut w, 1).unwrap();
        a.timed = false;
        a.mode = 0;
        let due = || Some(crate::faceanim::AimBlink::new(0, crate::dostime::now_ms() - 10, crate::faceanim::AnimRng::new(7)));
        let blinks = |o: &Poll| o.lines.iter().filter(|l| l.starts_with("aim blink")).count();
        // On the face (30, 40) with the eye time due: no eye frame.
        a.blink = due();
        let mut out = Poll::default();
        a.blink_step(&w, 30, 40, &mut out);
        assert_eq!(blinks(&out), 0, "a blink on the face");
        // Off the face (200, 150): one frame when due, none on the next pass (the eye time is 100 ms or more).
        a.blink = due();
        let before = a.base.clone();
        let mut out = Poll::default();
        a.blink_step(&w, 200, 150, &mut out);
        assert_eq!(blinks(&out), 1, "{:?}", out.lines);
        a.blink_step(&w, 200, 150, &mut out);
        a.blink_step(&w, 200, 150, &mut out);
        assert_eq!(blinks(&out), 1, "{:?}", out.lines);
        // The eye frame is drawn into the page at (14, 26) (FaceEyeFrame): the eyes are no longer the open ones.
        assert_eq!(out.lines.iter().find(|l| l.starts_with("aim blink")).unwrap(), &format!("aim blink {}", a.blink_shown));
        assert_ne!(a.blink_shown, 0);
        assert!(a.base != before, "no eye frame on the page");
    }

    /// panel.md "Item info box" step 7 for the gear view box: with the box shown, a press less than 10 pixels from
    /// the place after step 6 (both axes) takes the mouse back to the kept place; Item Info off with no voice waits
    /// for a press, not for a voice end.
    #[test]
    fn the_gear_box_press_takes_the_mouse_back_only_near() {
        let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
        let (iso, exe) = (root.join("rom/JA1.gog"), root.join("build/game113b/JA.EXE"));
        if !iso.exists() || !exe.exists() {
            return;
        }
        let (iso, exe) = (iso.to_str().unwrap(), exe.to_str().unwrap());
        let mut w = World::new_game(iso, exe, 0, 0).unwrap();
        let (mut a, _) = Aim::new(iso, exe, &mut w, 1).unwrap();
        use crate::iteminfo::Shown;
        let item = |shown, voice| Over::Item { img: None, shown, step: 1, voice, info: false, kept: (5, 6), at: (100, 100) };
        let mut press = |a: &mut Aim, x, y, shown, voice, held| {
            a.over.clear();
            a.over.push_back(item(shown, voice));
            a.warp = None;
            a.buttons = if held { 1 } else { 0 };
            a.item_wait(x, y, &mut Poll::default());
            (a.warp, matches!(a.over.front(), Some(Over::Item { step: 1, .. })))
        };
        assert_eq!(press(&mut a, 109, 91, Shown::Box, false, true).0, Some((5, 6)));
        assert_eq!(press(&mut a, 91, 109, Shown::Box, false, true).0, Some((5, 6)));
        assert_eq!(press(&mut a, 110, 100, Shown::Box, false, true).0, None);
        assert_eq!(press(&mut a, 100, 90, Shown::Box, false, true).0, None);
        // No press: Item Info off without a voice keeps waiting.
        assert!(press(&mut a, 0, 0, Shown::Nothing, false, false).1);
        assert!(press(&mut a, 0, 0, Shown::Box, true, false).1);
    }

    /// The browse repeat needs the right button alone, still down, and 200 ms since the last browse.
    #[test]
    fn the_browse_repeats_only_while_the_right_button_stays_down() {
        assert!(browse_again(true, 2, Some(1000), 1200));
        assert!(!browse_again(true, 2, Some(1000), 1199));
        assert!(!browse_again(true, 0, Some(1000), 1500));
        assert!(!browse_again(true, 3, Some(1000), 1500));
        assert!(!browse_again(false, 2, Some(1000), 1500));
        assert!(!browse_again(true, 2, None, 1500));
    }

    /// A row frame is due when its frame time has passed, not 1 ms later.
    #[test]
    fn a_row_frame_is_due_at_its_frame_time() {
        assert!(!frame_due(39, 40));
        assert!(frame_due(40, 40));
        assert!(frame_due(41, 40));
    }

    /// A question box press that is still down (or a Go wait in its release) holds the rows: no animation starts.
    #[test]
    fn a_held_question_press_holds_the_rows() {
        let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
        let (iso, exe) = (root.join("rom/JA1.gog"), root.join("build/game113b/JA.EXE"));
        if !iso.exists() || !exe.exists() {
            return;
        }
        let (iso, exe) = (iso.to_str().unwrap(), exe.to_str().unwrap());
        let _g = crate::sound::SCREEN_TEST_LOCK.lock().unwrap_or_else(|e| e.into_inner());
        let mut w = World::new_game(iso, exe, 0, 0).unwrap();
        let (mut a, _) = Aim::new(iso, exe, &mut w, 1).unwrap();
        a.over.clear();
        a.over.push_front(Over::Ask(Ask::Heli));
        a.ask_down = Some(0);
        a.sanim_step(&mut w, 300, 190, &mut Poll::default());
        assert!(a.sanim.is_none());
        a.ask_down = None;
        a.sanim_step(&mut w, 300, 190, &mut Poll::default());
        assert!(a.sanim.is_some());
        a.sanim_stop();
    }

    /// The hover word of a team face reads byte 0x19D of that face's own soldier record.
    #[test]
    fn the_team_face_word_reads_its_own_record() {
        let mut s = vec![0u8; 8 * SOLDIER];
        s[3 * SOLDIER + 0x19D] = 1;
        assert_eq!(team_tip(&s, 3), b"remove");
        assert_eq!(team_tip(&s, 2), b"fire");
        assert_eq!(team_tip(&s, 4), b"fire");
    }

    /// Ledger row d057aca:ja1front:2894 (panel.md "The travel animation"): a frame of the question box row stays for
    /// its frame time; the next frame comes only after it.
    #[test]
    fn a_question_row_frame_stays_for_its_frame_time() {
        let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
        let (iso, exe) = (root.join("rom/JA1.gog"), root.join("build/game113b/JA.EXE"));
        if !iso.exists() || !exe.exists() {
            return;
        }
        let (iso, exe) = (iso.to_str().unwrap(), exe.to_str().unwrap());
        let _g = crate::sound::SCREEN_TEST_LOCK.lock().unwrap_or_else(|e| e.into_inner());
        let mut w = World::new_game(iso, exe, 0, 0).unwrap();
        let (mut a, _) = Aim::new(iso, exe, &mut w, 1).unwrap();
        a.timed = true;
        a.over.clear();
        a.over.push_front(Over::Ask(Ask::Heli));
        let mut out = Poll::default();
        a.sanim_step(&mut w, 300, 190, &mut out);
        let k = |a: &Aim| a.sanim.as_ref().map(|s| s.2).unwrap();
        let ms = a.sanim.as_ref().map(|s| s.1.ms).unwrap();
        assert!(ms >= 20, "frame time {ms}");
        let k1 = k(&a);
        a.sanim_step(&mut w, 300, 190, &mut out);
        assert_eq!(k(&a), k1, "the next frame came before the frame time");
        std::thread::sleep(std::time::Duration::from_millis(ms as u64 + 15));
        a.sanim_step(&mut w, 300, 190, &mut out);
        assert_ne!(k(&a), k1, "no next frame after the frame time");
        a.sanim_stop();
    }

    #[test]
    fn the_count_down_leaves_the_new_balance_in_the_funds_box() {
        let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
        let (iso, exe) = (root.join("rom/JA1.gog"), root.join("build/game113b/JA.EXE"));
        if !iso.exists() || !exe.exists() {
            return;
        }
        let (iso, exe) = (iso.to_str().unwrap(), exe.to_str().unwrap());
        let mut w = World::new_game(iso, exe, 0, 0).unwrap();
        let (old, new) = (12_345i32, 11_045i32);
        w.accounts[0x24..0x28].copy_from_slice(&old.to_le_bytes());
        let (mut a, _) = Aim::new(iso, exe, &mut w, 1).unwrap();
        a.timed = false;
        a.redraw(&w);
        w.accounts[0x24..0x28].copy_from_slice(&new.to_le_bytes());
        a.over.push_front(Over::Count(FundsCount::new(old, old - new, None)));
        let mut out = Poll::default();
        assert!(a.count_pass(&mut w, &mut out));
        let boxed = |img: &[u8]| (185..192usize).flat_map(|y| img[y * 320 + 280..y * 320 + 312].to_vec()).collect::<Vec<u8>>();
        let counted = boxed(&a.base);
        a.redraw(&w);
        assert_eq!(counted, boxed(&a.base));
    }
}
