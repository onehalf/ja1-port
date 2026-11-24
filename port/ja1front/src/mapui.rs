//! The island map view of the --play window (docs/spec/strategic.md "Sector click on the island map" and "Mouse
//! over the island map"): the picture, the frame of the sector under the mouse, the map mode, the click, the start
//! key and the status line.
use crate::host::{Ja1Host, Ja1World, Rng};
use crate::island;
use crate::mappanel;
use crate::msgbox;
use crate::question;
use crate::panel::{self, Painter};
use crate::world::World;
use std::ffi::c_void;

pub mod guard;

extern "C" {
    fn ja1_map_click(w: *mut Ja1World, h: *const Ja1Host, sector: i32, mode: i32, buttons: i32, warned: *mut u8) -> i32;
    fn ja1_map_start(w: *const Ja1World, button: i32, hinted: *mut u8) -> i32;
    fn ja1_worker_pool(w: *mut Ja1World, h: *const Ja1Host);
    fn ja1_calc_ledger_all(w: *mut Ja1World, h: *const Ja1Host);
    fn ja1_new_game(w: *mut Ja1World, h: *const Ja1Host);
    fn ja1_fire_merc(w: *mut Ja1World, h: *const Ja1Host, slot: i32, x: i32);
    fn ja1_team_compact(w: *mut Ja1World, h: *const Ja1Host, redraw: i32);
    fn ja1_fire_merc_core(w: *mut Ja1World, h: *const Ja1Host, slot: i32, x: i32, y: i32);
}

/// The code of a kept Alt-X in `MapUi::take_kept` (no letter key has it).
pub const KEPT_ALT_X: u8 = 0xFF;

/// strategic.md "The map panel", native wage box.
pub const WAGE_MAX: &str = "Natives' salary is at its maximum!";
pub const NO_DUTY_ASK: &[u8] = b"Hmmm. No one assigned to duty?";
pub const DISCARD_ASK: &[u8] = b"Get rid of this item PERMANENTLY?";
pub const GUIDE_ASK: &[u8] = b"Accept this native guide?";
pub const WAGE_ASK: &[u8] = b"Raise natives' salary? \xB1 (don't expect to lower it later)";
/// panel.md "The travel animation", Callers: the wage question's archive and its wait / yes rows (no row none).
const WAGE_ARC: &str = "SANIM.DAT";
const WAGE_ROWS: (i32, i32) = (4, 5);

/// The texts of the message numbers 1..20 (strategic.md; 18..20 are the guard screen's).
pub const MESSAGES: [&str; 20] = [
    "This sector is in enemy hands!",
    "This sector is not linked!",
    "No one on duty!",
    "No Mercenaries there!",
    "Starting sector already specified!",
    "No Tappers there!",
    "Not enough workers available!",
    "There are no trees there!",
    "Tappers are too scared to work there!",
    "No more tappers will work there!",
    "No Guards there!",
    "No workers available!",
    "Guard capacity reached!",
    "Starting sector not specified!",
    "Costs are higher than you can afford!",
    "You may want to visit the ASSIGNMENT screen where you can outfit your mercs.",
    "To do this, click on the RAISED team cost button in the lower left area of the screen.",
    "Can't mix sectors!",
    "Not enough time!",
    "There are Guards without destinations!",
];

struct MapHost {
    rng: Rng,
    /// The item store records while a call that can store items runs (op 264 stash_put).
    stash: Vec<[u8; 3]>,
    /// The world's `last_quote` during a map click (play::npc_quote).
    last_quote: [u8; 6],
    /// Host op 302 guide_say calls since the window last took them (op, arguments): the face balloons.
    said: Vec<(i32, Vec<i32>)>,
    /// The window starts the guide's voice with the balloon (guide_balloon), not at the op.
    voice_later: bool,
}

/// The nickname in the guide offer's box (strategic.md "The guide's advice" 13): the name of profile `p`.
pub fn guide_nick(profiles: &[u8], p: u8) -> &[u8] {
    let nick = profiles.get(p as usize * 0x5A..p as usize * 0x5A + 10).unwrap_or(&[]);
    &nick[..nick.iter().position(|&b| b == 0).unwrap_or(nick.len())]
}

/// The face buffers of team slots 0..7 as at a sector entry (screen.rs): alive or dead face of the merc's face
/// number, 32 x 32 each.
pub fn team_faces(iso: &str, exe: &str, w: &World) -> Result<Vec<u8>, i32> {
    let d = panel::panel_data(iso, exe)?;
    let mut faces = vec![0u8; 8 * 1024];
    for k in 0..8 {
        let s = &w.soldiers[k * 0x200..(k + 1) * 0x200];
        // LoadAllMercFaces 0x2884C: a slot with inCurSector (+0x182) = 0 gets ClearFaceBuffer 0x46AC4 (zeros), no
        // face; a merc who leaves or dies is freed with FACE_CLEAR (pay.rs merc_leaves, corpse.rs). The last slot
        // keeps hp after merc_leaves (team_compact has no slot 8 to move in): face n = 99 would read past the sheet.
        if s[0x182] == 0 {
            continue;
        }
        let kind = if s[0x183] as i8 > 0 { 0 } else { 1 };
        panel::load_face(iso, &d, s[0x179] as i32, kind, &mut faces[k * 1024..(k + 1) * 1024])?;
    }
    Ok(faces)
}

/// panel.md "The travel animation", Playing: the next frame of a row is due at once for the first step, else when
/// its frame time `ms` passed since the last step `at`.
fn frame_due(at: Option<std::time::Instant>, ms: u32) -> bool {
    !at.is_some_and(|t| (t.elapsed().as_millis() as u32) < ms)
}

/// KeyOrClick 0x67E44: the time limit ends a message box only when no button is held (`pressed` 0: while it is set the
/// original polls its own loop and reads no timer) and the box is not the "Say it again" face box.
fn box_timed_out(not_again: bool, pressed: u8, elapsed_ms: u128, ms: u128) -> bool {
    not_again && pressed == 0 && elapsed_ms >= ms
}

/// The nickname of one soldier record: the bytes at +0x16E up to the first 0, at most 9.
fn slot_nick(s: &[u8]) -> &[u8] {
    let nick = &s[0x16E..0x16E + 9];
    &nick[..nick.iter().position(|&b| b == 0).unwrap_or(9)]
}

/// strategic.md "The day-end screen" part 2: the Tooltip box of payroll face k (0..7) is (13 + 37 k, 20, 41 + 37 k, 45).
fn payroll_tip_box(k: i32) -> (i32, i32, i32, i32) {
    let x0 = 13 + 37 * k;
    (x0, 20, x0 + 28, 45)
}

/// strategic.md "Firing a merc" steps 1 and 2: the question text ("Fire " JA.EXE 0x99EAF + the nickname +0x16E of
/// team slot k + "?"), the face number +0x179 and the nickname of that slot.
fn fire_question_parts(soldiers: &[u8], k: usize) -> (Vec<u8>, u8, &[u8]) {
    let s = &soldiers[k * 0x200..(k + 1) * 0x200];
    let nick = slot_nick(s);
    let mut text = b"Fire ".to_vec();
    text.extend_from_slice(nick);
    text.push(b'?');
    (text, s[0x179], nick)
}

/// All accounts (strategic.md Day accounts "All accounts") on a world outside the map screen (the office ledger).
pub fn count_accounts(w: &mut World) {
    let mut host = MapHost { rng: Rng::new(0), stash: Vec::new(), last_quote: [0; 6], said: Vec::new(), voice_later: false };
    let mut cw = Ja1World::view(w);
    let h = Ja1Host { ctx: &mut host as *mut MapHost as *mut c_void, random: cb_random, call: cb_call, host_ops: 0 };
    // SAFETY: `cw` views `w` and `host` lives for the call.
    unsafe { ja1_calc_ledger_all(&mut cw, &h) };
    cw.store(w);
}

/// `ja1_new_game` (world_load.md section 11) on the world loaded from the new-game save.
pub fn new_game_finish(w: &mut World) {
    let mut host = MapHost { rng: Rng::new(0), stash: Vec::new(), last_quote: [0; 6], said: Vec::new(), voice_later: false };
    let mut cw = Ja1World::view(w);
    let h = Ja1Host { ctx: &mut host as *mut MapHost as *mut c_void, random: cb_random, call: cb_call, host_ops: 0 };
    // SAFETY: `cw` views `w` and `host` lives for the call.
    unsafe { ja1_new_game(&mut cw, &h) };
    cw.store(w);
}

unsafe extern "C" fn cb_random(ctx: *mut c_void, n: i32) -> i32 {
    (*(ctx as *mut MapHost)).rng.random(n)
}

unsafe extern "C" fn cb_call(ctx: *mut c_void, op: i32, args: *const i32, nargs: i32, _out: *mut u8, _size: i32) -> u32 {
    let a = if nargs > 0 && !args.is_null() { std::slice::from_raw_parts(args, nargs as usize) } else { &[] };
    let arg = |i: usize| a.get(i).copied().unwrap_or(0);
    if op == 264 {
        return crate::play::stash_put(&mut (*(ctx as *mut MapHost)).stash, arg(0) as u8, arg(1) as u8, arg(2) as u8);
    }
    if op == 328 {
        // fire_quote (strategic.md "Firing a merc"): `said` keeps the call; the window shows the balloon and starts
        // the voice (main.rs fire_balloon).
        eprintln!("fired merc (profile {}) says AIM quote 9", arg(3));
        crate::play::merc_quote(&mut (*(ctx as *mut MapHost)).last_quote, &mut 0, |_| 0xFF, op, a);
        (*(ctx as *mut MapHost)).said.push((op, a.to_vec()));
        return 0;
    }
    if op == 302 {
        // guide_say (quote 0x2F, category 6): the voice on the screen sound; `said` keeps the call for the face
        // balloon (panel.md "The guide's and Brenda's balloons").
        crate::play::npc_quote(&mut (*(ctx as *mut MapHost)).last_quote, op, a);
        (*(ctx as *mut MapHost)).said.push((op, a.to_vec()));
        eprintln!("guide says quote 47");
        if !(*(ctx as *mut MapHost)).voice_later {
            for l in crate::sound::npc_say(None, op, a) {
                eprintln!("{}", l);
            }
        }
    }
    0
}

/// strategic.md "The map panel", Mouse over the map: accounts +0x70 = s + 1 for the sector s (0..59) under the
/// mouse (a game field: it stays when the mouse leaves the map and goes into the day save).
pub fn mouse_sector(w: &mut World, s: i32) {
    if (0..60).contains(&s) {
        w.accounts[0x70..0x74].copy_from_slice(&(s + 1).to_le_bytes());
    }
}

/// What a key did.
#[derive(PartialEq, Eq, Debug)]
pub enum Key {
    Nothing,
    Changed,
    /// The map view closes.
    Close,
    /// Enter in the day loop's map (world_load.md section 9): the caller runs the start checks and the day start.
    Start,
}

/// A message box that shows: the picture rectangle under it, its text, when it showed, a mouse press seen.
struct MsgBox {
    saved: Vec<u8>,
    text: String,
    since: std::time::Instant,
    /// KeyOrClick 0x67E44: the button (1 left, 2 right) whose release ends the box; 0 = none. While it is set the
    /// original polls in KeyOrClick's own loop: it reads no key and no timer.
    pressed: u8,
    /// The time limit in ms (25 000; inventory.md "inv_extra": 1000).
    ms: u128,
    /// panel.md "Say it again (key L)": the text-only merc-face box (speaker, name, text) instead of a message;
    /// `saved` then holds the whole picture and no time ends it.
    again: Option<(u8, Vec<u8>, Vec<u8>)>,
}

/// Input for a message box.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum BoxInput {
    Key,
    /// A button went down (1 left, 2 right, 0 any other).
    Down(u8),
    /// A button came up (as `Down`).
    Up(u8),
    Time,
}

/// strategic.md "Opening and leaving the assignment screen", Open step 3: the slide sound (volume 0x32).
/// sound.md "Island map sounds": every sector frame plays the faint release sound (OUT) at 22050, volume 1, once;
/// the left click that ends the native wage edit plays it at 11025.
const FRAME_SOUND: crate::payroll::Sfx = (1, 0x5622, 1, 1);
const WAGE_END_SOUND: crate::payroll::Sfx = (1, 0x2B11, 1, 1);
const ASSIGN_SLIDE_SOUND: crate::payroll::Sfx = (5, 0x2B11, 0x32, 2);
/// sound.md "Click sound" (the original's 0x5C880): main-bank effect 0 (IN) at 11025, volume 6, once.
pub const CLICK_SOUND: crate::payroll::Sfx = (0, 0x2B11, 6, 1);

pub struct MapUi {
    pub open: bool,
    /// The day loop's map (world_load.md section 9): Escape and M do nothing, Enter gives `Key::Start`.
    pub day: bool,
    pub iso: String,
    pub exe: String,
    cur: i32,
    mode: i32,
    warned: u8,
    /// The question on the status line.
    msg: Option<&'static str>,
    /// The question box's button (0 NO, 1 OK) held down.
    ask_down: Option<usize>,
    /// The message boxes still to show, and the one that shows.
    boxes: std::collections::VecDeque<(String, u128)>,
    box_up: Option<MsgBox>,
    /// The end of the sound of an assignment screen remark with a box (`sound_held`).
    sound_until: Option<std::time::Instant>,
    /// A box of an assignment screen remark shows: its end lets the rest of the voice play (`Sound::speech_stall`).
    /// The start of the box and the length of the voice: a window with no audio device has no mix position.
    remark_box: Option<(std::time::Instant, std::time::Duration)>,
    /// The frames drawn since the view opened, the last draw of each sector, oldest first (replayed by `redraw`).
    frames: Vec<(i32, u8)>,
    /// Every `frame` call since the view opened (sector, colour): the strategic screen's FlashSectorFrame calls,
    /// compared by the strat_input_diff tape harness.
    pub frame_log: Vec<(i32, u8)>,
    /// Presses of key L that reached SpecialNpcSpeech? 0x29D54 (mapkey_diff).
    pub again_keys: u32,
    /// Every flip run since the view opened (first picture, count, step): the original's PlayFlipAnim calls of the
    /// finance page, compared by the strat_input_diff tape harness.
    pub flip_log: Vec<(i32, i32, i32)>,
    /// The message number of the last sector click (`ja1_map_click`).
    last_click_msg: i32,
    /// The buttons (bit 0 left, bit 1 right) that StrategicScreen's WaitLeftUp + WaitRightUp 0x43A8F, 0x43C15 wait for after a
    /// press on the map: the loop stands still, so no sector frame, no second click and no water rotation.
    map_hold: i32,
    /// `map_hold` ended; the next window pass reads the mouse again (`after_hold`).
    hold_done: bool,
    /// The left button went down in the second wait of `map_hold`.
    hold_left: bool,
    host: MapHost,
    /// The mouse in screen pixels (the last `track`).
    mouse: (i32, i32),
    /// Number field 15 (the mouse sector + 1) once the mouse was over the map.
    mouse_num: Option<i32>,
    /// The native wage box is pressed: the wage when it was pressed.
    wage_edit: Option<i32>,
    /// The wage question is open: the wage before the box was pressed.
    wage_ask: Option<i32>,
    /// panel.md "The travel animation": the wage question's row playing in the box (wait row 4 while the question
    /// is open, yes row 5 after yes), its next frame and the time of the last frame step (None = the first step
    /// comes at once).
    wage_anim: Option<(crate::sanim::Anim, usize)>,
    wage_go: bool,
    /// KeyOrClick 0x67E44 of the yes row (0x277F6): the button that ends the row, until it comes up.
    wage_go_up: i32,
    /// KeyOrClick's start flags in the yes row (bit 0 left, bit 1 right): `invextra::latch` at the row start, kept only
    /// for a button that is still down then (the first poll clears the rest); the release of that button clears it.
    wage_held: u8,
    /// Keys that came while the yes row waited for a button release: KeyOrClick 0x67E44 reads none then, so they stay
    /// in the key queue and the map loop reads them after the box (StrategicScreen 0x436CB has no FlushKeys).
    kept: Vec<u8>,
    wage_at: Option<std::time::Instant>,
    /// The mouse at the start of the wage question (it comes back at the end of the box).
    wage_mouse: (i32, i32),
    /// A number field box (0 or 4) held down (strategic.md "Number field boxes").
    box_down: Option<usize>,
    /// The finance page is open; its back button is held down.
    pub finance: bool,
    /// The mouse limit (0x682AC) of the map screens, the window keeps the pointer inside it.
    pub limit: crate::mouselimit::Limit,
    pub(crate) map_limit: crate::mouselimit::MapLimit,
    /// Every limit set, in order (strat_input_diff).
    pub limit_log: Vec<crate::mouselimit::Limit>,
    back_down: bool,
    /// Tab 3 held down: the processing plants show.
    plants_down: bool,
    /// The finance page was opened once: screen row 199 keeps the last row of MAPFLIP1.PCC.
    flipped: bool,
    /// The assignment screen is open (strategic.md "The assignment screen"): the screen before it opened and the
    /// assignment picture.
    assign: Option<(Vec<u8>, Vec<u8>)>,
    /// Its exit button 9 is held down.
    exit_down: bool,
    /// The start button (button 7 of the button table, 281..306 x 176..191) of the day loop's map or the guard
    /// screen is held: its pressed face shows (the button routine 0x67CD8(7, 1)).
    pub start_down: bool,
    /// The duty menu of the assignment screen is open (strategic.md "The duty menu"); it draws on `img`.
    duty_menu: Option<crate::assign::DutyMenu>,
    /// The mouse buttons held (bit 0 left, bit 1 right) as the duty menu's input poll sees them.
    buttons: i32,
    /// The mouse must move to this screen pixel (the duty menu's Open step 6); the window does it.
    pub warp: Option<(i32, i32)>,
    /// The press of the button that is down (`button_down`): the mouse goes back there at the release.
    hold: Option<crate::pointer::ButtonHold>,
    /// The question "Hmmm. No one assigned to duty?" shows.
    assign_ask: bool,
    /// The merc selected on the assignment screen (strategic.md "Selecting a merc"); its item panel shows.
    selected: Option<usize>,
    /// The store view of the store list (1 at the game start, kept between openings) and its top type.
    store_view: u8,
    store_top: u8,
    /// The store list's shown types and button 14's overlay V (strategic.md "The panel buttons").
    store_shown: [u8; 4],
    store_v: u8,
    /// The current record of the store list (assign_items::NONE = none).
    store_cur: usize,
    /// The discard question is up for a press at this point (strategic.md "Item moves on the item panel" 3).
    discard_ask: Option<(i32, i32)>,
    /// The question "Fire <nickname>?" is up for team slot k (strategic.md "Firing a merc").
    fire_ask: Option<usize>,
    /// The screen under the fired merc's balloon (host op 328), for the window.
    pub fire_base: Option<Vec<u8>>,
    /// Shift and ctrl held (set by the window before a click).
    pub keys: (bool, bool),
    /// The last slot of the assignment picture's list (strategic.md "The panel buttons", previous / next merc).
    assign_last: usize,
    /// The screen of the assignment screen when it differs from the panel buffer (a button face stays on it).
    assign_screen: Option<Vec<u8>>,
    /// The item info box shows (panel.md "Item info box"): the screen before it, back at the next button down.
    info_under: Option<Vec<u8>>,
    /// ItemInfoBox 0x26BB0: the mouse place after the move into a box with text; None = no mouse step at the end.
    info_after: Option<(i32, i32)>,
    /// The tooltip box (panel.md "Tooltip box"; kept for the whole game) and the lit stat row of the assignment
    /// picture (99 = none; strategic.md "Hover on the assignment screen").
    tip: crate::assign::Tooltip,
    lit: u8,
    /// The panel button (0..4 = buttons 10..14) held down.
    panel_down: Option<usize>,
    /// Pictures still to show before `img` (the flip and slide steps), each `anim_ms` ms on the game clock (`tick`).
    pub anim: std::collections::VecDeque<Vec<u8>>,
    /// The wait after the picture of `anim` at the same place (60 = a picture step, 1 = a slide step).
    pub anim_ms: std::collections::VecDeque<i64>,
    /// The sound that starts when the picture of `anim` at the same place shows (PlayFlipAnim's page sound).
    anim_sfx: std::collections::VecDeque<Option<crate::payroll::Sfx>>,
    /// The mouse pointer while the picture of `anim` at the same place shows: -1 = hidden (PlayFlipAnim's callers call
    /// HidePointer? before it), 201 = shape 0xC9 (the slide of AssignPanelFlipIn? 0x47BD2), 0 = the screen's own rule.
    anim_ptr: std::collections::VecDeque<i32>,
    /// The pointer value that `push_step` gives the next steps.
    step_ptr: i32,
    /// The guard screen is open (tactical key M; strategic.md "The guard screen").
    guard: Option<guard::Guard>,
    /// The picture as shown (320 x 200) and its 8-bit palette.
    pub img: Vec<u8>,
    /// The payroll screen at its end (host op 301 of this morning), the picture under "Not enough funds".
    pub payroll: Option<Vec<u8>>,
    pub pal: [u8; 768],
    /// strategic.md "Water on the island map": the game-clock time of the last rotation (None = not started) and
    /// the rotations since the view opened (both ranges hold 4 colours: the phase is the count mod 4).
    cycled: Option<i64>,
    cycles: u32,
}

impl MapUi {
    pub fn new(iso: &str, exe: &str, seed: u32) -> MapUi {
        MapUi {
            open: false,
            day: false,
            iso: iso.to_string(),
            exe: exe.to_string(),
            cur: -1,
            mode: 0,
            warned: 0,
            msg: None,
            ask_down: None,
            boxes: std::collections::VecDeque::new(),
            box_up: None,
            sound_until: None,
            remark_box: None,
            frames: Vec::new(),
            frame_log: Vec::new(),
            again_keys: 0,
            flip_log: Vec::new(),
            last_click_msg: 0,
            map_hold: 0,
            hold_done: false,
            hold_left: false,
            host: MapHost { rng: Rng::new(seed), stash: Vec::new(), last_quote: [0; 6], said: Vec::new(), voice_later: true },
            mouse: (0, 0),
            mouse_num: None,
            wage_edit: None,
            wage_ask: None,
            wage_anim: None,
            wage_go: false,
            wage_go_up: 0,
            wage_held: 0,
            kept: Vec::new(),
            wage_at: None,
            wage_mouse: (0, 0),
            box_down: None,
            finance: false,
            limit: crate::mouselimit::FULL,
            map_limit: crate::mouselimit::MapLimit::new(),
            limit_log: Vec::new(),
            back_down: false,
            plants_down: false,
            flipped: false,
            assign: None,
            exit_down: false,
            start_down: false,
            duty_menu: None,
            buttons: 0,
            warp: None,
            hold: None,
            assign_ask: false,
            selected: None,
            store_view: 1,
            store_top: 0,
            store_shown: [0; 4],
            store_v: 0x38,
            store_cur: crate::assign_items::NONE,
            discard_ask: None,
            fire_ask: None,
            fire_base: None,
            keys: (false, false),
            assign_last: 0,
            assign_screen: None,
            panel_down: None,
            info_under: None,
            info_after: None,
            tip: Default::default(),
            lit: 99,
            anim: std::collections::VecDeque::new(),
            anim_sfx: std::collections::VecDeque::new(),
            anim_ms: std::collections::VecDeque::new(),
            anim_ptr: std::collections::VecDeque::new(),
            step_ptr: 0,
            guard: None,
            img: vec![0; 64000],
            payroll: None,
            pal: [0; 768],
            cycled: None,
            cycles: 0,
        }
    }

    /// Opens the view: mode 0, no message, the process warning flag 0, `mouse` = the mouse in screen pixels.
    pub fn show(&mut self, w: &mut World, mouse: (i32, i32)) -> Result<(), i32> {
        self.open = true;
        // StrategicScreen 0x432F4 starts with its last sector 0x3C: the mouse counts as below the map.
        self.map_limit = crate::mouselimit::MapLimit::new();
        self.cycled = None;
        self.cycles = 0;
        self.cur = -1;
        self.mode = 0;
        self.warned = 0;
        self.msg = None;
        self.boxes.clear();
        self.box_up = None;
        self.sound_until = None;
        if self.remark_box.take().is_some() {
            crate::sound::screen(|s| s.speech_resume());
        }
        self.frames.clear();
        self.frame_log.clear();
        self.flip_log.clear();
        self.last_click_msg = 0;
        self.map_hold = 0;
        self.hold_done = false;
        self.hold_left = false;
        self.mouse_num = None;
        self.wage_edit = None;
        self.wage_ask = None;
        self.wage_anim = None;
        self.wage_go = false;
        self.box_down = None;
        self.finance = false;
        self.back_down = false;
        self.plants_down = false;
        self.assign = None;
        self.exit_down = false;
        self.start_down = false;
        self.duty_menu = None;
        self.assign_ask = false;
        self.discard_ask = None;
        self.fire_ask = None;
        self.selected = None;
        self.anim.clear();
        self.anim_sfx.clear();
        self.anim_ms.clear();
        self.anim_ptr.clear();
        self.guard = None;
        self.redraw(w)?;
        self.track(w, mouse)
    }

    /// Draws the whole picture, the current frame and the status line.
    /// The screen of the assignment screen from the screen under it and the picture (strategic.md "Opening and
    /// leaving the assignment screen", Open step 3: the end of the slide).
    fn assign_base(under: &[u8], pic: &[u8]) -> Vec<u8> {
        let mut img = under.to_vec();
        for y in (1..143).chain(146..200) {
            img[y * 320..(y + 1) * 320].copy_from_slice(&pic[y * 320..(y + 1) * 320]);
        }
        img
    }

    /// A poll of the assignment screen with no button down (strategic.md "Hover on the assignment screen"): the
    /// tooltips and the lit stat row on its screen. True when the screen changed.
    fn assign_hover(&mut self, w: &World) -> Result<bool, i32> {
        let Some((under, pic)) = &self.assign else { return Ok(false) };
        if self.buttons != 0 || self.assign_ask || self.discard_ask.is_some() || self.fire_ask.is_some()
            || self.info_under.is_some()
            || self.panel_down.is_some() || self.exit_down || !self.anim.is_empty() {
            return Ok(false);
        }
        let kept = self.assign_screen.is_some();
        let mut screen = match self.assign_screen.take() { Some(s) => s, None => Self::assign_base(under, pic) };
        let before = screen.clone();
        let (c, d) = (crate::common(&self.iso)?, panel::panel_data(&self.iso, &self.exe)?);
        let mut table0 = c.remap[0];
        let mut p = Painter { d: &d, buf: &mut screen, table0: &mut table0, table29: panel::table29(&c) };
        crate::assign::hover(&mut p, &w.soldiers, self.selected, self.mouse, &mut self.tip, &mut self.lit);
        let changed = screen != before;
        // (#14 B: the screen after item moves lives only here; a poll that changes nothing keeps it)
        if changed || kept {
            self.assign_screen = Some(screen);
        }
        Ok(changed)
    }

    /// True when redraw draws the held item at the mouse (the assignment screen without a box over it).
    pub fn item_pointer(&self, w: &World) -> bool {
        item_pointer_of(self.guard.is_some(), self.assign.is_some(), self.assign_ask || self.fire_ask.is_some() || self.discard_ask.is_some(),
                        w.held[0])
    }

    pub fn redraw(&mut self, w: &World) -> Result<(), i32> {
        if self.guard.is_some() {
            return self.guard_redraw(w);
        }
        if let Some((under, pic)) = &self.assign {
            // strategic.md "Opening and leaving the assignment screen", Open step 3 (the end of the slide).
            self.img = match &self.assign_screen { Some(s) => s.clone(), None => Self::assign_base(under, pic) };
            let (c, d) = (crate::common(&self.iso)?, panel::panel_data(&self.iso, &self.exe)?);
            let mut table0 = c.remap[0];
            let mut p = Painter { d: &d, buf: &mut self.img, table0: &mut table0, table29: panel::table29(&c) };
            mappanel::back_button_at(&mut p, crate::assign::EXIT, self.exit_down);
            if let Some(i) = self.panel_down {
                crate::assign::panel_button(&mut p, i, true, self.store_v);
            }
            if self.assign_ask {
                Self::draw_question(&self.iso, &self.exe, &mut self.img, NO_DUTY_ASK, Some("Q_MARK.PCC"), self.ask_down, None)?;
            } else if let Some(k) = self.fire_ask {
                // strategic.md "Firing a merc" steps 1 and 2: the merc-face question box over the screen.
                let (text, face, nick) = fire_question_parts(&w.soldiers, k);
                Self::draw_face_question(&self.iso, &self.exe, &mut self.img, &text, face, nick, self.ask_down)?;
            } else if self.discard_ask.is_some() {
                // panel.md "Question box" step 2: the cursor item's picture at (26, 18) of the box, colour 0
                let pic = crate::pointer::held_picture(&w.item_types, w.held[0] as usize);
                let item = (w.held[0] != 0).then_some((pic, 26, 18, 0));
                Self::draw_question(&self.iso, &self.exe, &mut self.img, DISCARD_ASK, Some("GARBAGE.PCC"), self.ask_down,
                                    item)?;
            } else if w.held[0] != 0 && self.box_up.is_none() {
                // The cursor item at the mouse (as the inventory screen's frame). A held item is the pointer, so a
                // box (ShowMessage? 0x27BBE: HidePointer?) takes it away until the box ends.
                let (c, d, inv) = (crate::common(&self.iso)?, panel::panel_data(&self.iso, &self.exe)?,
                                   crate::invdraw::inv_data(&self.iso, &self.exe)?);
                let pic = crate::pointer::held_picture(&w.item_types, w.held[0] as usize);
                let mut table0 = c.remap[0];
                let mut p = Painter { d: &d, buf: &mut self.img, table0: &mut table0, table29: panel::table29(&c) };
                crate::pointer::held_item(&mut p, &inv, pic, self.mouse.0, self.mouse.1);
            }
            // inventory.md "inv_extra": the "No Room" box stays on top of the new picture.
            return self.draw_box();
        }
        let marks = island::guard_marks();
        let (img, p6) = island::island_map(&self.iso, &self.exe, &w.sector_state[..540], &marks)?;
        self.img = img;
        self.pal = crate::eod::scale_palette(&p6);
        for _ in 0..self.cycles % 4 {
            self.rotate_water();
        }
        for (sec, col) in self.frames.clone() {
            island::sector_frame(&self.iso, &self.exe, sec, col, &mut self.img)?;
        }
        self.panel(w, None)?;
        if self.start_down {
            let (c, d) = (crate::common(&self.iso)?, panel::panel_data(&self.iso, &self.exe)?);
            let mut table0 = c.remap[0];
            let mut p = Painter { d: &d, buf: &mut self.img, table0: &mut table0, table29: panel::table29(&c) };
            mappanel::back_button_at(&mut p, (281, 176), true);
        }
        if let Some(m) = self.msg {
            self.status(m)?;
        }
        if self.wage_go {
            // panel.md "The travel animation": after yes the box holds the yes row's text and frame.
            if let Some((a, k)) = &self.wage_anim {
                self.img = question::go_screen(&self.iso, &self.exe, &self.img, a)?;
                a.put(&mut self.img, k.saturating_sub(1));
            }
        } else if self.wage_ask.is_some() {
            // panel.md "The travel animation": the wait row in the picture frame.
            Self::draw_question(&self.iso, &self.exe, &mut self.img, WAGE_ASK, None, self.ask_down, None)?;
            if let Some((a, k)) = &self.wage_anim {
                a.put(&mut self.img, k.saturating_sub(1));
            }
        }
        // A box that shows stays on top of the new picture.
        self.draw_box()
    }

    fn frame(&mut self, s: i32, colour: u8) -> Result<(), i32> {
        island::sector_frame(&self.iso, &self.exe, s, colour, &mut self.img)?;
        crate::sound::screen_sfx(&self.iso, &self.exe, FRAME_SOUND);
        self.frame_log.push((s, colour));
        self.frames.retain(|f| f.0 != s);
        self.frames.push((s, colour));
        Ok(())
    }

    /// The mouse at (x, y) in screen pixels: the sector under it. When it changes, the old sector (0..59) gets a
    /// frame of colour 0x6E and the new one a frame of 0xFF (owned) or 0x6B; leaving (y >= 141) only unframes.
    fn set_limit(&mut self, l: crate::mouselimit::Limit) {
        self.limit = l;
        self.limit_log.push(l);
    }

    pub fn track(&mut self, w: &mut World, (x, y): (i32, i32)) -> Result<(), i32> {
        self.mouse = (x, y);
        if self.map_hold != 0 {
            return Ok(());
        }
        if self.guard.is_some() {
            // GuardAssignScreen? 0x429E2, 0x42A51: the same crossing rule as the island map.
            if let Some(l) = self.map_limit.step(y) {
                self.set_limit(l);
            }
            return Ok(());
        }
        if self.duty_menu.is_some() {
            return self.duty_poll(w);
        }
        if self.assign.is_some() {
            let hovered = self.assign_hover(w)?;
            if (w.held[0] != 0 && self.discard_ask.is_none()) || hovered {
                return self.redraw(w);
            }
            return Ok(());
        }
        if self.finance || self.box_up.is_some() || self.plants_down || self.box_down.is_some()
            || self.wage_edit.is_some() || self.wage_ask.is_some() || self.wage_anim.is_some() {
            // The original waits in a loop of its own here (the mouse stays in the panel rows on the page; the wage
            // box and its question wait for a click / answer, so the map behind them tracks nothing, #36 F).
            return Ok(());
        }
        // 0x43412, 0x43925: the limit changes when the mouse crosses row 0x8D.
        if let Some(l) = self.map_limit.step(y) {
            self.set_limit(l);
        }
        let s = island::sector_at(x, y);
        mouse_sector(w, s);
        if (0..60).contains(&s) && self.mouse_num != Some(s + 1) {
            self.mouse_num = Some(s + 1);
            self.panel(w, Some(15))?;
        }
        if s == self.cur {
            return Ok(());
        }
        let old = self.cur;
        self.cur = s;
        if (0..60).contains(&old) {
            self.frame(old, 0x6E)?;
        }
        if (0..60).contains(&s) {
            let c = if w.sector_state[s as usize * 9] & 1 != 0 { 0xFF } else { 0x6B };
            self.frame(s, c)?;
        }
        Ok(())
    }

    /// The map panel (strategic.md "The map panel") on the picture: None = the whole panel (with field 15 and the
    /// pressed wage box when they show), Some(k) = number field k alone.
    fn panel(&mut self, w: &World, k: Option<usize>) -> Result<(), i32> {
        let (c, d, inv) = (crate::common(&self.iso)?, panel::panel_data(&self.iso, &self.exe)?,
                           crate::invdraw::inv_data(&self.iso, &self.exe)?);
        let mut acc = w.accounts.clone();
        acc.resize(0xA8, 0);
        if let Some(n) = self.mouse_num {
            acc[0x70..0x74].copy_from_slice(&n.to_le_bytes());
        }
        let mut table0 = c.remap[0];
        let mut p = Painter { d: &d, buf: &mut self.img, table0: &mut table0, table29: panel::table29(&c) };
        match k {
            Some(k) => mappanel::field(&mut p, &acc, w.team_on_duty, k),
            None if self.finance => {
                mappanel::finance_page(&mut p, &inv, &acc);
                mappanel::back_button(&mut p, self.back_down);
            }
            None => {
                if self.flipped {
                    mappanel::flip(&mut p, &inv, 1);
                }
                mappanel::draw_panel(&mut p, &inv, &acc, w.team_on_duty, self.mode.clamp(0, 2) as usize);
                if let Some(b) = self.box_down {
                    mappanel::field_box(&mut p, &acc, w.team_on_duty, b, true);
                }
                if self.plants_down {
                    let m = island::map_data(&self.iso, &self.exe)?;
                    let sec = w.sector_state.clone();
                    mappanel::processing(&mut p, &m, &acc, &w.plants, &sec, &|b| mappanel::connected_home(&sec, b));
                }
                if self.mouse_num.is_some() {
                    mappanel::field(&mut p, &acc, w.team_on_duty, 15);
                }
                if self.wage_edit.is_some() {
                    mappanel::wage_pressed(&mut p, &acc, w.team_on_duty);
                }
            }
        }
        Ok(())
    }

    /// Worker pool, then All accounts (after a native wage change).
    fn recount(&mut self, w: &mut World) {
        let mut cw = Ja1World::view(w);
        let h = Ja1Host { ctx: &mut self.host as *mut MapHost as *mut c_void, random: cb_random, call: cb_call, host_ops: 0 };
        // SAFETY: as in `click`.
        unsafe {
            ja1_worker_pool(&mut cw, &h);
            ja1_calc_ledger_all(&mut cw, &h);
        }
        cw.store(w);
    }

    fn wage(w: &World) -> i32 {
        i32::from_le_bytes(w.accounts[0x54..0x58].try_into().unwrap())
    }

    fn set_wage(w: &mut World, v: i32) {
        w.accounts[0x54..0x58].copy_from_slice(&v.to_le_bytes());
    }

    /// A left or right click (buttons bit 0 / bit 1) at the mouse: the native wage box while it is pressed, a
    /// sector of the map, a tab or the wage box of the panel.
    pub fn click(&mut self, w: &mut World, buttons: i32) -> Result<(), i32> {
        if self.map_hold != 0 {
            // WaitLeftUp + WaitRightUp read no press. The right button makes the second wait longer; a left press
            // that comes after WaitLeftUp ended stays down for the loop's next round.
            self.map_hold |= buttons & 2;
            self.hold_left |= buttons & 1 != 0 && self.map_hold & 1 == 0;
            return Ok(());
        }
        let d0 = self.button_down();
        let r = self.click_in(w, buttons);
        if !d0 && self.button_down() {
            self.hold = Some(crate::pointer::ButtonHold::press(self.mouse));
            crate::sound::screen_button(&self.iso, &self.exe, 0);
        }
        r
    }

    /// sound.md "Button sounds": a button of the original's button routine is held (question box, number field
    /// box, back button, exit button, panel button).
    fn button_down(&self) -> bool {
        self.ask_down.is_some() || self.box_down.is_some() || self.back_down || self.exit_down || self.panel_down.is_some()
            || self.start_down
    }

    /// The assignment screen takes a press only while none of its buttons shows pressed (a question button, Exit,
    /// a panel button): the original waits for the release inside the button's routine and reads no other press.
    fn press_idle(&self) -> bool {
        self.ask_down.is_none() && !self.exit_down && self.panel_down.is_none()
    }

    fn click_in(&mut self, w: &mut World, buttons: i32) -> Result<(), i32> {
        if self.guard.is_some() {
            return self.guard_click(w, buttons);
        }
        if self.wage_go && buttons != 0 {
            // KeyOrClick 0x67E44: the right button first; the row ends when that button comes up.
            if self.wage_go_up == 0 {
                self.wage_go_up = if buttons & 2 != 0 { 2 } else { 1 };
            }
            return Ok(());
        }
        self.buttons |= buttons;
        if self.assign.is_some() {
            if self.duty_menu.is_some() {
                return self.duty_poll(w);
            }
            if let Some(under) = self.info_under.take() {
                // panel.md "Item info box" step 7: the wait ends on a button down (every sound stops); the screen
                // goes back
                if let Some(at) = self.info_after.take().and_then(|a| crate::iteminfo::mouse_close(true, a, self.mouse)) {
                    eprintln!("assign item box mouse back {} {}", at.0, at.1);
                    self.warp = Some(at);
                }
                crate::sound::screen(|s| s.stop_all());
                self.assign_screen = Some(under);
                return self.redraw(w);
            }
            let idle = self.press_idle();
            if buttons == 2 && idle && self.selected.is_some() && !self.assign_ask && self.discard_ask.is_none()
                && self.fire_ask.is_none() {
                // strategic.md "Right button on the item panel" (the item info box shows until the next press)
                return self.item_press(w, (self.mouse.0 + 1000, self.mouse.1), 0);
            }
            if buttons & 1 == 0 || !idle {
                return Ok(());
            }
            let (x, y) = self.mouse;
            if self.discard_ask.is_some() || self.fire_ask.is_some() {
                if let Some(k) = question::button_at(x, y) {
                    self.ask_down = Some(k);
                    return self.redraw(w);
                }
                return Ok(());
            }
            if !self.assign_ask && self.selected.is_some() {
                if crate::assign::panel_face_at(x, y) {
                    return self.deselect_merc(w);
                }
                if !crate::assign::panel_body_at(x, y) {
                    if let Some(i) = crate::assign::panel_button_at(x, y) {
                        self.panel_down = Some(i);
                        return self.redraw(w);
                    }
                }
                if !crate::assign::exit_at(x, y) {
                    if crate::assign_items::asks_discard(w.held[0], self.keys.1, (x, y)) {
                        // strategic.md "Item moves" 3: the click sound comes before the question.
                        crate::sound::screen_sfx(&self.iso, &self.exe, CLICK_SOUND);
                        self.discard_ask = Some((x, y));
                        self.question_open();
                        return self.redraw(w);
                    }
                    return self.item_press(w, (x, y), 0);
                }
            }
            if !self.assign_ask && self.selected.is_none() {
                if let Some(k) = crate::assign::duty_box_at(x, y) {
                    if crate::assign::in_sector(&w.soldiers, k) {
                        return self.open_duty_menu(w, k);
                    }
                    return Ok(());
                }
                if let Some(k) = crate::assign::face_box_at(x, y) {
                    if crate::assign::in_sector(&w.soldiers, k) {
                        return self.select_merc(w, k);
                    }
                    return Ok(());
                }
            }
            if self.assign_ask {
                if let Some(k) = question::button_at(x, y) {
                    self.ask_down = Some(k);
                    return self.redraw(w);
                }
            } else if crate::assign::exit_at(x, y) {
                self.exit_down = true;
                return self.redraw(w);
            }
            return Ok(());
        }
        if self.wage_ask.is_some() && self.box_up.is_none() && self.ask_down.is_none() && buttons & 1 != 0 {
            // panel.md "Question box" step 4: a left press on NO or OK shows its pressed face.
            if let Some(k) = question::button_at(self.mouse.0, self.mouse.1) {
                self.ask_down = Some(k);
                return self.redraw(w);
            }
            return Ok(());
        }
        if self.box_up.is_some() || self.wage_ask.is_some() || self.plants_down || self.box_down.is_some() || self.back_down {
            return Ok(());
        }
        if self.finance {
            // The page: only the back button reacts.
            let (x, y) = self.mouse;
            let r = mappanel::BACK_RECT;
            if buttons & 1 != 0 && r.0 <= x && x <= r.2 && r.1 <= y && y <= r.3 {
                self.back_down = true;
                return self.redraw(w);
            }
            return Ok(());
        }
        if let Some(w0) = self.wage_edit {
            if buttons & 1 != 0 {
                crate::sound::screen_sfx(&self.iso, &self.exe, WAGE_END_SOUND);
                self.wage_edit = None;
                self.set_limit(crate::mouselimit::FULL);
                self.msg = None;
                if Self::wage(w) != w0 {
                    self.wage_ask = Some(w0);
                    self.wage_anim = crate::sanim::load(&self.iso, &self.exe, WAGE_ARC, WAGE_ROWS.0, None).ok()
                        .map(|a| (a, 1));
                    self.wage_at = None;
                    // ConfirmDialog 0x2764B: a mouse outside the box moves to (237, 102); flags 1, 1, 1: it comes
                    // back at the end of the box.
                    self.wage_mouse = self.mouse;
                    crate::pointer::mouse_save(self.mouse); // ConfirmDialog 0x27646
                    if let Some(at) = crate::confirm::open_warp(self.mouse) {
                        self.warp = Some(at);
                        self.mouse = at;
                    }
                } else {
                    // 0x4363C: WaitLeftUp after the box's end.
                    self.map_hold = 1;
                }
            } else if buttons & 2 != 0 {
                crate::sound::screen_button(&self.iso, &self.exe, 0);
                if Self::wage(w) < 100 {
                    Self::set_wage(w, Self::wage(w) + 1);
                    self.recount(w);
                    self.msg = None;
                } else {
                    self.msg = None;
                    self.redraw(w)?;
                    return self.message_box(WAGE_MAX.to_string());
                }
            }
            return self.redraw(w);
        }
        // The map panel is below the island (mouse y >= 141), whatever sector the map has framed; the original's
        // panel branch at 0x433C2 runs there and never reads its current sector.
        if !(0..60).contains(&self.cur) || self.mouse.1 >= 141 {
            if buttons & 1 == 0 {
                return Ok(());
            }
            let (x, y) = self.mouse;
            let inside = |r: (i32, i32, i32, i32)| r.0 <= x && x <= r.2 && r.1 <= y && y <= r.3;
            if let Some(t) = (0..4).find(|&t| inside(mappanel::TABS[t])) {
                if t == 3 {
                    crate::sound::screen_button(&self.iso, &self.exe, 0);
                    self.plants_down = true;
                    return self.redraw(w);
                }
                if t as i32 != self.mode {
                    crate::sound::screen_button(&self.iso, &self.exe, 0);
                    self.mode = t as i32;
                    return self.redraw(w);
                }
            } else if let Some(b) = [0usize, 4].into_iter().find(|&b| {
                let (bx, by) = mappanel::BOXES[b];
                inside((bx, by, bx + 34, by + 8))
            }) {
                // The field 0 box opens the assignment screen at the release; field 4 the finance page.
                self.box_down = Some(b);
                return self.redraw(w);
            } else if inside(mappanel::WAGE_BOX) {
                crate::sound::screen_button(&self.iso, &self.exe, 0);
                self.wage_edit = Some(Self::wage(w));
                self.set_limit(crate::mouselimit::WAGE_BOX);
                return self.redraw(w);
            }
            return Ok(());
        }
        let mut cw = Ja1World::view(w);
        let h = Ja1Host { ctx: &mut self.host as *mut MapHost as *mut c_void, random: cb_random, call: cb_call, host_ops: 0 };
        // SAFETY: the pointers of `cw` point into `w`, which does not move during the call.
        self.host.last_quote = w.last_quote;
        let n = unsafe { ja1_map_click(&mut cw, &h, self.cur, self.mode, buttons, &mut self.warned) };
        cw.store(w);
        w.last_quote = self.host.last_quote;
        self.last_click_msg = n;
        // 0x43A8F, 0x43C15: every map press waits for its release, but messages 1 and 2 return before the waits; a message
        // box of the other answers ends with its own release in the port.
        if self.mouse.1 < 141 && n == 0 {
            self.map_hold = buttons & 3;
            self.hold_left = false;
            eprintln!("map hold");
        }
        self.redraw(w)?;
        self.message_number(n)
    }

    /// The left button goes up (buttons bit 0): a held box, tab 3 or the back button lets go.
    pub fn release(&mut self, w: &mut World, buttons: i32) -> Result<(), i32> {
        if self.map_hold != 0 {
            self.buttons &= !buttons;
            self.map_hold &= !buttons;
            self.hold_left &= buttons & 1 == 0;
            self.hold_done = self.map_hold == 0;
            if self.hold_done {
                eprintln!("map hold end");
            }
            return Ok(());
        }
        let d0 = self.button_down();
        // ButtonAt? 0x67CD8 plays the release sound and returns; the caller then does the button's action (the exit
        // button's slide sound comes after it).
        let left = buttons & 1 != 0;
        if d0 && left {
            crate::sound::screen_button(&self.iso, &self.exe, 1);
        }
        let r = self.release_in(w, buttons);
        if d0 && !self.button_down() {
            // The mouse goes back before the action of the button: a warp of that action comes later and wins.
            if self.warp.is_none() {
                self.warp = self.hold.take().and_then(|h| h.step(0));
            }
            if !left {
                crate::sound::screen_button(&self.iso, &self.exe, 1);
            }
        }
        r
    }

    fn release_in(&mut self, w: &mut World, buttons: i32) -> Result<(), i32> {
        if self.guard.is_some() {
            return if buttons & 1 != 0 { self.guard_release(w) } else { Ok(()) };
        }
        self.buttons &= !buttons;
        if self.wage_go_up & buttons != 0 {
            return self.end_wage_go(w);
        }
        if self.wage_go && self.wage_go_up == 0 && self.wage_held & buttons as u8 != 0 {
            // KeyOrClick 0x67E44: a button that was down at the row start counts only after it came up (not inside
            // the release wait of the other button: that loop reads no flag).
            self.wage_held &= !(buttons as u8);
            crate::invextra::set_latch(self.wage_held);
            return Ok(());
        }
        if self.duty_menu.is_some() {
            return self.duty_poll(w);
        }
        if buttons & 1 == 0 {
            return Ok(());
        }
        if let Some(k) = self.ask_down.take() {
            if let Some(at) = self.discard_ask.take() {
                self.question_back();
                return self.item_press(w, at, (k == 1) as i32);
            }
            if let Some(m) = self.fire_ask.take() {
                return self.answer_fire(w, m, k == 1);
            }
            if self.assign_ask {
                self.question_back();
                return self.answer_no_duty(w, k == 1);
            }
            return self.answer_wage(w, k == 1);
        }
        if self.exit_down {
            self.exit_down = false;
            return self.leave_assign(w);
        }
        if let Some(i) = self.panel_down.take() {
            return self.panel_button(w, i);
        }
        if self.plants_down {
            // strategic.md "The processing plants": the left button up ends the page with the release sound
            // (DrawProcessingUsage? 0x42328, routine 0x5C900).
            self.plants_down = false;
            crate::sound::screen_button(&self.iso, &self.exe, 1);
            return self.redraw(w);
        }
        if let Some(b) = self.box_down.take() {
            if b == 0 && self.day {
                // Only the day map (StrategicScreen) opens the assignment screen; key M on the tactical screen
                // opens the guard screen in the original (SPEC GAP front-end: the map view instead, the box only
                // presses).
                return self.open_assign(w);
            }
            if b == 4 {
                // strategic.md "The finance page", Open: three flip pictures, then the page.
                self.redraw(w)?;
                self.flip_steps([1, 2, 3])?;
                self.finance = true;
                self.set_limit(crate::mouselimit::PANEL_ROWS);
            }
            return self.redraw(w);
        }
        if self.back_down {
            self.back_down = false;
            self.redraw(w)?;
            self.flip_steps([3, 2, 1])?;
            self.finance = false;
            self.set_limit(crate::mouselimit::FULL);
            self.flipped = true;
            self.mouse_num = None;
            return self.redraw(w);
        }
        Ok(())
    }

    /// Queues the pictures of `img` with MAPFLIPn.PCC in the panel rows.
    fn flip_steps(&mut self, ns: [usize; 3]) -> Result<(), i32> {
        // The original's PlayFlipAnim call (0x51264): first picture, count, step.
        self.flip_log.push((ns[0] as i32, ns.len() as i32, ns[1] as i32 - ns[0] as i32));
        let (c, d, inv) = (crate::common(&self.iso)?, panel::panel_data(&self.iso, &self.exe)?,
                           crate::invdraw::inv_data(&self.iso, &self.exe)?);
        // PlayFlipAnim 0x51264 waits 60 ms before it draws each picture: the old screen shows first, the page sound
        // starts on the first picture, and the caller goes on as soon as the last picture is drawn.
        // StrategicScreen calls HidePointer? before it (0x43741, 0x437EA): no pointer during the flip.
        self.step_ptr = -1;
        self.push_step(self.img.clone(), 60, None);
        for (i, n) in ns.into_iter().enumerate() {
            let mut img = self.img.clone();
            let mut table0 = c.remap[0];
            let mut p = Painter { d: &d, buf: &mut img, table0: &mut table0, table29: panel::table29(&c) };
            mappanel::flip(&mut p, &inv, n);
            self.push_step(img, if i == 2 { 0 } else { 60 }, (i == 0).then_some(crate::payroll::PAGE_SOUND));
        }
        self.step_ptr = 0;
        Ok(())
    }

    /// The picture to show now: the oldest queued flip picture, else `img`.
    /// The guide's quotes (op, arguments) of the map clicks since the last call (panel.md "The guide's and
    /// Brenda's balloons").
    pub fn take_said(&mut self) -> Vec<(i32, Vec<i32>)> {
        std::mem::take(&mut self.host.said)
    }

    pub fn shown(&self) -> &[u8] {
        self.anim.front().unwrap_or(&self.img)
    }

    /// Queues one picture, its wait and the sound that starts with it.
    fn push_step(&mut self, img: Vec<u8>, ms: i64, sfx: Option<crate::payroll::Sfx>) {
        self.anim.push_back(img);
        self.anim_ms.push_back(ms);
        self.anim_sfx.push_back(sfx);
        self.anim_ptr.push_back(self.step_ptr);
        if self.anim.len() == 1 {
            self.start_sfx();
        }
    }

    /// The sound of the picture shown now, once.
    fn start_sfx(&mut self) {
        if let Some(Some(sfx)) = self.anim_sfx.front_mut().map(|x| x.take()) {
            crate::sound::screen_sfx(&self.iso, &self.exe, sfx);
        }
    }

    /// The colours 0xD7..0xDA and 0xDB..0xDE rotate by one (as guard::ROTATE).
    fn rotate_water(&mut self) {
        rotate_water(&mut self.pal);
    }

    /// strategic.md "Water on the island map": one loop round of the island map at `now` ms on the game clock; more
    /// than 500 ms after the last rotation (or the view's opening) the water colours rotate. Nothing rotates while
    /// the screen waits in a box, the finance page, the processing plants, the assignment screen, the wage box or a
    /// map press (the guard screen rotates in `guard_pass`); a rotation that came due in such a wait follows at once.
    /// PlayFlipAnim and the travel animation use the same timer 0x22A3FC, so after them a full wait starts again.
    /// Returns true after a rotation.
    pub fn cycle(&mut self, now: i64) -> bool {
        if !self.anim.is_empty() || self.wage_ask.is_some() || self.wage_anim.is_some() {
            self.cycled = None;
            return false;
        }
        let wait = self.assign.is_some() || self.guard.is_some() || self.finance || self.plants_down
            || self.box_down.is_some() || self.box_up.is_some() || self.msg.is_some() || self.fire_ask.is_some()
            || self.wage_edit.is_some() || self.map_hold != 0;
        let last = *self.cycled.get_or_insert(now);
        if wait || now - last <= 500 {
            return false;
        }
        self.cycled = Some(now);
        self.cycles += 1;
        self.rotate_water();
        true
    }

    /// True while a map press waits for its release (`map_hold`).
    pub fn map_held(&self) -> bool {
        self.map_hold != 0
    }

    /// The window pass after a map press ended: the loop reads the mouse again, also when the mouse did not move.
    pub fn after_hold(&mut self, w: &mut World) -> Result<(), i32> {
        if !std::mem::take(&mut self.hold_done) {
            return Ok(());
        }
        let m = self.mouse;
        self.track(w, m)?;
        if std::mem::take(&mut self.hold_left) {
            return self.click(w, 1);
        }
        Ok(())
    }

    /// One picture step passed (60 ms): drop the oldest queued picture.
    pub fn tick(&mut self) {
        self.anim.pop_front();
        self.anim_ms.pop_front();
        self.anim_sfx.pop_front();
        self.anim_ptr.pop_front();
        self.start_sfx();
    }

    /// The assignment screen is open (its slide-in may still play).
    /// StrategicScreen 0x4390F and GuardAssignScreen? 0x42A3B call HidePointer? when the mouse crosses row 0x8D onto the
    /// map, and ShowMouse? (0x4341C, 0x429EC) when it crosses back: the framed sector shows the mouse there, and a map
    /// press or a status line keeps the pointer off (live 2026-11-05: 0x284F0B = 1 over the map, 0 below).
    pub fn pointer_off_over_map(&self) -> bool {
        !self.map_limit.below && self.assign.is_none()
    }

    pub fn assign_open(&self) -> bool {
        self.assign.is_some()
    }

    /// The pointer of the picture shown now (see `anim_ptr`); None when no flip or slide plays.
    pub fn anim_pointer(&self) -> Option<i32> {
        self.anim_ptr.front().copied()
    }

    /// Every queued picture shown at once (a script has no time): their sounds start in order.
    pub fn drain(&mut self) {
        if !self.anim.is_empty() {
            self.cycled = None;
        }
        while !self.anim.is_empty() {
            self.tick();
        }
    }

    /// T / P / G / Enter / M / Escape, E on the assignment screen (`c` = the upper-case letter, 13 = Enter, 27 = Escape).
    pub fn key(&mut self, w: &mut World, c: u8) -> Result<Key, i32> {
        if self.guard.is_some() {
            return self.guard_key(w, c);
        }
        if self.assign.is_some() {
            if self.ask_down.is_some() || self.exit_down || self.duty_menu.is_some() {
                return Ok(Key::Nothing);
            }
            if let Some(at) = self.discard_ask {
                if let Some(yes) = question::key_answer(c) {
                    self.discard_ask = None;
                    self.question_back();
                    self.item_press(w, at, yes as i32)?;
                    return Ok(Key::Changed);
                }
                return Ok(Key::Nothing);
            }
            if let Some(m) = self.fire_ask {
                if let Some(yes) = question::key_answer(c) {
                    self.fire_ask = None;
                    self.answer_fire(w, m, yes)?;
                    return Ok(Key::Changed);
                }
                return Ok(Key::Nothing);
            }
            if self.assign_ask {
                if let Some(yes) = question::key_answer(c) {
                    self.assign_ask = false;
                    self.question_back();
                    self.answer_no_duty(w, yes)?;
                    return Ok(Key::Changed);
                }
            } else if c == 13 {
                self.leave_assign(w)?;
                return Ok(Key::Changed);
            } else if c == b'L' {
                // panel.md "Say it again (key L)" (AssignScreen? 0x48A6D).
                return self.say_again(w);
            } else if c == b'E' && self.selected.is_some() {
                // strategic.md "Key E: all items into the store"
                self.item_press(w, (-1, 0), 0)?;
                return Ok(Key::Changed);
            }
            return Ok(Key::Nothing);
        }
        if self.box_up.is_some() || self.finance || self.plants_down || self.box_down.is_some() || self.back_down {
            return Ok(Key::Nothing);
        }
        if self.wage_go && self.wage_go_up != 0 {
            // Inside the release wait of KeyOrClick 0x67E44 (the MousePoll loop) no key is read: it stays queued.
            if c != 0 {
                self.kept.push(c);
            }
            return Ok(Key::Nothing);
        }
        if self.wage_go {
            self.end_wage_go(w)?;
            return Ok(Key::Changed);
        }
        if self.wage_ask.is_some() {
            // The wage question (panel.md "Question box" step 4): Enter, O, Y = yes; Escape, N = no.
            if self.ask_down.is_none() {
                if let Some(yes) = question::key_answer(c) {
                    self.answer_wage(w, yes)?;
                    return Ok(Key::Changed);
                }
            }
            return Ok(Key::Nothing);
        }
        match c {
            // MapKeyHandler? 0x42726: the day loop's map has no mode keys (its tabs do that); key T is ToggleMessage
            // 0x3F994 on the Subtitles switch, as on the A.I.M. page. P and G do nothing there.
            b'T' if self.day => self.subtitle_key(w),
            b'P' | b'G' if self.day => Ok(Key::Nothing),
            b'T' | b'P' | b'G' => {
                self.mode = match c {
                    b'T' => 0,
                    b'P' => 1,
                    _ => 2,
                };
                self.redraw(w)?;
                Ok(Key::Changed)
            }
            13 if self.day => Ok(Key::Start),
            27 | b'M' if self.day => Ok(Key::Nothing),
            13 => {
                let cw = Ja1World::view(w);
                // SAFETY: as in `click`.
                let mut hinted = w.hinted;
                let n = unsafe { ja1_map_start(&cw, 0, &mut hinted) };
                w.hinted = hinted;
                if n == 0 {
                    self.open = false;
                    return Ok(Key::Close);
                }
                self.message_number(n)?;
                Ok(Key::Changed)
            }
            27 | b'M' => {
                self.open = false;
                Ok(Key::Close)
            }
            // panel.md "Say it again (key L)" (MapKeyHandler? 0x4271F).
            b'L' => self.say_again(w),
            _ => Ok(Key::Nothing),
        }
    }

    /// MapKeyHandler? 0x42726 (both map screens): the Subtitles switch flips, then the message "Text  on" / "Text  off".
    pub(crate) fn subtitle_key(&mut self, w: &mut World) -> Result<Key, i32> {
        let v = crate::ctlpanel::get(w, 5) ^ 1;
        crate::ctlpanel::set(w, 5, v);
        self.message_box(format!("Text  {}", if v != 0 { "on" } else { "off" }))?;
        Ok(Key::Changed)
    }

    /// panel.md "Say it again (key L)": the last quote's box (PANEL_P.PCC) until a key or a click.
    pub(crate) fn say_again(&mut self, w: &World) -> Result<Key, i32> {
        self.again_keys += 1;
        match crate::balloon::again_text(&self.iso, &self.exe, w) {
            Some(a) => {
                self.box_up = Some(MsgBox { saved: Vec::new(), text: String::new(), since: std::time::Instant::now(),
                                            pressed: 0, ms: 25_000, again: Some(a) });
                self.warp = question::prompt_warp(crate::ctlpanel::get(w, 5) != 0, self.mouse);
                self.draw_box()?;
                Ok(Key::Changed)
            }
            None => Ok(Key::Nothing),
        }
    }

    /// The wage question's answer: at "no" the wage goes back (strategic.md "The map panel" step 3).
    fn answer_wage(&mut self, w: &mut World, yes: bool) -> Result<(), i32> {
        self.wage_anim = None;
        self.cycled = None;
        if let Some(w0) = self.wage_ask.take() {
            if !yes {
                // No row: the box ends, every sound stops (0x5CE08).
                crate::sound::screen(|s| s.stop_all());
                Self::set_wage(w, w0);
                self.recount(w);
                self.warp = Some(self.wage_mouse);
                self.mouse = self.wage_mouse;
            } else if let Ok(a) = crate::sanim::load(&self.iso, &self.exe, WAGE_ARC, WAGE_ROWS.1, None) {
                // panel.md "The travel animation": the yes row starts with its sound (main bank slot, rate 0x2B11,
                // volume 20, the loop value), then plays until it stops or a key or a click comes.
                if a.sound != -1 {
                    crate::sound::screen_sfx(&self.iso, &self.exe, (a.sound as i32, 0x2B11, 20, a.sound_loop as i32));
                }
                self.wage_anim = Some((a, 1));
                self.wage_go = true;
                // ConfirmDialog? 0x277F6 sets no start flag: KeyOrClick starts with 0x284F1A / 0x284F1C as they are.
                let level = self.buttons as u8 & 3;
                self.wage_held = crate::invextra::latch() & level;
                crate::invextra::set_latch(self.wage_held);
                // The first poll: a button that is down without its flag starts the release wait (right first).
                let free = level & !self.wage_held;
                self.wage_go_up = if free & 2 != 0 { 2 } else { (free & 1) as i32 };
                self.wage_at = None;
            }
        }
        self.redraw(w)
    }

    /// The keys kept during a release wait, oldest first: the caller gives each to `key` after the release.
    pub fn take_kept(&mut self) -> Vec<u8> {
        std::mem::take(&mut self.kept)
    }

    /// The yes row of the wage question stops (its last frame, a key or a click): the box goes.
    fn end_wage_go(&mut self, w: &World) -> Result<(), i32> {
        crate::sound::screen(|s| s.stop_all());
        self.warp = Some(self.wage_mouse);
        self.mouse = self.wage_mouse;
        self.wage_go = false;
        self.wage_go_up = 0;
        crate::invextra::set_latch(self.wage_held);
        self.wage_held = 0;
        self.wage_anim = None;
        self.redraw(w)
    }

    /// panel.md "The travel animation", Playing: one frame step of the wage question's row when its frame time
    /// passed since the last step (the first step at once); true when the picture changed. The window calls it on
    /// every pass; a script has no time, so its pictures keep frame 0.
    pub fn anim_step(&mut self, w: &World) -> Result<bool, i32> {
        // Inside PressPanelButton 0x36040 and KeyOrClick 0x67E44 the original runs no PlaySanim.
        if self.ask_down.is_some() || self.wage_go_up != 0 {
            return Ok(false);
        }
        let Some((a, k)) = &mut self.wage_anim else { return Ok(false) };
        if !frame_due(self.wage_at, a.ms) {
            return Ok(false);
        }
        let first = self.wage_at.is_none();
        self.wage_at = Some(std::time::Instant::now());
        let mut k0 = if first { 0 } else { *k };
        if k0 >= a.shown() {
            if self.wage_go {
                return self.end_wage_go(w).map(|_| true);
            }
            k0 = 0;
        }
        a.put(&mut self.img, k0);
        *k = k0 + 1;
        Ok(true)
    }

    /// strategic.md "Opening and leaving the assignment screen", Open.
    fn open_assign(&mut self, w: &mut World) -> Result<(), i32> {
        // AssignScreen 0x4893A: a visit to this screen counts as the start button's hint (messages 16 + 17 stay off).
        w.hinted = 1;
        self.redraw(w)?;
        self.assign_start(|m| m.assign_picture(w))?;
        self.assign_screen = None;
        self.panel_down = None;
        self.lit = 99;
        let n = crate::assign::merc_count(&w.soldiers);
        if n > 0 {
            self.assign_last = n - 1;
        }
        self.selected = None;
        self.store_top = 0;
        self.store_cur = crate::assign_items::NONE;
        w.held[..6].fill(0);                  // strategic.md "Opening": cursor and swap bytes 0..5 = 0
        w.inv.swap[..6].fill(0);
        self.redraw(w)
    }

    /// 0x47AE4: the slide-in steps. The screen as it is before the slide stays beside the panel buffer `pic`; the
    /// original saves it to sheet 0x93E5C and to the back buffer. The panel is drawn in `pic`, so nothing changes `img`.
    fn assign_start(&mut self, pic: impl FnOnce(&mut Self) -> Result<Vec<u8>, i32>) -> Result<(), i32> {
        self.assign_flip_steps()?;
        let pic = pic(self)?;
        self.assign_slide_steps(&pic);
        self.assign = Some((self.img.clone(), pic));
        Ok(())
    }

    /// Test door of `assign_start`: the saved screen for the screen `screen`.
    pub fn assign_start_test(&mut self, screen: &[u8]) -> Result<Vec<u8>, i32> {
        self.img.copy_from_slice(screen);
        self.assign_start(|_| Ok(vec![0u8; 64000]))?;
        Ok(self.assign.take().map(|a| a.0).unwrap_or_default())
    }

    /// AssignPanelFlipIn? 0x47B68, the first part: the three pictures ASSFLIP1..3.PCC over the screen's panel rows.
    fn assign_flip_steps(&mut self) -> Result<(), i32> {
        let (c, d, inv) = (crate::common(&self.iso)?, panel::panel_data(&self.iso, &self.exe)?,
                           crate::invdraw::inv_data(&self.iso, &self.exe)?);
        // PlayFlipAnim 0x51264 waits 60 ms before it draws each picture: the old screen shows first, and the page sound
        // starts on the first picture. 0x47B68 then waits 60 ms on the third picture. HidePointer? 0x47B71 before the
        // flip: no pointer during it.
        self.step_ptr = -1;
        self.push_step(self.img.clone(), 60, None);
        for n in 1..4 {
            let mut img = self.img.clone();
            let mut table0 = c.remap[0];
            let mut p = Painter { d: &d, buf: &mut img, table0: &mut table0, table29: panel::table29(&c) };
            crate::invdraw::load_pcx(&mut p, &inv, &format!("ASSFLIP{}.PCC", n), 0, 146);
            self.push_step(img, 60, (n == 1).then_some(crate::payroll::PAGE_SOUND));
        }
        self.step_ptr = 0;
        Ok(())
    }

    /// AssignPanelFlipIn? 0x47B68, the second part: P's rows 146..199 (a picture step), the slide sound, the 24 slide
    /// steps (as payroll.rs). `pic` = the panel buffer P.
    fn assign_slide_steps(&mut self, pic: &[u8]) {
        const ROW: usize = 320;
        let back = self.img[..143 * ROW].to_vec();
        let mut s = self.anim.back().cloned().unwrap_or_else(|| self.img.clone());
        s[146 * ROW..].copy_from_slice(&pic[146 * ROW..64000]);
        // SetMouseShape?(0xC9) + ShowMouse? 0x47BD2 after RestoreMapTextStrip?: shape 201 through the slide; its end
        // 0x47CE4 sets shape 0 again (MouseShapeAndShow?).
        self.step_ptr = 201;
        self.push_step(s.clone(), 1, Some(ASSIGN_SLIDE_SOUND));
        for j in 0..24usize {
            let n = 136 - 6 * j as i32;
            if n >= 1 {
                let (src, n) = ((7 + 6 * j) * ROW, n as usize * ROW);
                s[3 * ROW..3 * ROW + n].copy_from_slice(&back[src..src + n]);
            }
            let top = (139 - 6 * j) * ROW;
            s[top..143 * ROW].copy_from_slice(&pic[top..143 * ROW]);
            // The loop waits before each step, so nothing waits after the last one; the last picture stays with
            // shape 0 (0x47CE4).
            if j == 23 {
                self.step_ptr = 0;
            }
            self.push_step(s.clone(), if j == 23 { 0 } else { 1 }, None);
        }
    }

    /// flipin_diff: 0x47B68 on this screen and panel buffer: `panel` gets ASSPAN.PCC and the fields 27..30. Returns
    /// each step: its picture, wait and sound.
    pub fn flipin_test(&mut self, screen: &[u8], panel_buf: &mut [u8], acc: &[u8]) -> Result<Vec<(Vec<u8>, i64, Option<crate::payroll::Sfx>, i32)>, i32> {
        self.img.copy_from_slice(screen);
        let (c, d, inv) = (crate::common(&self.iso)?, panel::panel_data(&self.iso, &self.exe)?,
                           crate::invdraw::inv_data(&self.iso, &self.exe)?);
        // The queue takes the sound of its first step at once; here it stays in the list.
        self.assign_flip_steps()?;
        {
            let mut table0 = c.remap[0];
            let mut p = Painter { d: &d, buf: panel_buf, table0: &mut table0, table29: panel::table29(&c) };
            crate::assign::pay_panel(&mut p, &inv, acc);
        }
        self.assign_slide_steps(panel_buf);
        Ok(self.anim.drain(..).zip(self.anim_ms.drain(..)).zip(self.anim_sfx.drain(..)).zip(self.anim_ptr.drain(..))
            .map(|(((i, m), s), p)| (i, m, s, p)).collect())
    }

    /// The face buffers as at a sector entry (screen.rs): alive or dead face of the merc's face number.
    pub fn faces(&self, w: &World) -> Result<Vec<u8>, i32> {
        team_faces(&self.iso, &self.exe, w)
    }

    /// Select merc k (strategic.md "Selecting a merc"): the merc pay panel and its item panel, all 200 rows of the
    /// panel buffer to the screen, the mouse to (50, 26).
    fn select_merc(&mut self, w: &mut World, k: usize) -> Result<(), i32> {
        let (c, d, inv) = (crate::common(&self.iso)?, panel::panel_data(&self.iso, &self.exe)?,
                           crate::invdraw::inv_data(&self.iso, &self.exe)?);
        let faces = self.faces(w)?;
        let counts = w.stash_counts.clone();
        let mut is = self.take_store(w);
        let Some((_, pic)) = &mut self.assign else {
            w.stash_records = is.records;
            return Ok(());
        };
        let (mut t0, mut t1) = (c.remap[0], c.remap[0]);
        let mut pp = Painter { d: &d, buf: pic, table0: &mut t0, table29: panel::table29(&c) };
        let mut sp = Painter { d: &d, buf: &mut self.img, table0: &mut t1, table29: panel::table29(&c) };
        let (so, acc) = (&mut w.soldiers[..9 * 0x200], &w.accounts);
        // strategic.md "Selecting a merc": the click sound.
        crate::sound::screen_sfx(&self.iso, &self.exe, CLICK_SOUND);
        is.with(&counts, |st| crate::assign::select(&mut pp, &mut sp, &inv, so, k, &faces, acc, st));
        let pic = pic.clone();
        self.put_store(w, is);
        self.selected = Some(k);
        self.assign_screen = None;
        self.tip.n = 0;
        self.assign = Some((pic.clone(), pic));
        self.warp = Some(crate::assign::SELECT_MOUSE);
        self.mouse = crate::assign::SELECT_MOUSE;
        self.redraw(w)
    }

    /// A press on the item panel's face (strategic.md "The panel buttons" 1): the picture again, no merc selected,
    /// the mouse to the merc's nickname.
    fn deselect_merc(&mut self, w: &mut World) -> Result<(), i32> {
        let Some(k) = self.selected.take() else { return Ok(()) };
        crate::sound::screen_sfx(&self.iso, &self.exe, CLICK_SOUND);
        let pic = self.assign_picture(w)?;
        self.assign = Some((pic.clone(), pic));
        self.assign_screen = None;
        self.tip.n = 0;
        let m = crate::assign::deselect_mouse(k);
        self.warp = Some(m);
        self.mouse = m;
        self.redraw(w)
    }

    /// The release of panel button i (0..4 = buttons 10..14): the up face, then its action.
    fn panel_button(&mut self, w: &mut World, i: usize) -> Result<(), i32> {
        let Some(k) = self.selected else { return Ok(()) };
        let (c, d, inv) = (crate::common(&self.iso)?, panel::panel_data(&self.iso, &self.exe)?,
                           crate::invdraw::inv_data(&self.iso, &self.exe)?);
        let faces = if i < 2 { self.faces(w)? } else { Vec::new() };
        let n = crate::assign::merc_count(&w.soldiers);
        let last = if n > 0 { n - 1 } else { self.assign_last };
        let counts = w.stash_counts.clone();
        let mut is = self.take_store(w);
        let Some((_, pic)) = &mut self.assign else {
            w.stash_records = is.records;
            return Ok(());
        };
        let mut screen = self.assign_screen.take().unwrap_or_else(|| pic.clone());
        let (mut t0, mut t1) = (c.remap[0], c.remap[0]);
        let mut pp = Painter { d: &d, buf: pic, table0: &mut t0, table29: panel::table29(&c) };
        let mut sp = Painter { d: &d, buf: &mut screen, table0: &mut t1, table29: panel::table29(&c) };
        let (so, acc) = (&mut w.soldiers[..9 * 0x200], &w.accounts);
        let mut sel = k;
        is.with(&counts, |st| {
            if i == 4 {
                crate::assign::toggle_view(st);
            }
            crate::assign::panel_button(&mut sp, i, false, st.v);
            match i {
                0 | 1 => {
                    sel = crate::assign::other_merc(k, last, i == 1);
                    crate::assign::show_merc(&mut pp, &mut sp, &inv, so, sel, &faces, acc, st);
                }
                2 | 3 => {
                    if st.scroll(i == 3) {
                        crate::assign::store_again(&mut pp, &mut sp, &inv, st);
                    }
                }
                _ => crate::assign::store_view(&mut pp, &mut sp, &inv, st),
            }
        });
        self.selected = Some(sel);
        self.put_store(w, is);
        self.assign_screen = Some(screen);
        self.redraw(w)
    }

    /// The item store: the screen's list state with the world's records (out of `w` until put_store).
    fn take_store(&self, w: &mut World) -> crate::assign_items::ItemStore {
        crate::assign_items::ItemStore {
            view: self.store_view, records: std::mem::take(&mut w.stash_records), cur: self.store_cur,
            top: self.store_top, shown: self.store_shown, v: self.store_v,
        }
    }

    fn put_store(&mut self, w: &mut World, st: crate::assign_items::ItemStore) {
        (self.store_view, self.store_cur, self.store_top, self.store_shown, self.store_v) =
            (st.view, st.cur, st.top, st.shown, st.v);
        w.stash_records = st.records;
    }

    /// A left press on the item panel of the selected merc (strategic.md "Item moves on the item panel");
    /// `answer` = the discard question's answer; at.0 < 0 = the key E; at.0 ≥ 1000 = a right press at x − 1000.
    fn item_press(&mut self, w: &mut World, at: (i32, i32), answer: i32) -> Result<(), i32> {
        let Some(k) = self.selected else { return Ok(()) };
        let (c, d, inv) = (crate::common(&self.iso)?, panel::panel_data(&self.iso, &self.exe)?,
                           crate::invdraw::inv_data(&self.iso, &self.exe)?);
        let faces = self.faces(w)?;
        let is = self.take_store(w);
        let keys = self.keys;
        let Some((_, pic)) = &mut self.assign else {
            w.stash_records = is.records;
            return Ok(());
        };
        let mut screen = self.assign_screen.take().unwrap_or_else(|| pic.clone());
        let (mut t0, mut t1) = (c.remap[0], c.remap[0]);
        let pp = Painter { d: &d, buf: pic, table0: &mut t0, table29: panel::table29(&c) };
        let sp = Painter { d: &d, buf: &mut screen, table0: &mut t1, table29: panel::table29(&c) };
        let mut armour = crate::invdraw::armour(&w.soldiers[k * 0x200..(k + 1) * 0x200]);
        let (iso, exe) = (self.iso.clone(), self.exe.clone());
        crate::sound::screen(|s| {
            if !s.has_files() {
                *s = crate::sound::Sound::with_files(&iso, &exe);
                s.load_main();
            }
        });
        let (is, under, extra, info_mouse) = crate::assign_items::window_press(pp, sp, &inv, &faces, w, is, k, at, keys,
                                                                         answer, &mut armour);
        self.info_under = under;
        self.info_after = None;
        if info_mouse {
            // ItemInfoBox 0x260C6: every call of AssignScreen? has the mouse flag 1.
            let moved = crate::iteminfo::mouse_open(true, self.mouse);
            if let Some(m) = moved {
                eprintln!("assign item box mouse {} {}", m.0, m.1);
                self.warp = Some(m);
            }
            self.info_after = Some(moved.unwrap_or(self.mouse));
        }
        self.assign_screen = Some(screen);
        self.put_store(w, is);
        self.redraw(w)?;
        // inventory.md "inv_extra": with Subtitles on the text shows in the message box for 1000 ms (table 32 here)
        if let Some(t) = extra {
            let voice = crate::sound::screen(|s| {
                s.speech_stall();
                s.speech_left()
            });
            self.message_box_ms(t.to_string(), 1000)?;
            self.remark_box = Some((std::time::Instant::now(), voice));
        }
        Ok(())
    }

    /// The assignment picture of the world (strategic.md "The assignment picture"; it can change duties).
    fn assign_picture(&self, w: &mut World) -> Result<Vec<u8>, i32> {
        let (c, d, inv) = (crate::common(&self.iso)?, panel::panel_data(&self.iso, &self.exe)?,
                           crate::invdraw::inv_data(&self.iso, &self.exe)?);
        let faces = self.faces(w)?;
        let mut pic = vec![0u8; 64000];
        let mut table0 = c.remap[0];
        let mut p = Painter { d: &d, buf: &mut pic, table0: &mut table0, table29: panel::table29(&c) };
        crate::assign::draw(&mut p, &inv, &c.remap[30], &mut w.soldiers[..9 * 0x200], &faces, &w.stat_changed,
                            &w.accounts);
        Ok(pic)
    }

    /// The duty menu opens for slot k (strategic.md "The duty menu", Open).
    fn open_duty_menu(&mut self, w: &mut World, k: usize) -> Result<(), i32> {
        let (c, d, inv) = (crate::common(&self.iso)?, panel::panel_data(&self.iso, &self.exe)?,
                           crate::invdraw::inv_data(&self.iso, &self.exe)?);
        let Some((_, pic)) = &mut self.assign else { return Ok(()) };
        let (mut t0, mut t1) = (c.remap[0], c.remap[0]);
        let mut pp = Painter { d: &d, buf: pic, table0: &mut t0, table29: panel::table29(&c) };
        let mut sp = Painter { d: &d, buf: &mut self.img, table0: &mut t1, table29: panel::table29(&c) };
        let (menu, wy) = crate::assign::DutyMenu::open(&mut pp, &mut sp, &inv, &w.soldiers, &w.accounts, k);
        // strategic.md "The duty menu" Open step 2: the press sound (sound.md "Button sounds", routine 0x5C8A0).
        crate::sound::screen_button(&self.iso, &self.exe, 0);
        self.duty_menu = Some(menu);
        self.warp = Some((self.mouse.0, wy));
        self.mouse.1 = wy;
        Ok(())
    }

    /// strategic.md "Firing a merc" step 1: the question for slot k over the screen as it is. YesNoPrompt? 0x28D88
    /// moves a mouse outside the box only with Subtitles on (question::prompt_warp).
    fn fire_question_open(&mut self, w: &World, k: usize) {
        self.fire_ask = Some(k);
        self.assign_screen = Some(self.img.clone());
        self.warp = question::prompt_warp(crate::ctlpanel::get(w, 5) != 0, self.mouse);
    }

    /// One input poll of the open duty menu with the mouse and the buttons as they are now.
    fn duty_poll(&mut self, w: &mut World) -> Result<(), i32> {
        let (c, d) = (crate::common(&self.iso)?, panel::panel_data(&self.iso, &self.exe)?);
        let Some(menu) = &mut self.duty_menu else { return Ok(()) };
        let mut t0 = c.remap[0];
        let mut sp = Painter { d: &d, buf: &mut self.img, table0: &mut t0, table29: panel::table29(&c) };
        let mut click = false;
        let r = menu.poll(&mut sp, self.mouse.0, self.mouse.1, self.buttons, &mut click);
        if click {
            // A choice in the duty menu or the subject menu: the press sound (routine 0x5C8A0).
            crate::sound::screen_button(&self.iso, &self.exe, 0);
        }
        let Some(ch) = r else { return Ok(()) };
        let k = menu.k;
        self.duty_menu = None;
        if matches!(ch, Some((6, _))) {
            // strategic.md "Firing a merc" step 1: the question over the screen as it is.
            self.fire_question_open(w, k);
            return self.redraw(w);
        }
        if crate::assign::choose(&mut w.soldiers, k, ch) {
            let mut cw = Ja1World::view(w);
            let h = Ja1Host { ctx: &mut self.host as *mut MapHost as *mut c_void, random: cb_random, call: cb_call,
                              host_ops: 0 };
            // SAFETY: as in `click`.
            unsafe { ja1_calc_ledger_all(&mut cw, &h) };
            cw.store(w);
        }
        // The whole picture: all 200 rows of the panel buffer to the screen.
        let pic = self.assign_picture(w)?;
        self.assign = Some((pic.clone(), pic));
        self.redraw(w)
    }

    /// strategic.md "Firing a merc" steps 3..9 for team slot k.
    fn answer_fire(&mut self, w: &mut World, k: usize, yes: bool) -> Result<(), i32> {
        if yes {
            // The screen under the balloon: the question box and the duty menu are gone (their save areas went
            // back: the panel buffer), the team is not yet compacted.
            self.assign_screen = None;
            self.redraw(w)?;
            self.fire_base = Some(self.img.clone());
            let mut cw = Ja1World::view(w);
            self.host.stash = std::mem::take(&mut w.stash_records);
            let h = Ja1Host { ctx: &mut self.host as *mut MapHost as *mut c_void, random: cb_random, call: cb_call,
                              host_ops: 0 };
            // SAFETY: cw points into `w`, which stays in place for the calls; the host outlives them.
            unsafe {
                ja1_fire_merc(&mut cw, &h, k as i32, 11 + 37 * k as i32);
                ja1_team_compact(&mut cw, &h, 0);
                ja1_calc_ledger_all(&mut cw, &h);
            }
            cw.store(w);
            w.stash_records = std::mem::take(&mut self.host.stash);
        }
        self.selected = None;
        self.assign_screen = None;
        let pic = self.assign_picture(w)?;
        self.assign = Some((pic.clone(), pic));
        self.redraw(w)
    }

    /// Leave: at once when someone is on duty, else the question.
    fn leave_assign(&mut self, w: &mut World) -> Result<(), i32> {
        if let Some(k) = self.selected {
            // strategic.md "Selecting a merc": Leave with a selected merc first checks its duty.
            let s = &mut w.soldiers[k * 0x200..(k + 1) * 0x200];
            let f = crate::assign::duty_flags(s);
            if (s[0x17E] == 3 && !f[3]) || (s[0x17E] == 5 && !f[5]) {
                s[0x17E] = 0;
            }
        }
        if w.team_on_duty > 0 {
            return self.close_assign(w);
        }
        self.assign_ask = true;
        self.question_open();
        self.redraw(w)
    }

    /// QuestionBox? 0x279B1 for the two questions of AssignScreen? (0x48B58, 0x49CCA): the place goes to MouseSavePos2?,
    /// a mouse outside the box moves to (237, 102).
    fn question_open(&mut self) {
        crate::pointer::mouse_save(self.mouse);
        if let Some(at) = crate::confirm::open_warp(self.mouse) {
            eprintln!("map question mouse {} {}", at.0, at.1);
            self.warp = Some(at);
        }
    }

    /// QuestionBox? 0x27AF7: these two questions have the return flag 1, so the mouse goes back with no distance test.
    fn question_back(&mut self) {
        let at = crate::pointer::mouse_saved();
        eprintln!("map question mouse back {} {}", at.0, at.1);
        self.warp = Some(at);
    }

    fn answer_no_duty(&mut self, w: &mut World, yes: bool) -> Result<(), i32> {
        self.assign_ask = false;
        if yes {
            return self.close_assign(w);
        }
        self.redraw(w)
    }

    /// Leave steps 1 and 2.
    fn close_assign(&mut self, w: &mut World) -> Result<(), i32> {
        let cur = w.cur_sector as usize;
        if cur < 60 {
            w.sector_state[cur * 9 + 2] = w.team_on_duty as u8;
        }
        let shown = self.img.clone();
        self.assign = None;
        self.assign_screen = None;
        self.panel_down = None;
        self.mouse_num = None;
        self.redraw(w)?;
        // Leave step 2: the slide back. The island map view (rows 0..143) comes down from the top over the
        // assignment picture: the slide sound, then 24 steps j = 0..23, each after a clock wait of 1 ms: screen
        // rows 4 .. 4 + 6 j = map rows 142 - 6 j .. 142. Then the picture steps ASSFLIP3, 2, 1 (the page sound with
        // the first; PlayFlipAnim waits 60 ms before each) and the map with its panel.
        const ROW: usize = 320;
        let map = self.img.clone();
        let mut st = shown;
        self.push_step(st.clone(), 1, Some(ASSIGN_SLIDE_SOUND));
        for j in 0..24usize {
            let n = (1 + 6 * j) * ROW;
            let src = (142 - 6 * j) * ROW;
            st[4 * ROW..4 * ROW + n].copy_from_slice(&map[src..src + n]);
            self.push_step(st.clone(), if j == 23 { 0 } else { 1 }, None);
        }
        self.leave_flip_steps(&st)?;
        // StrategicScreen 0x434F6, after AssignScreen returns: no team on duty clears the current sector, and the
        // map is drawn again.
        if w.team_on_duty == 0 {
            w.cur_sector = 0xFF;
            self.redraw(w)?;
        }
        Ok(())
    }

    /// The flip of Leave step 2: PlayFlipAnim 0x4A3E2, first picture 3, count 3, step -1, over the screen `st`. The
    /// old screen shows 60 ms first, the page sound starts on the first picture (see `flip_steps`).
    fn leave_flip_steps(&mut self, st: &[u8]) -> Result<(), i32> {
        let (c, d, inv) = (crate::common(&self.iso)?, panel::panel_data(&self.iso, &self.exe)?,
                           crate::invdraw::inv_data(&self.iso, &self.exe)?);
        self.flip_log.push((3, 3, -1));
        // HidePointer? 0x4A3CC before the flip: no pointer during it (the slide before keeps it).
        self.step_ptr = -1;
        self.push_step(st.to_vec(), 60, None);
        for n in (1..4).rev() {
            let mut img = st.to_vec();
            let mut table0 = c.remap[0];
            let mut p = Painter { d: &d, buf: &mut img, table0: &mut table0, table29: panel::table29(&c) };
            crate::invdraw::load_pcx(&mut p, &inv, &format!("ASSFLIP{}.PCC", n), 0, 146);
            self.push_step(img, if n == 1 { 0 } else { 60 }, (n == 3).then_some(crate::payroll::PAGE_SOUND));
        }
        self.step_ptr = 0;
        Ok(())
    }

    /// flipin_diff, a plain PlayFlipAnim 0x51264 call of the port: `kind` 0 finance page open, 1 finance page close,
    /// 2 assignment leave, 3 payroll screen (pictures with their wait and sound).
    pub fn flip_test(&mut self, screen: &[u8], kind: i32) -> Result<Vec<(Vec<u8>, i64, Option<crate::payroll::Sfx>)>, i32> {
        self.img.copy_from_slice(screen);
        match kind {
            0 => self.flip_steps([1, 2, 3])?,
            1 => self.flip_steps([3, 2, 1])?,
            2 => self.leave_flip_steps(screen)?,
            _ => {
                let (c, d, inv) = (crate::common(&self.iso)?, panel::panel_data(&self.iso, &self.exe)?,
                                   crate::invdraw::inv_data(&self.iso, &self.exe)?);
                let mut pic = vec![0u8; 64000];
                let mut table0 = c.remap[0];
                let mut p = Painter { d: &d, buf: &mut pic, table0: &mut table0, table29: panel::table29(&c) };
                return Ok(crate::payroll::flip_steps(&mut p, &inv, screen).into_iter()
                    .map(|st| (st.screen, st.ms as i64, st.sound)).collect());
            }
        }
        self.anim_ptr.clear();
        Ok(self.anim.drain(..).zip(self.anim_ms.drain(..)).zip(self.anim_sfx.drain(..)).map(|((i, m), s)| (i, m, s)).collect())
    }

    /// The assignment screen is open.
    /// The merc whose item panel the assignment screen shows.
    pub fn selected_merc(&self) -> Option<usize> {
        self.selected
    }

    pub fn assigning(&self) -> bool {
        self.assign.is_some()
    }

    /// Tab 3's processing page shows (held until the left button comes up).
    pub fn plants_shown(&self) -> bool {
        self.plants_down
    }

    /// The assignment screen's question "Hmmm. No one assigned to duty?" shows.
    pub fn no_duty_asked(&self) -> bool {
        self.assign_ask
    }

    /// The question "Fire <nickname>?" shows.
    pub fn firing(&self) -> bool {
        self.fire_ask.is_some()
    }

    /// The wage question shows.
    pub fn asking(&self) -> bool {
        self.wage_ask.is_some()
    }

    /// The picture with question box `text` (picture `pic`, None = the frame only) on it, button `pressed` held.
    pub fn question_img(&mut self, w: &World, text: &[u8], pic: Option<&str>, pressed: Option<usize>) -> Result<Vec<u8>, i32> {
        self.redraw(w)?;
        let mut img = self.img.clone();
        Self::draw_question(&self.iso, &self.exe, &mut img, text, pic, pressed, None)?;
        Ok(img)
    }

    /// The island map with the guide offer (strategic.md "The guide's advice" 13: the merc-face question box
    /// "Accept this native guide?" with the face and nickname of profile `p`), button `pressed` held.
    pub fn guide_offer_img(&mut self, w: &World, p: u8, pressed: Option<usize>) -> Result<Vec<u8>, i32> {
        self.redraw(w)?;
        let mut img = self.img.clone();
        let nick = guide_nick(&w.profiles, p);
        Self::draw_face_question(&self.iso, &self.exe, &mut img, GUIDE_ASK, p, nick, pressed)?;
        Ok(img)
    }

    /// Host op 301 (strategic.md "The payroll screen"): the shown screens over `under` (the replay screen at the
    /// end of the day replay; None: the island map as it is now); the last one is kept as the morning's picture
    /// (`payroll`).
    pub fn payroll_steps(&mut self, w: &World, under: Option<&[u8]>) -> Result<Vec<crate::payroll::Step>, i32> {
        let (c, d, inv) = (crate::common(&self.iso)?, panel::panel_data(&self.iso, &self.exe)?,
                           crate::invdraw::inv_data(&self.iso, &self.exe)?);
        let faces = self.faces(w)?;
        let mut pic = vec![0u8; 64000];
        let mut table0 = c.remap[0];
        let mut p = Painter { d: &d, buf: &mut pic, table0: &mut table0, table29: panel::table29(&c) };
        let steps = crate::payroll::run(&mut p, &inv, under.unwrap_or(&self.img), &w.soldiers[..9 * 0x200], &faces, &w.stat_changed,
                                        &w.accounts, w.prev_balance);
        self.payroll = steps.last().map(|s| s.screen.clone());
        Ok(steps)
    }

    /// world_load.md section 9 step 1: the morning's "Not enough funds" picture: the payroll screen (its picture
    /// rows 1..142 drawn again for the team as it is now; without a payroll screen the assignment picture of the
    /// world, drawn on a copy: the drawing can change duties) with the message box of `text` (None: no box) and
    /// the rest of the dialog's state `v` (strategic.md "Not enough funds"): fresh panel numbers (POSTFIN.PCC and
    /// payroll fields 43..51), element 103 on the paid faces, the blink (fields 52 and 53 in colour 41) and the
    /// face tooltip "fire". At the end (`v.end`) the screen is the panel buffer: no box, no tooltip, no blink.
    pub fn payday_img(&self, w: &World, text: Option<&str>, v: &crate::dayloop::PayView) -> Result<Vec<u8>, i32> {
        let mut wc = w.clone();
        let (c, d, inv) = (crate::common(&self.iso)?, panel::panel_data(&self.iso, &self.exe)?,
                           crate::invdraw::inv_data(&self.iso, &self.exe)?);
        let mut img = match &self.payroll {
            Some(under) => {
                let faces = self.faces(w)?;
                let mut pic = vec![0u8; 64000];
                let mut table0 = c.remap[0];
                let mut p = Painter { d: &d, buf: &mut pic, table0: &mut table0, table29: panel::table29(&c) };
                crate::payroll::picture(&mut p, &inv, &w.soldiers[..9 * 0x200], &faces, &w.stat_changed);
                if v.fields {
                    crate::payroll::panel(&mut p, &inv, &w.accounts, w.prev_balance);
                }
                for &k in &v.frames {
                    p.element(103, 13 + 37 * k, 20, None);
                }
                let mut img = under.clone();
                img[320..143 * 320].copy_from_slice(&pic[320..143 * 320]);
                if v.fields {
                    img[146 * 320..].copy_from_slice(&pic[146 * 320..]);
                }
                img
            }
            None => self.assign_picture(&mut wc)?,
        };
        let mut table0 = c.remap[0];
        let mut p = Painter { d: &d, buf: &mut img, table0: &mut table0, table29: panel::table29(&c) };
        if v.end {
            return Ok(img);
        }
        if v.blink {
            for k in 52..54 {
                crate::mappanel::pay_field(&mut p, &w.accounts, w.prev_balance, k);
            }
        }
        if let Some(t) = text {
            msgbox::draw(&mut p, t.as_bytes(), &c.remap[msgbox::MAP_TABLE], msgbox::frame_colour(msgbox::MAP_TABLE));
        }
        if (1..=8).contains(&v.tip) {
            crate::assign::Tooltip::default().show(&mut p, v.tip as u8, payroll_tip_box(v.tip - 1), [b"fire", b"", b""], 0);
        }
        Ok(img)
    }

    /// strategic.md "The day-end screen" part 2, no button down: the Tooltip box number k + 1 at (13 + 37 k, 20,
    /// 41 + 37 k, 45), text "fire", font 0, on `img`; None: the tooltip goes.
    pub fn payroll_tip(&self, img: &mut [u8], tip: &mut crate::assign::Tooltip, k: Option<usize>) -> Result<(), i32> {
        let (c, d) = (crate::common(&self.iso)?, panel::panel_data(&self.iso, &self.exe)?);
        let mut table0 = c.remap[0];
        let mut p = Painter { d: &d, buf: img, table0: &mut table0, table29: panel::table29(&c) };
        match k {
            Some(k) => {
                tip.show(&mut p, k as u8 + 1, payroll_tip_box(k as i32), [b"fire", b"", b""], 0);
            }
            None => tip.hide(&mut p),
        }
        Ok(())
    }

    /// strategic.md "The day-end screen" part 2: the merc-face question "Fire NICK?" of team slot k over `under`,
    /// as [up, NO pressed, OK pressed] for the question loop.
    pub fn payroll_fire_imgs(&self, w: &World, k: usize, under: &[u8]) -> Result<Vec<Vec<u8>>, i32> {
        let s = w.soldiers.get(k * 0x200..(k + 1) * 0x200).ok_or(1)?;
        let nick = slot_nick(s);
        let text = crate::dayend::fire_text(nick);
        [None, Some(0), Some(1)].into_iter().map(|pr| {
            let mut img = under.to_vec();
            Self::draw_face_question(&self.iso, &self.exe, &mut img, &text, s[0x179], nick, pr)?;
            Ok(img)
        }).collect()
    }

    /// strategic.md "The day-end screen" part 2, yes: Fire the merc (soldier k's slot byte, balloon at (11 + 37 k,
    /// 20)), then Close up the team (redraw 0); no hire-day salary, no CalcLedgerAll. Returns the host op 328
    /// fire_quote arguments (None: a native or a merc that leaves without a quote).
    pub fn payroll_fire(&mut self, w: &mut World, k: usize) -> Option<Vec<i32>> {
        let (slot, x, y) = crate::dayend::fire_args(&w.soldiers, k);
        self.host.said.clear();
        let mut cw = Ja1World::view(w);
        self.host.stash = std::mem::take(&mut w.stash_records);
        let h = Ja1Host { ctx: &mut self.host as *mut MapHost as *mut c_void, random: cb_random, call: cb_call,
                          host_ops: 0 };
        // SAFETY: cw points into `w`, which stays in place for the calls; the host outlives them.
        unsafe {
            ja1_fire_merc_core(&mut cw, &h, slot, x, y);
            ja1_team_compact(&mut cw, &h, 0);
        }
        cw.store(w);
        w.stash_records = std::mem::take(&mut self.host.stash);
        let said = std::mem::take(&mut self.host.said);
        said.into_iter().find(|(op, _)| *op == 328).map(|(_, a)| a)
    }

    /// strategic.md "The day-end screen" part 2, after the question: the payroll picture drawn again for the team
    /// as it is now (rows 1..142 over the payroll screen; the panel numbers stay) and the team slot marks
    /// (DrawTeamSlotMarks 0x4508C: element 0x67 at (13 + 37 k, 20) for k = 0.. while soldier k `inCurSector` ≠ 0,
    /// k < 8, a merc profile below 60).
    pub fn payroll_again(&self, w: &World) -> Result<Vec<u8>, i32> {
        let frames = crate::dayend::slot_marks(&w.soldiers);
        self.payday_img(w, None, &crate::dayloop::PayView { frames, end: true, ..Default::default() })
    }

    /// The message box (panel.md "Message box", strategic table) with `text` over `under`.
    pub fn msg_img(&self, under: &[u8], text: &[u8]) -> Result<Vec<u8>, i32> {
        let (c, d) = (crate::common(&self.iso)?, panel::panel_data(&self.iso, &self.exe)?);
        let mut img = under.to_vec();
        let mut table0 = c.remap[0];
        let mut p = Painter { d: &d, buf: &mut img, table0: &mut table0, table29: panel::table29(&c) };
        msgbox::draw(&mut p, text, &c.remap[msgbox::MAP_TABLE], msgbox::frame_colour(msgbox::MAP_TABLE));
        Ok(img)
    }

    /// Section 9 step 1, op 275 pd_ask: the merc-face question box ("Don't pay NICK?", or "Don't pay NICK's
    /// family?" for family 1; face and nickname of team slot k) over `under`, button `pressed` held.
    pub fn payday_ask_img(&self, w: &World, under: &[u8], k: usize, family: i32, pressed: Option<usize>)
                          -> Result<Vec<u8>, i32> {
        let mut img = under.to_vec();
        let s = w.soldiers.get(k * 0x200..(k + 1) * 0x200).ok_or(1)?;
        let nick = slot_nick(s);
        let text = crate::dayloop::pay_ask_text(&String::from_utf8_lossy(nick), family);
        Self::draw_face_question(&self.iso, &self.exe, &mut img, text.as_bytes(), s[0x179], nick, pressed)?;
        Ok(img)
    }

    fn draw_face_question(iso: &str, exe: &str, img: &mut [u8], text: &[u8], profile: u8, nick: &[u8],
                          pressed: Option<usize>) -> Result<(), i32> {
        let (c, d, inv) = (crate::common(iso)?, panel::panel_data(iso, exe)?, crate::invdraw::inv_data(iso, exe)?);
        let mut table0 = c.remap[0];
        let mut p = Painter { d: &d, buf: img, table0: &mut table0, table29: panel::table29(&c) };
        question::draw_face(&mut p, &inv, text, profile, nick, "PANEL_P.PCC");
        if let Some(k) = pressed {
            question::button(&mut p, k, true);
        }
        Ok(())
    }

    fn draw_question(iso: &str, exe: &str, img: &mut [u8], text: &[u8], pic: Option<&str>, pressed: Option<usize>,
                     item: Option<(u8, i32, i32, u8)>) -> Result<(), i32> {
        let (c, d, inv) = (crate::common(iso)?, panel::panel_data(iso, exe)?, crate::invdraw::inv_data(iso, exe)?);
        let mut table0 = c.remap[0];
        let mut p = Painter { d: &d, buf: img, table0: &mut table0, table29: panel::table29(&c) };
        question::draw(&mut p, &inv, text, pic, item);
        if let Some(k) = pressed {
            question::button(&mut p, k, true);
        }
        Ok(())
    }

    /// The random numbers of the map's clicks (the day loop keeps them equal to play's).
    pub fn rng(&mut self) -> &mut Rng {
        &mut self.host.rng
    }

    /// The map mode (0 team, 1 tappers, 2 guards).
    pub fn mode(&self) -> i32 {
        self.mode
    }

    /// Sets the map mode (the original's byte 0x93DA0): the strat_input_diff harness starts its tape on the mode
    /// the panel tab set before the loop.
    pub fn set_mode(&mut self, m: i32) {
        self.mode = m;
    }

    /// The message number of the last sector click (`ja1_map_click`; 0 = none). 1 (enemy hands) and 2 (not
    /// linked) show their message without the WaitLeftUp that every other click branch has.
    pub fn last_click_msg(&self) -> i32 {
        self.last_click_msg
    }

    /// The text of the message box that shows (None = no box).
    pub fn box_text(&self) -> Option<&str> {
        self.box_up.as_ref().map(|b| b.text.as_str())
    }

    /// The process warning flag 0x1C of the sector click (strategic.md "Sector click on the island map").
    pub fn warned(&self) -> u8 {
        self.warned
    }

    /// Shows message number `n` (0 = none; 16 = messages 16 and 17, one box after the other) in message boxes.
    /// strategic.md "Start the day": (x, y) is on the start button of the day loop's map (button 7 of the
    /// button table: 281..306 x 176..191) and the map answers it now (no message box, no other screen open).
    pub fn start_button_at(&self, (x, y): (i32, i32)) -> bool {
        self.day && self.box_up.is_none() && self.boxes.is_empty() && self.assign.is_none() && self.guard.is_none()
            && !self.finance && (281..=306).contains(&x) && (176..=191).contains(&y)
    }

    /// The map panel's start button (field 7 of the button table) pressed at `(x, y)`: the original's
    /// ButtonAt(7,1) runs ja1_map_start (button 1). -1 when the point is not the button, else the message number
    /// (0 = the day starts).
    pub fn start_pressed(&mut self, w: &mut World, (x, y): (i32, i32)) -> Result<i32, i32> {
        if !self.start_button_at((x, y)) {
            return Ok(-1);
        }
        let cw = Ja1World::view(w);
        // SAFETY: as in `click`.
        let mut hinted = w.hinted;
        let n = unsafe { ja1_map_start(&cw, 1, &mut hinted) };
        w.hinted = hinted;
        Ok(n)
    }

    pub fn set_msg(&mut self, _w: &World, n: i32) -> Result<(), i32> {
        self.message_number(n)
    }

    /// Shows `text` in a message box.
    pub fn set_text(&mut self, _w: &World, text: String) -> Result<(), i32> {
        self.message_box(text)
    }

    fn message_number(&mut self, n: i32) -> Result<(), i32> {
        if let Some(m) = message(n) {
            self.message_box(m.to_string())?;
        }
        if n == 16 {
            self.message_box(MESSAGES[16].to_string())?;
        }
        Ok(())
    }

    /// panel.md "Message box": `text` waits for its turn, then shows until a key, a new mouse press or 25 s.
    pub fn message_box(&mut self, text: String) -> Result<(), i32> {
        self.message_box_ms(text, 25_000)
    }

    /// A message box with the time limit `ms`.
    pub fn message_box_ms(&mut self, text: String, ms: u128) -> Result<(), i32> {
        self.boxes.push_back((text, ms));
        if self.box_up.is_none() {
            self.next_box()?;
        }
        Ok(())
    }

    /// PlayInvExtra? 0x25AD8, the wait after the box: while the rest of the sound plays the screen reads no key and
    /// draws no hover. A button that is down ends it (`end_sound_hold`). A remark with no box has no such wait here:
    /// the press that made it is still down at the first poll.
    pub fn sound_held(&self) -> bool {
        self.box_up.is_none() && self.sound_until.is_some()
    }

    /// True once when the wait above ended: at a button down, or at the end of the sound.
    pub fn sound_hold_over(&mut self, button_down: bool) -> bool {
        let over = self.sound_held() && (button_down || self.sound_until.is_some_and(|t| std::time::Instant::now() >= t));
        if over {
            self.sound_until = None;
            eprintln!("assign sound wait end {}", if button_down { "button" } else { "sound" });
        }
        over
    }

    /// A message box shows.
    pub fn box_shown(&self) -> bool {
        self.box_up.is_some()
    }

    /// panel.md "Exit to DOS": the island map or the assignment screen with no box, question, menu or held button
    /// (MapKeyHandler? 0x42712, AssignScreen? 0x48A37 read Alt-X there); not the guard screen.
    /// Alt-X on the island map (panel.md "Exit to DOS"): true when the Exit to DOS question opens now. In the wage
    /// question's yes row KeyOrClick 0x67E44 reads the key and ends the row (the map loop never sees it); in its
    /// release wait the key stays in the queue and acts after the release (`take_kept` gives `KEPT_ALT_X`).
    pub fn alt_x(&mut self, w: &World) -> Result<bool, i32> {
        if self.wage_go && self.wage_go_up != 0 {
            self.kept.push(KEPT_ALT_X);
            return Ok(false);
        }
        if self.wage_go {
            self.end_wage_go(w)?;
            return Ok(false);
        }
        Ok(self.exit_key_ok())
    }

    pub fn exit_key_ok(&self) -> bool {
        self.guard.is_none() && self.box_up.is_none() && self.ask_down.is_none() && !self.exit_down
            && self.duty_menu.is_none() && self.discard_ask.is_none() && self.fire_ask.is_none() && !self.assign_ask
            && !self.finance && !self.plants_down && self.box_down.is_none() && !self.back_down && !self.wage_go
            && self.wage_ask.is_none()
    }

    /// A mouse button went down while the box shows (its release ends the box).
    pub fn box_pressed(&self) -> bool {
        self.box_up.as_ref().is_some_and(|b| b.pressed != 0)
    }

    fn next_box(&mut self) -> Result<(), i32> {
        if let Some((t, ms)) = self.boxes.pop_front() {
            eprintln!("map box {}", t);
            self.box_up = Some(MsgBox { saved: Vec::new(), text: t, since: std::time::Instant::now(), pressed: 0, ms, again: None });
            self.draw_box()?;
        }
        Ok(())
    }

    /// Saves the rectangle under the box and draws the box of `box_up` on the picture.
    fn draw_box(&mut self) -> Result<(), i32> {
        let Some(b) = self.box_up.as_mut() else { return Ok(()) };
        if let Some((k, name, text)) = &b.again {
            b.saved = self.img.clone();
            let (c, d) = (crate::common(&self.iso)?, panel::panel_data(&self.iso, &self.exe)?);
            let inv = crate::invdraw::inv_data(&self.iso, &self.exe)?;
            let mut table0 = c.remap[0];
            let mut p = Painter { d: &d, buf: &mut self.img, table0: &mut table0, table29: panel::table29(&c) };
            question::draw_face_text(&mut p, &inv, text, *k, name, "PANEL_P.PCC");
            return Ok(());
        }
        let (x, y, bw, bh) = msgbox::SAVED;
        b.saved.clear();
        for yy in y..y + bh {
            let at = (yy * 320 + x) as usize;
            b.saved.extend_from_slice(&self.img[at..at + bw as usize]);
        }
        let (c, d) = (crate::common(&self.iso)?, panel::panel_data(&self.iso, &self.exe)?);
        let mut table0 = c.remap[0];
        let mut p = Painter { d: &d, buf: &mut self.img, table0: &mut table0, table29: panel::table29(&c) };
        msgbox::draw(&mut p, b.text.as_bytes(), &c.remap[msgbox::MAP_TABLE], msgbox::frame_colour(msgbox::MAP_TABLE));
        Ok(())
    }

    /// The box goes: the saved rectangle comes back, then the next box (if any) shows.
    pub fn dismiss(&mut self) -> Result<(), i32> {
        if let (true, Some((t0, voice))) = (self.box_up.is_some(), self.remark_box) {
            self.remark_box = None;
            crate::sound::screen(|s| s.speech_resume());
            let left = voice.saturating_sub(t0.elapsed().min(crate::sound::STREAM_BUFFER_TIME));
            eprintln!("assign sound tail {}", left.as_millis());
            self.sound_until = Some(std::time::Instant::now() + left);
        }
        if let Some(b) = self.box_up.take() {
            if b.again.is_some() {
                self.img = b.saved;
                return self.next_box();
            }
            let (x, y, bw, bh) = msgbox::SAVED;
            for (k, yy) in (y..y + bh).enumerate() {
                let at = (yy * 320 + x) as usize;
                self.img[at..at + bw as usize].copy_from_slice(&b.saved[k * bw as usize..(k + 1) * bw as usize]);
            }
        }
        self.next_box()
    }

    /// Input while a box shows (`true` = used up by the box): a key ends it; a mouse button pressed while it shows
    /// ends it when it comes up; 25 000 ms end it too (`Input::Time`).
    pub fn box_input(&mut self, i: BoxInput) -> Result<bool, i32> {
        let Some(b) = self.box_up.as_mut() else { return Ok(false) };
        match i {
            BoxInput::Key => {
                if b.pressed == 0 {
                    self.dismiss()?;
                }
            }
            BoxInput::Down(n) => {
                if b.pressed == 0 {
                    b.pressed = n;
                }
            }
            BoxInput::Up(n) => {
                if b.pressed == n && n != 0 {
                    self.dismiss()?;
                }
            }
            BoxInput::Time => {
                if box_timed_out(b.again.is_none(), b.pressed, b.since.elapsed().as_millis(), b.ms) {
                    self.dismiss()?;
                }
            }
        }
        Ok(true)
    }

    /// The picture as shown with `text` on the status line instead of the message.
    pub fn with_status(&mut self, w: &World, text: &'static str) -> Result<Vec<u8>, i32> {
        let m = self.msg;
        self.msg = Some(text);
        self.redraw(w)?;
        let img = self.img.clone();
        self.msg = m;
        self.redraw(w)?;
        Ok(img)
    }

    /// The message on the bottom lines, font 1, wrapped to 320 pixels.
    fn status(&mut self, m: &str) -> Result<(), i32> {
        let (c, d) = (crate::common(&self.iso)?, panel::panel_data(&self.iso, &self.exe)?);
        let mut table0 = c.remap[0];
        let widths = d.fonts[1].widths;
        let mut lines: Vec<String> = vec![String::new()];
        let mut width = 0;
        for word in m.split(' ') {
            let ww: i32 = word.bytes().chain(std::iter::once(b' ')).map(|b| widths.get(b as usize - 0x20).copied().unwrap_or(0) as i32).sum();
            if width + ww > 316 && width > 0 {
                lines.push(String::new());
                width = 0;
            }
            let l = lines.last_mut().unwrap();
            if !l.is_empty() {
                l.push(' ');
            }
            l.push_str(word);
            width += ww;
        }
        let mut p = Painter { d: &d, buf: &mut self.img, table0: &mut table0, table29: panel::table29(&c) };
        for (i, l) in lines.iter().enumerate() {
            p.text(l.as_bytes(), 160, 170 + 9 * i as i32, true, 0xFF, 0, 1);
        }
        Ok(())
    }
}

fn message(n: i32) -> Option<&'static str> {
    if (1..=20).contains(&n) {
        Some(MESSAGES[n as usize - 1])
    } else {
        None
    }
}

/// The water rotation of the island map on a 768-byte palette: PaletteRotate? 0x528AC (0xD7, 0xDA), then
/// (0xDB, 0xDE) (strategic.md "Water on the island map"; palrotate_diff).
pub fn rotate_water(pal: &mut [u8]) {
    for (a, b) in guard::ROTATE {
        pal[a * 3..b * 3 + 3].rotate_left(3);
    }
}

/// The assignment screen shows the held item as the mouse pointer: the screen is open (no guard screen over it), no
/// question box (day ask, Fire, discard) shows, and an item is on the cursor.
pub fn item_pointer_of(guard: bool, assign: bool, asking: bool, held: u8) -> bool {
    !guard && assign && !asking && held != 0
}

#[cfg(test)]
mod tests {
    /// Ledger rows c2aea50:ja1front:12549/12554: the held item is the pointer only on the open assignment screen with
    /// no box and no guard screen over it.
    #[test]
    fn the_held_item_is_the_pointer_on_the_assignment_screen() {
        assert!(super::item_pointer_of(false, true, false, 5));
        assert!(!super::item_pointer_of(true, true, false, 5));
        assert!(!super::item_pointer_of(false, false, false, 5));
        assert!(!super::item_pointer_of(false, true, true, 5));
        assert!(!super::item_pointer_of(false, true, false, 0));
    }
    use super::*;

    /// Ledger row c2aea50:ja1front:12905: a wage row frame waits its frame time; the first one comes at once.
    #[test]
    fn a_wage_frame_waits_its_time() {
        let now = std::time::Instant::now();
        assert!(frame_due(None, 1000));
        assert!(!frame_due(Some(now), 1000));
        assert!(!frame_due(Some(now), 500));
        assert!(frame_due(Some(now - std::time::Duration::from_millis(1200)), 1000));
    }

    /// Ledger row c2aea50:ja1front:13348: the 25 000 ms limit ends a box only while no mouse button is held.
    #[test]
    fn the_box_time_ends_only_with_no_button_held() {
        assert!(box_timed_out(true, 0, 25_000, 25_000));
        assert!(!box_timed_out(true, 1, 30_000, 25_000));
        assert!(!box_timed_out(true, 0, 24_999, 25_000));
        assert!(!box_timed_out(false, 0, 30_000, 25_000));
    }

    /// Ledger row c2aea50:ja1front:12829: with no map press just ended, the pass after a hold does nothing (no
    /// mouse read, no click).
    #[test]
    fn no_hold_end_no_after_hold_click() {
        let mut w = pattern_world();
        let mut m = MapUi::new("", "", 1);
        m.hold_left = true;
        assert_eq!(m.after_hold(&mut w), Ok(()));
        assert!(m.hold_left);
    }

    /// Ledger rows c2aea50:ja1front:13183/13184/13188: the "fire" tooltip of face k sits over that face (13 + 37 k,
    /// 20, 41 + 37 k, 45; PayrollScreen, strategic.md "The day-end screen" part 2).
    #[test]
    fn the_fire_tooltip_sits_over_each_payroll_face() {
        assert_eq!(payroll_tip_box(0), (13, 20, 41, 45));
        assert_eq!(payroll_tip_box(1), (50, 20, 78, 45));
        assert_eq!(payroll_tip_box(7), (272, 20, 300, 45));
    }

    /// Ledger row c2aea50:ja1front:13237: the pay question names the merc by the nickname up to its first 0 byte.
    #[test]
    fn the_nickname_ends_at_the_first_zero() {
        let mut s = vec![0u8; 0x200];
        s[0x16E..0x16E + 9].copy_from_slice(b"IVAN\0XYZW");
        assert_eq!(slot_nick(&s), b"IVAN");
        s[0x16E..0x16E + 9].copy_from_slice(b"ABCDEFGHI");
        assert_eq!(slot_nick(&s), b"ABCDEFGHI");
    }

    /// Ledger row 0b4d529:ja1front:13088 (guard screen, Start button 0x67CD8(7, 1): the pressed face stays until the
    /// LEFT button goes up): a right release while Start is held does not run the exit test.
    #[test]
    fn only_the_left_release_ends_the_guard_start_press() {
        let Some((mut m, mut w)) = game() else { return };
        w.cur_sector = 38;
        assert!(m.guard_open(&mut w).unwrap());
        m.mouse = (290, 180);
        m.guard_click(&mut w, 1).unwrap();
        assert!(m.start_down);
        m.release_in(&mut w, 2).unwrap();
        assert!(m.start_down, "a right release keeps the Start press");
        m.release_in(&mut w, 1).unwrap();
        assert!(!m.start_down);
    }

    /// Ledger row 0b4d529:ja1front:13171: assign_open tells the window that the assignment screen is open (the
    /// slide-in pointer 201 goes back to the arrow then, main.rs slide_shape_ends).
    #[test]
    fn assign_open_follows_the_assignment_screen() {
        let mut m = MapUi::new("", "", 1);
        assert!(!m.assign_open());
        m.assign = Some((vec![], vec![]));
        assert!(m.assign_open());
    }

    /// Ledger row 0b4d529:ja1front:13550 (op 275 pd_ask): the "Don't pay NICK?" box of team slot k shows the face and
    /// nickname of record k, for every slot (not only slot 0).
    #[test]
    fn the_pay_question_shows_the_merc_of_its_slot() {
        let Some((m, mut w)) = game() else { return };
        let under = vec![0u8; 320 * 200];
        let r = 0x200;
        w.soldiers[0x16E..0x16E + 5].copy_from_slice(b"IVAN\0");
        w.soldiers[0x179] = 1;
        w.soldiers[2 * r + 0x16E..2 * r + 0x16E + 5].copy_from_slice(b"FIDE\0");
        w.soldiers[2 * r + 0x179] = 5;
        let slot2 = m.payday_ask_img(&w, &under, 2, 0, None).unwrap();
        assert_ne!(slot2, m.payday_ask_img(&w, &under, 0, 0, None).unwrap());
        let rec2 = w.soldiers[2 * r..3 * r].to_vec();
        w.soldiers[..r].copy_from_slice(&rec2);
        assert_eq!(slot2, m.payday_ask_img(&w, &under, 0, 0, None).unwrap());
    }

    fn game() -> Option<(MapUi, World)> {
        let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
        let (iso, exe) = (root.join("rom/JA1.gog"), root.join("build/game113b/JA.EXE"));
        if !iso.exists() || !exe.exists() {
            return None;
        }
        let (iso, exe) = (iso.to_str().unwrap(), exe.to_str().unwrap());
        Some((MapUi::new(iso, exe, 1), World::new_game(iso, exe, 0, 0).unwrap()))
    }

    fn pattern_world() -> World {
        let (exe, jag, save) = crate::world::tests::files(&[]);
        World::from_files(&exe, &jag, &save).unwrap()
    }

    /// Ledger rows d057aca:ja1front:12845/12846/12847/12850 (KeyOrClick 0x67E44 in the wage question's row): a button
    /// that was down at the row start counts only after its own release; that release clears only its flag and acts
    /// on nothing else. A release of the other button goes on as usual.
    #[test]
    fn a_held_button_of_the_wage_row_counts_after_its_own_release() {
        let mut w = pattern_world();
        let mut m = MapUi::new("", "", 1);
        (m.wage_go, m.wage_go_up, m.wage_held) = (true, 0, 3);
        crate::invextra::set_latch(3);
        m.release_in(&mut w, 1).unwrap();
        assert_eq!((m.wage_held, crate::invextra::latch()), (2, 2));
        // the right button was not down at the start in this case: its release reads no flag
        m.wage_held = 1;
        crate::invextra::set_latch(3);
        m.release_in(&mut w, 2).unwrap();
        assert_eq!((m.wage_held, crate::invextra::latch()), (1, 3));
    }

    /// Ledger row d057aca:ja1front:12920: StrategicScreen hides the pointer for a flip (HidePointer? 0x43741): the
    /// queued picture carries its pointer (-1 = none) to the window.
    #[test]
    fn a_queued_flip_picture_carries_its_pointer() {
        let mut m = MapUi::new("", "", 1);
        assert_eq!(m.anim_pointer(), None);
        m.step_ptr = -1;
        m.push_step(vec![0; 64], 60, None);
        m.step_ptr = 0;
        m.push_step(vec![0; 64], 0, None);
        assert_eq!(m.anim_pointer(), Some(-1));
    }

    /// Ledger rows d057aca:ja1front:12984/13350 (panel.md "The travel animation"; "Exit to DOS"): the wage question's
    /// yes row steps one frame per frame time and ends after its last frame; no step runs while a button is held on
    /// the box. Alt-X in the row ends it; in the row's release wait the key waits for the release.
    #[test]
    fn the_wage_yes_row_plays_to_its_end_and_alt_x_ends_it() {
        let Some((mut m, w)) = game() else { return };
        let _g = crate::sound::SCREEN_TEST_LOCK.lock().unwrap_or_else(|e| e.into_inner());
        let a = crate::sanim::load(&m.iso, &m.exe, WAGE_ARC, WAGE_ROWS.1, None).unwrap();
        let n = a.shown();
        assert!(n > 1);
        let past = || Some(std::time::Instant::now() - std::time::Duration::from_secs(10));
        (m.wage_anim, m.wage_go, m.wage_at) = (Some((a, 1)), true, None);
        m.ask_down = Some(1);
        assert_eq!(m.anim_step(&w), Ok(false), "a step under a held box button");
        m.ask_down = None;
        assert_eq!(m.anim_step(&w), Ok(true));
        assert_eq!(m.anim_step(&w), Ok(false), "a step before its frame time");
        let mut steps = 1;
        while m.wage_go && steps < 1000 {
            m.wage_at = past();
            assert_eq!(m.anim_step(&w), Ok(true));
            steps += 1;
        }
        assert_eq!(steps, n + 1, "the row did not end after its last frame");
        // Alt-X in the row: the row ends, no question; in the release wait the key is kept.
        let a = crate::sanim::load(&m.iso, &m.exe, WAGE_ARC, WAGE_ROWS.1, None).unwrap();
        (m.wage_anim, m.wage_go, m.wage_go_up) = (Some((a, 1)), true, 0);
        assert_eq!(m.alt_x(&w), Ok(false));
        assert!(!m.wage_go && m.kept.is_empty());
        (m.wage_go, m.wage_go_up) = (true, 1);
        assert_eq!(m.alt_x(&w), Ok(false));
        assert_eq!(m.kept, [KEPT_ALT_X]);
    }

    /// strategic.md "Water on the island map" (StrategicScreen 0x432F4, timer 0x22A3FC): the water rotates only
    /// when more than 500 ms passed since the last rotation; no wait state rotates it, and a rotation that came due
    /// in a wait follows at once.
    #[test]
    fn the_water_waits_500_ms_and_for_no_box() {
        let mut m = MapUi::new("", "", 1);
        m.pal = std::array::from_fn(|i| i as u8);
        let p0 = m.pal;
        assert!(!m.cycle(1000), "the first round starts the timer");
        assert!(!m.cycle(1500), "500 ms is not more than 500 ms");
        assert_eq!(m.pal, p0);
        assert!(m.cycle(1501));
        assert_ne!(m.pal, p0);
        // a map press waits for its release: no rotation, then at once
        m.map_hold = 1;
        assert!(!m.cycle(2600));
        m.map_hold = 0;
        assert!(m.cycle(2601));
        assert_eq!(m.cycles, 2);
    }

    /// Box 3 point 5b (#31, live DOSBox-X 2026-11-22, timer 0x22A3FC read every 2 ms): after a map press the status
    /// box stops the water and its closing press rotates at once; the finance flip restarts the timer on each
    /// picture, the page stops the water, and the first rotation comes about 550 ms after the closing flip; the
    /// wage box stops the water and its closing press rotates at once; the wage question's picture row restarts
    /// the timer every 220 ms, so a full wait follows its answer, also when a flip follows at once.
    #[test]
    fn the_water_after_a_map_press_a_wage_box_and_a_flip() {
        let mut m = MapUi::new("", "", 1);
        assert!(!m.cycle(0));
        // status box after the release: no rotation, the closing press rotates at once
        m.msg = Some("This sector is in enemy hands!");
        assert!(!m.cycle(2000));
        m.msg = None;
        assert!(m.cycle(2001));
        // finance flip at once (300 ms later): no rotation in the flip or the page; full wait after the back flip
        m.anim.push_back(Vec::new());
        assert!(!m.cycle(2300));
        m.anim.clear();
        m.finance = true;
        assert!(!m.cycle(4000));
        m.finance = false;
        m.anim.push_back(Vec::new());
        assert!(!m.cycle(4100));
        m.anim.clear();
        assert!(!m.cycle(4400), "the round after the flip starts the timer");
        assert!(!m.cycle(4900));
        assert!(m.cycle(4901));
        // wage box: no rotation while it is open, the closing press rotates at once
        m.wage_edit = Some(35);
        assert!(!m.cycle(6000));
        m.wage_edit = None;
        assert!(m.cycle(6001));
        // wage question with its picture row, answered, then the finance flip at once: full wait after the flip
        m.wage_ask = Some(35);
        assert!(!m.cycle(8000));
        m.wage_ask = None;
        assert!(!m.cycle(8400), "the answer starts the timer");
        m.anim.push_back(Vec::new());
        assert!(!m.cycle(8800));
        m.anim.clear();
        assert!(!m.cycle(9000));
        assert!(!m.cycle(9500));
        assert!(m.cycle(9501));
        assert_eq!(m.cycles, 4);
    }

    /// QuestionBox? 0x279B1 / 0x27AF7 for the two questions of AssignScreen? (0x48B58, 0x49CCA): the place goes to
    /// MouseSavePos2?, a mouse outside the box moves to (237, 102), and the answer puts the mouse back.
    #[test]
    fn an_assignment_question_moves_the_mouse_and_puts_it_back() {
        let mut m = MapUi::new("", "", 1);
        m.mouse = (10, 150);
        m.question_open();
        assert_eq!(crate::pointer::mouse_saved(), (10, 150));
        assert_eq!(m.warp, Some((237, 102)));
        m.warp = None;
        m.mouse = (200, 60);
        m.question_back();
        assert_eq!(m.warp, Some((10, 150)));
        // a mouse inside the box stays
        m.warp = None;
        m.mouse = (100, 60);
        m.question_open();
        assert_eq!((crate::pointer::mouse_saved(), m.warp), ((100, 60), None));
    }

    /// The Fire question (strategic.md "Firing a merc" step 1) is a YesNoPrompt? 0x28D88 box: with Subtitles on, a
    /// mouse outside the box moves to the box (question::prompt_warp); with Subtitles off the mouse stays.
    #[test]
    fn the_fire_question_moves_the_mouse_only_with_subtitles() {
        let mut w = pattern_world();
        for (sub, want) in [(1u8, question::prompt_warp(true, (10, 150))), (0, None)] {
            let mut m = MapUi::new("", "", 1);
            m.mouse = (10, 150);
            crate::ctlpanel::set(&mut w, 5, sub);
            m.fire_question_open(&w, 2);
            assert_eq!((m.fire_ask, m.warp), (Some(2), want));
        }
        assert!(question::prompt_warp(true, (10, 150)).is_some());
    }

    /// AssignScreen? 0x48B58: a pressed button (question NO / OK, Exit, a panel button) waits for its release in
    /// its own loop, so no second press (the other mouse button) counts until then.
    #[test]
    fn a_pressed_assignment_button_takes_no_second_press() {
        let mut m = MapUi::new("", "", 1);
        assert!(m.press_idle());
        m.ask_down = Some(0);
        assert!(!m.press_idle(), "a question button is down");
        m.ask_down = None;
        m.exit_down = true;
        assert!(!m.press_idle(), "Exit is down");
        m.exit_down = false;
        m.panel_down = Some(1);
        assert!(!m.press_idle(), "a panel button is down");
    }

    /// strategic.md "Firing a merc" steps 1 and 2: the question names the merc of the pressed team slot ("Fire "
    /// JA.EXE 0x99EAF + nickname + "?") and shows that merc's face (+0x179).
    #[test]
    fn the_fire_question_names_the_merc_of_its_slot() {
        let mut s = vec![0u8; 8 * 0x200];
        s[0x16E..0x16E + 4].copy_from_slice(b"IVAN");
        s[0x179] = 3;
        s[2 * 0x200 + 0x16E..2 * 0x200 + 0x16E + 5].copy_from_slice(b"FIDEL");
        s[2 * 0x200 + 0x179] = 17;
        assert_eq!(fire_question_parts(&s, 2), (b"Fire FIDEL?".to_vec(), 17, &b"FIDEL"[..]));
        assert_eq!(fire_question_parts(&s, 0), (b"Fire IVAN?".to_vec(), 3, &b"IVAN"[..]));
    }

    /// StrategicScreen 0x432F4 after a map press: WaitLeftUp, then WaitRightUp, read no press. Only a right press
    /// makes the second wait longer; a left press counts for the next round only after WaitLeftUp ended.
    #[test]
    fn a_press_in_the_map_hold_only_adds_the_right_button() {
        let mut w = pattern_world();
        let mut m = MapUi::new("", "", 1);
        m.map_hold = 1;
        m.click(&mut w, 1).unwrap();
        assert_eq!((m.map_hold, m.hold_left), (1, false), "a left press inside WaitLeftUp");
        m.click(&mut w, 2).unwrap();
        assert_eq!((m.map_hold, m.hold_left), (3, false));
        m.map_hold = 2;
        m.click(&mut w, 1).unwrap();
        assert_eq!((m.map_hold, m.hold_left), (2, true), "a left press inside WaitRightUp stays down");
    }

    /// strategic.md "Selecting a merc" (AssignScreen? Leave): Leave with a selected merc takes away a Doctor duty (3)
    /// or a Repair duty (5) that the merc cannot do now; every other duty stays.
    #[test]
    fn leave_checks_the_duty_of_the_selected_merc() {
        for (duty, after) in [(3u8, 0u8), (5, 0), (1, 1), (4, 4), (2, 2)] {
            let mut w = pattern_world();
            let mut m = MapUi::new("", "", 1);
            let s = &mut w.soldiers[2 * 0x200..3 * 0x200];
            s[0] = 0; // no medical kit and no tool kit in the hand: no Doctor, no Repair
            s[0x17E] = duty;
            assert!(!crate::assign::duty_flags(s)[3] && !crate::assign::duty_flags(s)[5]);
            m.selected = Some(2);
            w.team_on_duty = 0;
            let _ = m.leave_assign(&mut w); // (the picture needs the game files)
            assert_eq!(w.soldiers[2 * 0x200 + 0x17E], after, "duty {}", duty);
            assert!(m.assign_ask, "nobody on duty: the question");
        }
    }

    /// KeyOrClick 0x67E44 of the wage row (panel.md "The travel animation"): a key is kept only inside the release
    /// wait; with no button down at the start the first key ends the row and is used up.
    #[test]
    fn a_key_ends_the_wage_row_outside_the_release_wait() {
        let mut w = pattern_world();
        let mut m = MapUi::new("", "", 1);
        (m.wage_go, m.wage_go_up) = (true, 0);
        let _ = m.key(&mut w, b'A');
        assert!(!m.wage_go && m.kept.is_empty());
        (m.wage_go, m.wage_go_up) = (true, 1);
        assert!(matches!(m.key(&mut w, b'A'), Ok(Key::Nothing)));
        assert!(m.wage_go && m.kept == vec![b'A']);
    }

    /// ConfirmDialog? 0x277F6 sets no start flag, so KeyOrClick 0x67E44 of the wage row starts with the button flags
    /// 0x284F1A / 0x284F1C as they are: a button that is down with its flag is no new press; a button that is down
    /// without its flag starts the release wait (right first).
    #[test]
    fn the_wage_row_keeps_only_the_flags_of_buttons_that_are_down() {
        let Some((mut m, mut w)) = game() else { return };
        for (latch, level, held, up) in [(1u8, 2i32, 0u8, 2), (1, 1, 1, 0), (3, 1, 1, 0), (0, 3, 0, 2), (2, 1, 0, 1)] {
            crate::invextra::set_latch(latch);
            m.buttons = level;
            m.wage_ask = Some(5);
            m.answer_wage(&mut w, true).unwrap();
            assert_eq!((m.wage_held, crate::invextra::latch(), m.wage_go_up), (held, held, up), "latch {} level {}", latch, level);
            assert!(m.wage_go);
            m.end_wage_go(&w).unwrap();
        }
    }

    /// strategic.md "Firing a merc": the fired merc's quote balloon stands at the duty name box of its team slot,
    /// x = 11 + 37 k, y = 0x14 (the call of 0x5097C in AssignScreen?).
    #[test]
    fn the_fired_merc_speaks_at_its_duty_box() {
        for k in [0usize, 3] {
            let Some((mut m, mut w)) = game() else { return };
            m.show(&mut w, (0, 0)).unwrap();
            let s = &mut w.soldiers[k * 0x200..(k + 1) * 0x200];
            (s[0x182], s[0x183], s[0x17A]) = (1, 50, k as u8);
            m.host.said.clear();
            m.answer_fire(&mut w, k, true).unwrap();
            let said: Vec<_> = m.host.said.iter().filter(|(op, _)| *op == 328).collect();
            assert_eq!(said.len(), 1);
            assert_eq!(said[0].1[1..3], [11 + 37 * k as i32, 0x14]);
        }
    }

    /// strategic.md "The guide's advice" 13: the offer is the island map with the merc-face question box over it.
    #[test]
    fn the_guide_offer_is_a_box_over_the_island_map() {
        let Some((mut m, mut w)) = game() else { return };
        m.show(&mut w, (0, 0)).unwrap();
        let img = m.guide_offer_img(&w, 60, None).unwrap();
        assert_eq!(img.len(), 64000);
        let diff: Vec<usize> = (0..64000).filter(|&i| img[i] != m.img[i]).collect();
        assert!(diff.len() > 2000, "the box is on the picture ({} pixels)", diff.len());
        // the box stays inside the question box frame (x 0x34..=0x10B, y 0x25..=0x74 holds its buttons)
        assert!(diff.iter().all(|&i| (40..=280).contains(&(i % 320)) && (20..=140).contains(&(i / 320))),
                "x {:?} y {:?}", diff.iter().map(|i| i % 320).min().zip(diff.iter().map(|i| i % 320).max()),
                diff.iter().map(|i| i / 320).min().zip(diff.iter().map(|i| i / 320).max()));
        let pressed = m.guide_offer_img(&w, 60, Some(0)).unwrap();
        assert_ne!(pressed, img, "a held button shows");
    }
}

#[cfg(test)]
mod gap_minor_tests {
    use super::*;

    fn world() -> World {
        let (exe, jag, save) = crate::world::tests::files(&[]);
        World::from_files(&exe, &jag, &save).unwrap()
    }

    fn game() -> Option<(MapUi, World)> {
        let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
        let (iso, exe) = (root.join("rom/JA1.gog"), root.join("build/game113b/JA.EXE"));
        if !iso.exists() || !exe.exists() {
            return None;
        }
        let (iso, exe) = (iso.to_str().unwrap(), exe.to_str().unwrap());
        Some((MapUi::new(iso, exe, 1), World::new_game(iso, exe, 0, 0).unwrap()))
    }

    /// Ledger rows ja1front:12015/12010: the assignment screen hover does nothing while its question waits or the
    /// exit button is down (no file is read: with no files the hover would fail).
    #[test]
    fn no_assign_hover_under_a_question_or_the_exit_press() {
        let _g = crate::sound::SCREEN_TEST_LOCK.lock().unwrap_or_else(|e| e.into_inner());
        let w = world();
        let mut m = MapUi::new("", "", 1);
        m.assign = Some((vec![0; 64000], vec![0; 64000]));
        m.assign_ask = true;
        assert_eq!(m.assign_hover(&w), Ok(false));
        m.assign_ask = false;
        m.exit_down = true;
        assert_eq!(m.assign_hover(&w), Ok(false));
    }

    /// Ledger row c2aea50:ja1front:12586: with no held item and no change by the hover, the assignment screen pass
    /// draws nothing.
    #[test]
    fn an_idle_assign_pass_draws_nothing() {
        let _g = crate::sound::SCREEN_TEST_LOCK.lock().unwrap_or_else(|e| e.into_inner());
        let mut w = world();
        let mut m = MapUi::new("", "", 1);
        m.assign = Some((vec![0; 64000], vec![0; 64000]));
        m.assign_ask = true;
        w.held[0] = 0;
        assert_eq!(m.track(&mut w, (100, 100)), Ok(()));
    }

    /// Ledger rows ja1front:12061/12067: under the wage field edit the map tracks no sector; a sector already in
    /// the panel's field 15 is not drawn again.
    #[test]
    fn map_track_under_wage_edit_and_same_sector() {
        let _g = crate::sound::SCREEN_TEST_LOCK.lock().unwrap_or_else(|e| e.into_inner());
        let mut w = world();
        let (x, y, s) = (0..140)
            .flat_map(|y| (0..320).map(move |x| (x, y)))
            .map(|(x, y)| (x, y, island::sector_at(x, y)))
            .find(|t| (0..60).contains(&t.2))
            .unwrap();
        let mut m = MapUi::new("", "", 1);
        m.wage_edit = Some(0);
        assert_eq!(m.track(&mut w, (x, y)), Ok(()));
        assert_eq!((m.mouse_num, m.cur), (None, -1));
        let mut m = MapUi::new("", "", 1);
        (m.mouse_num, m.cur) = (Some(s + 1), s);
        assert_eq!(m.track(&mut w, (x, y)), Ok(()));
    }

    /// Ledger rows ja1front:12255/12261: a key does nothing while a question button (assignment screen) or the back
    /// button (map) is down.
    #[test]
    fn keys_wait_for_a_held_button() {
        let _g = crate::sound::SCREEN_TEST_LOCK.lock().unwrap_or_else(|e| e.into_inner());
        let mut w = world();
        let mut m = MapUi::new("", "", 1);
        m.assign = Some((vec![0; 64000], vec![0; 64000]));
        m.ask_down = Some(0);
        assert!(matches!(m.key(&mut w, 13), Ok(Key::Nothing)));
        let mut m = MapUi::new("", "", 1);
        m.back_down = true;
        assert!(matches!(m.key(&mut w, b'T'), Ok(Key::Nothing)));
        assert_eq!(m.mode, MapUi::new("", "", 1).mode);
    }

    /// Ledger row ja1front:12284: each key L counts once.
    #[test]
    fn say_again_counts_its_key() {
        let _g = crate::sound::SCREEN_TEST_LOCK.lock().unwrap_or_else(|e| e.into_inner());
        let w = world();
        let mut m = MapUi::new("", "", 1);
        let _ = m.say_again(&w);
        let _ = m.say_again(&w);
        assert_eq!(m.again_keys, 2);
    }

    /// Ledger row ja1front:12223: the sound of the first queued picture starts at once (it is taken from the queue);
    /// the sound of the next picture waits for its picture.
    #[test]
    fn the_first_queued_picture_starts_its_sound() {
        let _g = crate::sound::SCREEN_TEST_LOCK.lock().unwrap_or_else(|e| e.into_inner());
        let mut m = MapUi::new("", "", 1);
        m.push_step(vec![0; 64], 60, Some((4, 0x2B11, 0, 1)));
        assert_eq!(m.anim_sfx[0], None);
        m.push_step(vec![0; 64], 60, Some((4, 0x2B11, 0, 1)));
        assert!(m.anim_sfx[1].is_some());
    }

    /// Ledger rows ja1front:12303/12304/12306 + 12382/12383: AssignScreen keeps the last merc index (count - 1)
    /// for the face buttons; with 3 mercs the next button after the third merc goes back to the first one.
    #[test]
    fn assign_last_merc_and_the_next_button() {
        let _g = crate::sound::SCREEN_TEST_LOCK.lock().unwrap_or_else(|e| e.into_inner());
        let Some((mut m, mut w)) = game() else { return };
        for k in 0..8 {
            w.soldiers[k * 0x200 + 0x182] = 0;
        }
        m.assign_last = 7;
        m.open_assign(&mut w).unwrap();
        assert_eq!(m.assign_last, 7);
        for k in 0..3 {
            w.soldiers[k * 0x200 + 0x182] = 1;
        }
        m.open_assign(&mut w).unwrap();
        assert_eq!(m.assign_last, 2);
        m.assign_last = 7;
        m.selected = Some(2);
        m.panel_button(&mut w, 1).unwrap();
        assert_eq!(m.selected, Some(0));
    }

    /// Ledger rows ja1front:12086/12087: a click that presses a panel button (the field 0 box) starts the button hold
    /// at the mouse; a click on no button starts none.
    #[test]
    fn a_button_press_starts_the_hold() {
        let _g = crate::sound::SCREEN_TEST_LOCK.lock().unwrap_or_else(|e| e.into_inner());
        let mut w = world();
        let mut m = MapUi::new("", "", 1);
        m.mouse = (0, 199);
        let _ = m.click(&mut w, 1);
        assert!(m.hold.is_none());
        let (bx, by) = mappanel::BOXES[0];
        m.mouse = (bx + 1, by + 1);
        let _ = m.click(&mut w, 1);
        assert_eq!(m.box_down, Some(0));
        assert!(m.hold.is_some());
    }

    /// Ledger rows ja1front:12180/12181: the release of a held button sends the mouse back to its press place; a
    /// release with no button down moves nothing.
    #[test]
    fn a_button_release_moves_the_mouse_back() {
        let _g = crate::sound::SCREEN_TEST_LOCK.lock().unwrap_or_else(|e| e.into_inner());
        let mut w = world();
        let mut m = MapUi::new("", "", 1);
        m.hold = Some(crate::pointer::ButtonHold::press((5, 6)));
        m.release(&mut w, 1).unwrap();
        assert_eq!(m.warp, None);
        m.box_down = Some(0);
        m.mouse = (100, 100);
        let _ = m.release(&mut w, 1);
        assert_eq!(m.box_down, None);
        assert_eq!(m.warp, Some((5, 6)));
    }

    /// Ledger row w45:0x432F4:si_c: StrategicScreen 0x433C2 takes a click on pixel row 141 as a panel click (the
    /// panel branch); with no tab or box there it does nothing, and no map click runs.
    #[test]
    fn a_click_on_row_141_is_a_panel_click() {
        let _g = crate::sound::SCREEN_TEST_LOCK.lock().unwrap_or_else(|e| e.into_inner());
        let mut w = world();
        let mut m = MapUi::new("", "", 1);
        m.cur = 5;
        m.mouse = (0, 141);
        assert_eq!(m.click(&mut w, 1), Ok(()));
        assert_eq!((m.map_hold, m.last_click_msg), (0, MapUi::new("", "", 1).last_click_msg));
    }

    /// Ledger row ja1front:12020: a hover that changes the assignment screen (the "exit" tip over the face of the
    /// selected merc) keeps the new screen for the next pass.
    #[test]
    fn a_changed_assign_hover_screen_is_kept() {
        let _g = crate::sound::SCREEN_TEST_LOCK.lock().unwrap_or_else(|e| e.into_inner());
        let Some((mut m, w)) = game() else { return };
        m.assign = Some((vec![0; 64000], vec![0; 64000]));
        m.selected = Some(0);
        m.mouse = (45, 20);
        assert_eq!(m.assign_hover(&w), Ok(true));
        assert!(m.assign_screen.is_some());
    }

    /// Ledger row ja1front:12403: an item press gives the screen sound the game files when it has none, and keeps
    /// the files it has.
    #[test]
    fn an_item_press_loads_the_sound_files_once() {
        let _g = crate::sound::SCREEN_TEST_LOCK.lock().unwrap_or_else(|e| e.into_inner());
        let Some((mut m, mut w)) = game() else { return };
        m.assign = Some((vec![0; 64000], vec![0; 64000]));
        m.selected = Some(0);
        crate::sound::screen(|s| *s = crate::sound::Sound::default());
        let _ = m.item_press(&mut w, (-1, 0), 0);
        assert_eq!(crate::sound::screen(|s| s.files()).map(|f| f.0), Some(m.iso.clone()));
        crate::sound::screen(|s| *s = crate::sound::Sound::default());
        crate::sound::screen_files("x.gog", "x/JA.EXE");
        let _ = m.item_press(&mut w, (-1, 0), 0);
        assert_eq!(crate::sound::screen(|s| s.files()).map(|f| f.0), Some("x.gog".to_string()));
        crate::sound::screen(|s| *s = crate::sound::Sound::default());
    }

    /// Ledger row c2aea50:ja1front:5722: a press at screen column 0 is a press (on no item there), not key E: the
    /// merc keeps the items of its pockets.
    #[test]
    fn a_press_at_column_0_is_not_key_e() {
        let _g = crate::sound::SCREEN_TEST_LOCK.lock().unwrap_or_else(|e| e.into_inner());
        let Some((mut m, mut w)) = game() else { return };
        w.soldiers[0x182] = 1;
        w.soldiers[7 * 12] = 5;
        w.soldiers[7 * 12 + 1] = 1;
        w.soldiers[7 * 12 + 2] = 100;
        m.assign = Some((vec![0; 64000], vec![0; 64000]));
        m.selected = Some(0);
        let _ = m.item_press(&mut w, (0, 60), 0);
        assert_eq!(w.soldiers[7 * 12], 5);
        let _ = m.item_press(&mut w, (-1, 0), 0);
        assert_eq!(w.soldiers[7 * 12], 0);
    }
}
