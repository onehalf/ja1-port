//! Play (docs/spec/world_load.md section 8): the front-end as host during play, and the library's move order,
//! timer tick and frame on a `World`.
use crate::host::{Ja1Host, Ja1World, Rng};
use crate::invscreen::{can_open, InvInput, InvPoll, InvScreen};
use crate::world::World;
use crate::soldier::REC;
use std::collections::BTreeSet;
use std::ffi::c_void;

/// Host state during play: the random numbers (section 5) and the op numbers already named on stderr.
pub struct PlayHost {
    pub rng: Rng,
    /// Script line `clockseed N` (test only, validate/replay_week.py): the first draw after a day clock step writes
    /// the seed N + day * 0x10000 + minutes, and the island map's opening writes N + 0x7F000000 + day. The other side of
    /// the compare is a guest patch at 0x1F724 and 0x432F4 in the original.
    pub clock_seed: Option<(u32, u64)>,
    /// Script line `setupseed N` (test only, validate/replay_week.py): the next sector move writes the seed N right
    /// before its sector setup. The other side of the compare is a breakpoint at 0x2E918 in the original.
    pub setup_seed: Option<u32>,
    /// Script line `framefree` (test only, validate/replay_week.py): a per-frame draw uses this second seed, so the
    /// main number stream follows the actions and not the frame count. The other side of the compare is a guest patch
    /// at the original's per-frame `call Random` sites.
    pub frame_free: Option<u32>,
    /// `JA1_RNGLOG=1` (test only, validate/replay_week.py): each main-stream draw writes its seed and limit to stderr.
    pub rng_log: bool,
    /// Every op number the host answered (in the order of first use: `order`).
    pub seen: BTreeSet<i32>,
    pub order: Vec<i32>,
    /// Write `host op N` to stderr for each new op.
    pub log: bool,
    /// events.rs: the fight event log (`JA1_EVENTS=1`).
    pub ev: crate::events::EvLog,
    /// The front-end's animation timers (soldier_update.md "The front-end's animation timers").
    pub anims: AnimTimers,
    /// The latest turn banner (host op 194 banner, turns.md Turn start); the front-end's own state.
    pub banner: Option<Banner>,
    /// The latest "… hears a … noise …" line (host op 213 hears_msg, ai.md Hearing a noise).
    pub noise_msg: Option<String>,
    /// NPC quotes spoken since the last script log (category, quote): `speech CAT Q TEXT` lines.
    pub npc_said: Vec<(u8, i32)>,
    /// Host ops 302 guide_say / 317 brenda_say since the window or the script last showed them (op, arguments):
    /// the face balloons (panel.md "The guide's and Brenda's balloons").
    pub guide_said: Vec<(i32, Vec<i32>)>,
    /// The window's balloon for op 302 / 317, shown at the call (op, arguments): the original waits inside the say,
    /// so the quote comes before the guide offer (op 319) of the same steps (#36 D). None: `guide_said` keeps it.
    /// The third argument: the screen under the balloon when it is not the island map (`quote_under`).
    pub guide_say: Option<Box<dyn FnMut(i32, &[i32], Option<(&[u8], &[u8; 768])>)>>,
    /// The screen under a guide or Brenda quote of the day's verdict (strategic.md "The day's verdict"): DayEndVerdict
    /// 0x60756 speaks over the day replay screen (GuideSpeech? 0x5F5AC saves and restores the screen that is on), with
    /// the rating word of op 331 from step 5 on. None: the island map.
    pub quote_under: Option<(Vec<u8>, [u8; 768])>,
    /// The window starts the voices of ops 302 / 317 with their balloons (main.rs guide_balloon), not at the op.
    pub voice_later: bool,
    /// Host op 217 end_of_day (soldier_update.md Leaving the sector) came: the window title shows "end of day".
    pub end_of_day: bool,
    /// The selection's UI state (host ops 195..200, select.md); the front-end's own state.
    pub select: SelectUi,
    /// Host op 130 tile_marker (kind, tile) came (cursor.md "Tile marker"); Play puts it in its cursor state.
    pub marker: Option<(i32, i32)>,
    /// ClearMarksBeforeQuote? 0x11418 came with host op 348 walter_say: the original resets the pointer before the
    /// quote's wait loop (HidePathMarks(0): AP number 0x229DEC = 0 and cursor tile 0x229D2C = 0xC41; over_map 0,
    /// mouse wobble 0). Play clears its cursor.
    pub cursor_reset: bool,
    /// RedrawMercPortrait? 0x336B4 came with host op 139 step_sound_stop of StopAnim: the panel redraws and the
    /// selected soldier's stop clears the cursor tile 0x229D2C = 0xC41 (not with opt_onscreen 2+). Play clears its
    /// cursor tile; the portrait redraw is the front-end's whole-frame draw.
    pub marker_clear: bool,
    /// The window: when the first message box in the inventory screen ended (ShowMessage? 0x27CCA Unlock? frees the
    /// round lock that the screen's Lock?(0) took, so 1.13b time runs from there; #70 row 73).
    pub inv_box_end: Option<std::time::Instant>,
    /// CursorModeSet? 0x1C438 came with host op 185 click_panel (slot, mode) of TacticalClick, SoldierDies and
    /// StoryFind: Play sets the cursor mode and runs MoveCursorState on that slot after the call.
    pub click_panel: Option<(i32, i32)>,
    /// Op 206 pal_cycle during play (the ambient step of the tactical tick, Pal Cycling on): palette rotates
    /// (first, last) that the window or the script has not applied yet.
    pub pal_rot: Vec<(i32, i32)>,
    /// The world of the library call now running (set by `Session` before each call; null between calls): host
    /// ops 55 and 199 run Hide path marks on it.
    pub world: *mut Ja1World,
    /// soldier_update.md "Squad lost": the Play and the world of the running `Play::pass` (null else); host op 188
    /// moves to sector 59 inside the death with them.
    pub squad_play: *mut Play,
    pub squad_world: *mut World,
    /// The inventory screen while it is open (section 8 "Inventory screen"; null otherwise): its host ops go to it.
    pub inv: *mut InvScreen<'static>,
    /// The end-of-day screen (section 8 "End-of-day screen"); None = no front-end for it: its ops answer 0.
    pub eod: Option<crate::eod::EodHost>,
    /// The sunrise screen of op 304 (strategic.md "The sunrise screen"); None: no screen.
    pub sunrise: Option<crate::sunrise::Sunrise>,
    /// world_load.md section 9: the morning and the sunrise run (op 221 inv_poll answers 1: stop).
    pub day_phase: bool,
    /// Host op 304 sunrise_show came (section 9 step 3: the window title shows "Day N").
    pub sunrise_shown: bool,
    /// Host op 303 day_ask (kind, wait_row, go_row, number): the answer (section 9 step 4). None: answer 0.
    pub day_ask: Option<Box<dyn FnMut([i32; 4]) -> u32>>,
    /// The kind of the last op 303.
    pub day_ask_args: Option<[i32; 4]>,
    /// Script mode: called with the op 303 values before the answer (JA1_ASK_PNG pictures).
    pub ask_show: Option<Box<dyn FnMut([i32; 4])>>,
    /// The day's verdict (strategic.md): the score of op 331 rating_word (None = no op 331 came).
    pub rating: Option<i32>,
    /// Op 330 home_lost_msg came.
    pub home_lost: bool,
    /// The profiles of op 305 merc_gone, in order (section 9, the strategic screen opening).
    pub gone: Vec<i32>,
    /// The guide's advice (strategic.md; the last step of the strategic screen opening): the quotes of op 317
    /// brenda_say, in order.
    pub brenda: Vec<i32>,
    /// The world's `last_quote` while a session runs (npc_quote).
    pub last_quote: [u8; 6],
    /// The world's `quote_member` while a session runs (merc_quote).
    pub last_member: u8,
    /// The answer to op 319 guide_offer ("Accept this native guide?"). The opening runs before the map shows, so
    /// the front-end cannot ask the player; false (no) by default.
    pub guide_yes: bool,
    /// The window's question for op 319 (profile → answer); None: `guide_yes`.
    pub guide_ask: Option<Box<dyn FnMut(i32) -> bool>>,
    /// Op 345 disarm_ask (combat.md "Disarming an explosive"): the script's answers (`disarm Y|N` lines), used
    /// first, in order.
    pub disarm_answers: std::collections::VecDeque<bool>,
    /// The window's merc-face question for op 345 (text, profile, nickname → answer); None and no script answer: no.
    pub disarm_win: Option<Box<dyn FnMut(&[u8], u8, &[u8]) -> bool>>,
    /// Op 215 fade with no end-of-day screen open, in the window: FadePalette? 0x52908 on the tactical picture (steps, from, to).
    pub fade_win: Option<Box<dyn FnMut(i32, i32, i32)>>,
    /// Fan fix F28 (#70 row 17): the leave path's op 215 fade (steps, from, to) came after battle show steps of the
    /// same pass; the show plays lit first, then this fade runs.
    pub fade_held: Option<(i32, i32, i32)>,
    /// StoryFind 0x6179C runs inside one pass, and the original waits inside each quote: the window shows what the scene queued
    /// so far (merc balloons, Brenda) and redraws the picture, at each Brenda quote and fade.
    pub scene_win: Option<Box<dyn FnMut()>>,
    /// ShowMercMessage? 0x299B8 waits inside the call, and a right click on its balloon skips the view move: the rest
    /// of the pass reads the old view (on screen tests, blast volume). The window shows each merc balloon at op 54 /
    /// op 202 inside the pass (not in the story scene, which has `scene_win`).
    pub balloon_win: Option<Box<dyn FnMut()>>,
    /// The timer ticks of the balloons that `balloon_win` showed in this pass: (release wait, locked wait). The
    /// library's Timer tick of these ticks runs inside the pass (`timer_in_pass`), as the original's timer runs in
    /// the wait; the window runs the front-end's timers of them after the pass (no session may run inside a pass).
    pub balloon_owed: (u64, u64),
    /// `balloon_win` showed a balloon in this pass (op 157 restores the mouse behind it).
    pub balloon_shown: bool,
    /// `ja1_story_find` runs (ops 317 and 215 call `scene_win`).
    pub story_scene: bool,
    /// The window closed in `scene_win`.
    pub scene_closed: bool,
    /// The last op 345 question: text, profile, nickname (JA1_DISARM_PNG).
    pub disarm_last: Option<(Vec<u8>, u8, Vec<u8>)>,
    /// Op 292 corpse_ask (strategic.md "Corpse export"): the script's answers (`corpse Y|N` lines), used first.
    pub corpse_answers: std::collections::VecDeque<bool>,
    /// The window's question for op 292 (cost, can_pay → answer); None and no script answer: no.
    pub corpse_win: Option<Box<dyn FnMut(i32, bool) -> bool>>,
    /// The last op 292 question: cost, can_pay (JA1_CORPSE_PNG).
    pub corpse_last: Option<(i32, bool, bool)>,
    /// Op 297 raise_offer (strategic.md "Salary raise" step 3): the script's presses (`raise up|down|ok|no ...`
    /// lines, one line per box; 0 NO, 1 OK, 2 down, 3 up), used first.
    pub raise_presses: std::collections::VecDeque<Vec<usize>>,
    /// The window's salary box for op 297 (profile, nickname, start offer → offer, 0 = NO); None and no script
    /// line: NO.
    pub raise_win: Option<Box<dyn FnMut(u8, &[u8], i16) -> i16>>,
    /// The last op 297 box: profile, nickname, start offer, the script's presses (JA1_RAISE_PNG).
    pub raise_last: Option<(u8, Vec<u8>, i16, Vec<usize>)>,
    /// The profiles of op 319 guide_offer, in order.
    pub offers: Vec<i32>,
    /// Section 9 step 1, "Not enough funds" (strategic.md): the script's polls (x, y, buttons, the answer to an op
    /// 275 pd_ask in that poll), used when `pay_win` is None; none left: op 221 answers 1 (stop).
    pub pay_polls: std::collections::VecDeque<(i32, i32, i32, bool, i32)>,
    /// The script's value of op 272 pd_elapsed in the current poll (the fifth number of the `pay` line).
    pub pay_ms: i32,
    /// The answer to the next op 275 pd_ask (from the last script poll).
    pub pay_answer: bool,
    /// The window's poll (the shown messages of op 270, newest last; true when op 268 came since the last poll) →
    /// (x, y, buttons), None = the window closed (stop).
    pub pay_win: Option<Box<dyn FnMut(&[i32], &crate::dayloop::PayView, bool) -> Option<(i32, i32, i32)>>>,
    /// The window's answer to op 275 pd_ask (team slot, family).
    pub pay_ask: Option<Box<dyn FnMut(i32, i32) -> bool>>,
    /// Script mode: called at each script poll (shown messages, None) and each op 275 (messages, Some((team slot,
    /// family))), for the dialog's pictures (JA1_PAY_PNG, JA1_PAY_ASK_PNG).
    pub pay_shot: Option<Box<dyn FnMut(&[i32], &crate::dayloop::PayView, Option<(i32, i32)>)>>,
    /// The dialog's picture state besides the messages (ops 233/234, 269, 274, 278..280).
    pub pay_view: crate::dayloop::PayView,
    /// Op 276 pd_quote: the profile whose PST quote 0 the balloon of op 277 speaks.
    pub pay_quote: i32,
    /// Op 277 pd_balloon in order: (team slot, face x, profile); `pay balloon K X P` lines.
    pub pay_said: Vec<(i32, i32, i32)>,
    /// The window's balloon of op 277 (team slot, face x, profile, Subtitles), over the dialog's picture.
    pub pay_say: Option<Box<dyn FnMut(i32, i32, i32, bool)>>,
    /// Ops 289 cp_quote / 291 hate_quote (`MercSay`): the window or the script shows the boxes.
    pub merc_say: Option<Box<dyn FnMut(MercSay)>>,
    /// The `MercSay` of the last morning, for the script's `complain` lines.
    pub merc_said: Vec<MercSay>,
    /// Script mode: the picture of op 280 (JA1_PAY_END_PNG) and of op 277 (JA1_PAY_BALLOON_PNG).
    pub pay_end_shot: Option<Box<dyn FnMut(&crate::dayloop::PayView, Option<(i32, i32, i32, bool)>)>>,
    /// Script mode: called after each op 274 (JA1_PAY_BLINK_PNG: the first picture with the blink on).
    pub pay_blink_shot: Option<Box<dyn FnMut(&crate::dayloop::PayView)>>,
    /// Host op 301 payroll_show (strategic.md "The payroll screen"): the window shows the screen, the script draws
    /// it, over `under` (the replay screen as it is at the end of the day replay; None: the island map); returns the
    /// script line `payroll steps N sounds A B`.
    pub payroll_show: Option<Box<dyn FnMut(Option<&[u8]>) -> Option<String>>>,
    /// strategic.md "The day-end screen": the window waits for OK button `b` (15 on the replay screen `shot` with
    /// its palette, 16 on the payroll screen: None, the window keeps its own picture); no hook (script): no wait.
    pub dayend_win: Option<Box<dyn FnMut(&mut World, u8, Option<(&[u8], &[u8; 768])>) -> i32>>,
    /// strategic.md "The day-end screen": DayEndScreen? 0x65BD4 ends with a 24 step fade out (FadePalette? 0x52908,
    /// 256 -> 0) in the POSTPAN.PCC palette after the payroll wait and DayEndBankrupt?, or on the replay screen `shot`
    /// when the verdict ends the game (None: the payroll screen of the window); None (script): no fade.
    pub dayend_fade: Option<Box<dyn FnMut(Option<(&[u8], &[u8; 768])>)>>,
    /// panel.md "The mouse pointer": the window's pointer shape set by the morning (201 at op 301 and op 278, 0 at
    /// op 226 with 0 in the Not enough funds dialog and at op 287 xp_end).
    pub pointer: Option<Box<dyn FnMut(i32)>>,
    /// The mouse limit of the morning's host ops (mouselimit::of_op).
    pub mouse_limit: Option<Box<dyn FnMut(crate::mouselimit::Limit)>>,
    /// The mouse place of the morning's host ops (mouselimit::place_of_op).
    pub mouse_place: Option<Box<dyn FnMut(i32, i32)>>,
    /// Host ops 282..287 of the morning stat changes (strategic.md "The experience count"): the op and its
    /// arguments; returns a script line (the count of op 283 and the end of op 287).
    pub xp_op: Option<Box<dyn FnMut(i32, &[i32]) -> Option<String>>>,
    /// The `payroll ..` lines of the morning (day_steps moves them to the script lines).
    pub payroll_lines: Vec<String>,
    /// The texts of op 270 pd_message since the dialog began (0, 1, 2), the ops 275 (team slot, family, answer) in
    /// order, op 268 came since the last poll, and the start of the blink timer (op 271).
    pub pay_msgs: Vec<i32>,
    pub pay_asks: Vec<(i32, i32, bool)>,
    pub pay_redraw: bool,
    pub pay_timer: Option<i64>,
    /// Section 9 step 2, the office (strategic.md "The office"): the script's polls (x, y, buttons; bit 0 = a left
    /// press, bit 1 = the key V, bit 2 = a right press, bit 3 = the key Return), used when `office_win` is None; none
    /// left: the office ends as after Sleep. While the team ledger is open a poll with bit 0, 2 or 3 closes it.
    /// While the slot list is open (strategic.md "Save Game and Restore Game") every poll goes to it: B = the
    /// buttons held (bit 0 left, bit 1 right), with the key bytes of a `slots` line.
    pub office_polls: std::collections::VecDeque<(i32, i32, i32, Vec<u8>)>,
    /// Script line `aimdebt P N`: written when A.I.M. opens (the menu lines of a new game run after the parse).
    pub aim_debt: Option<(usize, u16)>,
    /// The window's office loop: returns when Sleep ended it (0), the window closed (1) Leave ended it (2) or Exit to DOS ended it (3).
    pub office_win: Option<Box<dyn FnMut(&mut World, &mut u32) -> i32>>,
    /// Script mode: the last office picture and its palette (6-bit), for JA1_OFFICE_PNG.
    pub office_shot: Option<Box<dyn FnMut(&[u8], &[u8; 768])>>,
    /// Script mode: the team ledger page when it opens, for JA1_LEDGER_PNG.
    pub ledger_shot: Option<Box<dyn FnMut(&[u8], &[u8; 768])>>,
    pub switchbox_shot: Option<Box<dyn FnMut(&[u8], &[u8; 768])>>,
    /// Script mode: the slot list after each of its polls, for JA1_SLOTS_PNG.
    pub slots_shot: Option<Box<dyn FnMut(&[u8], &[u8; 768])>>,
    /// Script mode: the A.I.M. screen after each of its polls that does not end it, for JA1_AIM_PNG.
    pub aim_shot: Option<Box<dyn FnMut(&[u8], &[u8; 768])>>,
    /// The stdout lines of the office (`office click K`, `office msg`, `office sleep`) and the window closed in it.
    pub office_lines: Vec<String>,
    pub office_quit: bool,
    /// `DAT\QUESTS.DAT` from the game folder, else the CD (section 9; op 307 quest_record). None: op 307 gives 250 zero bytes.
    pub quests: Option<Vec<u8>>,
    /// `DAT\EXPLODE.DAT` from the CD (combat.md Craters and blast leftovers; op 86 open_patch kind 5) and the read
    /// position while it is open (ops 96 seek, 87 read, 88 close).
    pub explode: Option<Vec<u8>>,
    pub explode_pos: Option<usize>,
    /// The tmp file made by op 219 save_tmp (sector `cur_sector`, bytes); `Play::pass` moves it into
    /// `World::tmp_files` when the session ends (the session's C view cannot reach the front-end world).
    pub saved_tmp: Option<(u8, Vec<u8>)>,
    /// Host op 188 squad_lost came in this pass: `Play::pass` runs soldier_update.md "Squad lost" after the update.
    pub squad_lost: bool,
    /// The status line message (section 8 "Time compression": key C refused, op 314 guards_msg); None = none.
    pub status: Option<String>,
    /// A message box to show (panel.md "Tactical messages"): text and time limit in ms (host ops 203, 326).
    pub msg_box: Option<(String, u32)>,
    /// Loud deaths (op 186 death_show with quiet 0) in order: the slots whose death morphs the window plays (panel.md
    /// "Faces"). A queue: two deaths in one batch of ticks (a slow frame) each keep their morph, as MercDies? 0x3A4A4
    /// plays it inside each death (a single slot dropped the first: smoke 38_window_squad_lost under load, 2026-11-07).
    /// Each entry keeps its own "NAME is dead!" text (None with the inventory open): MercDies? shows the box
    /// (MsgMercText 0x3A65C, 800 ms) right after the morph frames of that death, so two deaths show two boxes.
    pub morph: Vec<(u8, Option<String>)>,
    /// Message boxes that show one after the other once `msg_box` closed (cursor.md "Debug keys (level 4)", key E).
    pub msg_more: Vec<String>,
    /// cursor.md "Debug keys (level 4)": the soldier whose debug pages show next (Ctrl+P, Shift+left click).
    pub debug_pages: Option<usize>,
    /// strategic.md "The battle show": ops 332 / 333 / 334 of the pass in order; the window plays them after the pass
    /// (sounds, waits, boxes), a script starts the sounds and keeps the last box.
    pub battle_show: Vec<BattleEv>,
    /// strategic.md "The day replay": the day's events of op 89 replay_event (from, to, code) in order; the list
    /// empties at the first sector entry of a day (`replay_fresh`).
    pub replay_events: Vec<[u8; 3]>,
    pub replay_fresh: bool,
    /// Ops 355 replay_anim / 356 replay_draw of `ja1_day_replay` as sounds and waits; the day steps play them.
    pub replay_show: Vec<ReplayEv>,
    /// strategic.md "The replay pictures": the replay screen while `ja1_day_replay` runs (None: sounds only).
    pub replay_pic: Option<crate::replaypic::ReplayPic>,
    /// The fifth flag picture of the last day replay with its palette (JA1_REPLAY_PNG).
    pub replay_mid: Option<(Vec<u8>, [u8; 768])>,
    /// The last picture of the last day replay with its palette: the day's verdict draws its rating word on it
    /// (strategic.md "The day's verdict" step 5; JA1_RATING_PNG = that picture with the word).
    pub replay_last: Option<(Vec<u8>, [u8; 768])>,
    /// The start picture of the last day replay (the screen when the replay showed no picture).
    pub replay_start: Option<Vec<u8>>,
    /// Op 344 cursor_to_soldier (port_api.md): the merc whose inventory screen opens after the pass with the
    /// picked-up item on the cursor.
    pub pick_inv: Option<i32>,
    /// pick_inv came from op 344 (CursorItemToSoldier: the screen opens with the cursor item's info box).
    pub pick_info: bool,
    /// panel.md "Clicks on the bottom strip": op 350 mouse_to (x, y) for the window to warp the mouse to.
    pub mouse_to: Option<(i32, i32)>,
    /// The mouse at the start of the pass (pointer::turn_mouse).
    pub mouse_now: (i32, i32),
    /// A move of pointer::turn_mouse in this pass; see `Play::pass`.
    pub turn_warp: Option<(i32, i32)>,
    /// Op 157 came in this pass behind a balloon of the pass: MouseRestorePos2? (NewTurn? 0x12A44, InterruptTurn? 0x13150) stands
    /// behind SelectMerc?, so the window puts the mouse back again when the balloon ends.
    pub turn_back: bool,
    /// From op 147 mouse_save (EndTurn?) to op 157 mouse_refresh: the game does not read the mouse queue and the pointer is off.
    pub mouse_hold: bool,
    /// Op 352 weapon_cursor came: the cursor value follows the hand item after the strip click.
    pub weapon_due: bool,
    /// Op 351 wait_up (button): StripClick 0x54F3C ends in WaitLeftUp 0x67C54 or WaitRightUp 0x67C68; the window stops the game until the release.
    pub wait_up: i32,
    /// soldier_update.md "Merc sighting": the bytes of the last op 184 merc_sees, and the reactions (loop head and
    /// op 54 merc_message) in order.
    pub sight: crate::sighting::Sight,
    pub reactions: Vec<crate::sighting::Reaction>,
    /// Op 183 loss_msg (slot, amount, part) waiting for `loss_flush`: the library's calls see only `Ja1World`.
    pub loss_new: Vec<(i32, i32, i32)>,
    /// turns.md "New turn" step 14 e: the view wipes of ops 164 view_save (0 = close) and 165 view_restore (1 = open)
    /// in order; the window plays them (wipe.rs), a script logs `wipe close` / `wipe open`.
    pub wipes: Vec<u8>,
    /// combat.md "Doors" (op 210 door_anim): the door films of the pass in order (member, x, y, FLC bytes); the
    /// window plays them after the pass, a script logs them (`door film MEMBER X Y FRAMES`).
    pub door_films: Vec<(String, i32, i32, Vec<u8>)>,
    /// LoadPopups? 0x288DC (host op 71) came: the drown (flag 1) / injection (flag 0) popups of the pass in order
    /// (slot, flag, selected slot at the call). The window grows the picture over the slot after the pass; a script
    /// logs `anim popup SLOT FLAG FRAMES`.
    pub popups: Vec<(u8, i32, u8)>,
    /// sound.md: the effect bank, channels and mix.
    pub sound: crate::sound::Sound,
    /// The "sees enemies" line made by host op 197 during a session; the session puts it into the world.
    pub sees_text: Option<Vec<u8>>,
    /// The day bar length of the last op 309 day_bar (turns.md The day clock), kept for the status line.
    pub day_bar: i32,
    /// The item store records (type, count, condition) in list order while a session runs (`Session` moves
    /// `World::stash_records` in and back): host ops 264 stash_put and 308 stash_take.
    pub stash: Vec<[u8; 3]>,
}

/// ToggleMessage 0x3F994: the switch byte flips (xor 1) and the word gets " on" / " off" (the same bytes the
/// original appends from 0x9A04F / 0x9A069). The caller shows the text in the status box.
pub fn toggle_text(word: &str, switch: &mut u8) -> String {
    *switch ^= 1;
    format!("{}{}", word, if *switch != 0 { " on" } else { " off" })
}

/// panel.md "Toggle switch" step 1: Show Paths goes off. The library's Hide path marks runs while the switch is
/// still on (the caller flipped it already) with force = 1 when path_preview != 0. Returns marks_n before.
pub fn show_paths_off(w: &mut World, host: &mut PlayHost) -> i32 {
    let before = w.update.marks_n;
    crate::ctlpanel::set(w, 7, 1);
    let force = (w.path_preview != 0) as i32;
    Session::new(w, host).hide_path_marks(force);
    crate::ctlpanel::set(w, 7, 0);
    before
}

/// The gates of the Timer tick's steps 1-2 (soldier_update.md "Timer tick"): the animation timers run only while
/// both round locks are 0.
pub fn timer_gates_open(round_lock2: u8, round_lock: u8) -> bool {
    round_lock2 == 0 && round_lock == 0
}

/// TacticalKeyHandler case 0x20 (Space, the next merc): no step in a fight while the screen is not drawn or the
/// view is held, and none with an item on the cursor.
pub fn space_blocked(combat: bool, drawn: bool, view_hold: bool, held: bool) -> bool {
    (combat && (!drawn || view_hold)) || held
}

/// Op 264 stash_put: the sorted insert of `assign_items::ItemStore::put` (no store list shown here).
pub fn stash_put(recs: &mut Vec<[u8; 3]>, t: u8, q: u8, c: u8) -> u32 {
    let mut st = crate::assign_items::ItemStore { view: 0, records: std::mem::take(recs),
                                                  cur: crate::assign_items::NONE, top: 0, shown: [0; 4], v: 0 };
    let r = st.put(t, q, c);
    *recs = st.records;
    r
}

/// Op 308 stash_take (port_api.md): 1 when the store holds a record of type `t`; with `take` the first such
/// record goes out. The original misses the record when it is the last of the list and take = 0: the port does
/// the same. SPEC GAP: Shift (a whole stack) is not read: the day start runs without the keyboard.
pub fn stash_take(recs: &mut Vec<[u8; 3]>, t: u8, take: bool) -> u32 {
    match recs.iter().position(|r| r[0] == t) {
        None => 0,
        Some(i) if !take && i + 1 == recs.len() => 0,
        Some(i) => {
            if take {
                recs.remove(i);
            }
            1
        }
    }
}

/// combat.md "walter_say": the clip `wp<q>.8` of the CD archive WPRO\WPRO1 (q < 0x3C) or WPRO\WPRO2 and the decoded
/// record q (200 bytes) of EDT\WALQUOTE.EDT; None / "" when a file or member is missing.
pub fn walter_files(iso: &str, exe: &str, q: i32) -> (Option<Vec<u8>>, String) {
    let arc = if q < 0x3C { "WPRO\\WPRO1" } else { "WPRO\\WPRO2" };
    let clip = crate::iso::Iso::open(iso)
        .ok()
        .and_then(|mut cd| cd.read(arc).ok())
        .and_then(|a| crate::panel::archive_member(&a, &format!("WP{}.8", q)).ok());
    let edt = crate::aim::edt(iso, exe, "WALQUOTE.EDT");
    let at = (q.max(0) as usize) * 200;
    let text = crate::msgbox::game_text(&crate::balloon::shared_text(&edt, at));
    (clip, text)
}

/// Op 203 enemies_msg: MsgEnemiesNearby 0x1B0EC's text and milliseconds.
pub const NEARBY_MSG: (&str, u32) = ("Enemies nearby!", 1500);

/// The box of op 203: text and milliseconds. msgtext_diff reads this same function.
pub fn nearby_box() -> (String, u32) {
    (NEARBY_MSG.0.to_string(), NEARBY_MSG.1)
}
/// A status line message box stays this long (MsgStrInt 0x27CE0).
pub const STATUS_MS: u32 = 25000;

/// The status line text of op 314 guards_msg (count, number) (MsgStrInt 0x27CE0, turns.md The day clock).
pub fn guards_text(count: i32, number: i32) -> String {
    if count == 1 {
        format!("1 Guard arrives in sector {}", number)
    } else {
        format!("{} Guards arrive in sector", count)
    }
}

/// R (`sector_replay`, 60 x 9 bytes) of the world of the running library call.
/// LoadQuestRecord 0x614D4 (op 307): record `n` of DAT\QUESTS.DAT, bytes 250 (n - 5) .. + 249, into `out`.
/// Zero past the end of the file (the original reads short and keeps the old buffer; the game asks only for real records).
pub fn quest_record(quests: Option<&[u8]>, n: i32, out: &mut [u8]) {
    out.fill(0);
    if let (Some(q), Ok(start)) = (quests, usize::try_from((n as i64 - 5) * 250)) {
        for (i, b) in out.iter_mut().take(250).enumerate() {
            *b = q.get(start + i).copied().unwrap_or(0);
        }
    }
}

/// RecordReplayEvent 0x3FA04 (op 89): one record (from, to, code). The original appends the 3 bytes to replay.rec
/// and counts them in the word 0x22A46E; the port keeps the records with the world.
pub fn record_replay_event(log: &mut Vec<[u8; 3]>, from: i32, to: i32, code: i32) {
    log.push([from as u8, to as u8, code as u8]);
}

/// The count word 0x22A46E of RecordReplayEvent 0x3FA04.
pub fn replay_count(log: &[[u8; 3]]) -> u32 {
    log.len() as u32 & 0xFFFF
}

unsafe fn replay_table(host: &PlayHost) -> Option<Vec<u8>> {
    if host.world.is_null() || (*host.world).sector_replay.is_null() {
        return None;
    }
    Some(std::slice::from_raw_parts((*host.world).sector_replay, 540).to_vec())
}

/// strategic.md "The day replay", host op 355: the steps of one animation's sound timeline.
#[derive(Clone, Debug, PartialEq)]
pub enum ReplayEv {
    /// A `FX\REPLAY` member (`.8` added) on an effect channel at 11025 Hz with this volume.
    Sound(&'static str, i32),
    /// Effect e of the tactical bank (the merc's DIE.8) at 11025 Hz with this volume.
    Fx(i32, i32),
    /// A wait (ms).
    Wait(u32),
    /// A picture of the replay screen (320 x 200, the map palette; strategic.md "The replay pictures").
    Frame(Vec<u8>),
}

/// strategic.md "The replay pictures": which pictures of a replay timeline a player sees. The original draws each
/// step straight into the VGA memory and then the next step at once; only a wait keeps a picture on the screen. So a
/// picture is shown when the next picture or wait after it (sounds take no time) is a wait, or when it is the last
/// one. A picture with another picture right behind it (the clean cell before an explosion frame) is not shown.
pub fn replay_shown(ev: &[ReplayEv]) -> Vec<bool> {
    let mut shown = vec![false; ev.len()];
    let mut next_frame = false;
    for (i, e) in ev.iter().enumerate().rev() {
        match e {
            ReplayEv::Frame(_) => {
                shown[i] = !next_frame;
                next_frame = true;
            }
            ReplayEv::Wait(_) => next_frame = false,
            _ => {}
        }
    }
    shown
}

/// The sound timeline of host op 355 replay_anim (k, a1, a2, a3) (strategic.md "The day replay").
pub fn replay_steps(k: i32, a: [i32; 3]) -> Vec<ReplayEv> {
    use ReplayEv::*;
    match k {
        0 if a[2] != 0 => vec![Wait(3 * 90), Sound("WHOOSH", 88), Wait(5 * 90 + 90)],
        0 => vec![Wait(9 * 90)],
        1 if a[0] == a[1] => vec![],
        1 => (0..9).flat_map(|_| [Sound("MARCH2", 88), Wait(150)]).collect(),
        2 if a[1] == 0 => vec![Sound("BATTLE", 88), Wait(30)],
        2 => vec![Wait(30)],
        3 => vec![Sound("BURN", 88), Wait(11 * 90), Sound("ASHES", 88), Wait(11 * 90 + 90)],
        4 => (0..3).flat_map(|i| [Sound("EXPLODE", 104), Wait(if i == 0 { 400 } else { 200 }), Wait(if i == 2 { 600 } else { 400 })]).collect(),
        5 => vec![Fx(0x30 + 4 * a[2], 50), Wait(10 * 20 + 450 + 10 * 30 + 600 + 10 * 20)],
        _ => vec![],
    }
}

/// One step of the battle show (strategic.md "The battle show").
#[derive(Clone, Debug, PartialEq)]
pub enum BattleEv {
    /// Op 332 k 2 / 3 / 4: the speech channel plays this `FX\BATTLE` member.
    Sound(String),
    /// Op 332 k 5: SpeechRefill? 0x5CCF8, one stream refill and no wait (no loop at its 3 call sites).
    WaitSound,
    /// Op 332 k 6: the speech channel stops.
    Stop,
    /// Op 333 battle_wait (ms).
    Wait(u32),
    /// Op 334 battle_box (s, enemies, guards, tappers, kind).
    Box(i32, u8, u8, u8, i32),
    /// Op 332 k 9 (fan fix F27 only): the last key byte (0x95DC3) is cleared.
    ClearKey,
}

/// The text of op 335 battle_msg (t, n) (strategic.md Sector battles); n = the sector number s + 1.
pub fn battle_text(t: i32, n: i32) -> String {
    match t {
        0 => format!("Mine explodes, killing enemy in sector {}", n),
        1 => format!("GUARDS REGAIN SECTOR {}", n),
        2 => format!("ENEMY DEFEATED IN SECTOR {}", n),
        3 => format!("ENEMIES GAIN SECTOR {}", n),
        4 => format!("NATIVES LOSE SECTOR {}", n),
        _ => format!("GUARDS DEFEATED IN SECTOR {}", n),
    }
}

/// The status line text when key C is refused (turns.md "Time compression").
pub const NO_COMPRESS: &str = "Can't compress time, there are enemies nearby!";

/// The clock on the status line: `H:MM` (clock_hour, clock_minute with two digits; section 8).
pub fn clock_text(w: &World) -> String {
    format!("{}:{:02}", w.clock.hour, w.clock.minute)
}

/// The front-end's UI state that the selection ops (select.md host ops 195..200) change.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SelectUi {
    /// Op 195 select_light: the last light kind per slot (5 = off, 0xC = on; 0 = never set).
    pub lights: [u8; 48],
    /// Op 196 panel_view: the view window in view mode 3 (columns, rows) once a selection moved the panel.
    pub panel: Option<(i32, i32)>,
    /// Op 196: the panel timer (3000 when the selection moves the panel).
    pub panel_timer: i32,
    /// The mouse pointer's y (pixels) as op 196 sees it; op 196 can move it 14 pixels up.
    pub mouse_y: i32,
    /// Op 197 sees_msg: the slot of the latest "… sees N enemies!" message.
    pub sees_msg: Option<i32>,
    /// Op 199 cursor_state: the soldier the cursor was last updated for.
    pub cursor_slot: Option<i32>,
    /// Op 200 cursor_select: the cursor (state, cursor number); (1, 0x8E) = "a merc is selected".
    pub cursor: Option<(i32, i32)>,
}

impl Default for SelectUi {
    fn default() -> SelectUi {
        SelectUi {
            lights: [0; 48],
            panel: None,
            panel_timer: 0,
            mouse_y: 0,
            sees_msg: None,
            cursor_slot: None,
            cursor: None,
        }
    }
}

impl SelectUi {
    /// select.md "Panel view (op 196)": the new view_y. The window becomes 20 x 13 (182 pixels high), the panel
    /// timer 3000; with the mouse at y >= 182 and room below, the view scrolls 14 pixels down and the mouse 14 up.
    pub fn panel_view(&mut self, view_y: i32) -> i32 {
        self.panel = Some((20, 13));
        self.panel_timer = 3000;
        if self.mouse_y >= 182 && view_y.wrapping_add(14) < 0x310 - 182 {
            self.mouse_y -= 14;
            view_y.wrapping_add(14)
        } else {
            view_y
        }
    }
    /// One selection op (195..200) with its arguments; the op's result.
    pub fn op(&mut self, op: i32, a: &[i32]) -> u32 {
        let arg = |i: usize| a.get(i).copied().unwrap_or(0);
        match op {
            195 => {
                let k = arg(0) as u8 as usize;
                if k < self.lights.len() {
                    self.lights[k] = arg(1) as u8;
                }
                crate::panel::note_face_light(k, arg(1) as u8);
                0
            }
            196 => self.panel_view(arg(0)) as u32,
            197 => {
                self.sees_msg = Some(arg(0));
                0
            }
            199 => {
                self.cursor_slot = Some(arg(0));
                0
            }
            200 => {
                self.cursor = Some((1, 0x8E));
                0
            }
            // 198 mouse_wait: the front-end polls the mouse between frames; nothing to wait for here.
            _ => 0,
        }
    }
}

/// The turn banner that host op 194 banner (kind) asks for: text and colour (palette index) from turns.md.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Banner {
    pub kind: i32,
    pub text: &'static str,
    pub colour: u8,
}

impl Banner {
    /// turns.md Turn start, the banner table. None for a kind outside 0..4 (SPEC GAP: no such kind is listed).
    pub fn from_kind(kind: i32) -> Option<Banner> {
        let (text, colour) = match kind {
            0 => ("OPPONENTS' TURN...", 0x72),
            1 => ("GUARDS' TURN...", 0xA2),
            2 => ("OPPONENT INTERRUPTS!", 0x72),
            3 => ("GUARD INTERRUPTS!", 0xA2),
            4 => ("SANTINO INTERRUPTS!", 0x72),
            _ => return None,
        };
        Some(Banner { kind, text, colour })
    }
}

/// soldier_update.md "Traverse to a neighbour sector", the popup's condition: selected < 8, byte 2 (mercs) of
/// sector_state entry cur_sector >= 1 and hp (selected) > 15.
pub fn can_traverse(w: &World) -> bool {
    traverse_ok(w, w.selected as usize)
}

fn traverse_ok(w: &World, k: usize) -> bool {
    let mercs = w.sector_state.get(w.cur_sector as usize * 9 + 2).copied().unwrap_or(0);
    k < 8 && crate::travprompt::gate(k as u8, mercs, w.soldiers[k * 512 + 0x183])
}

/// The text of host op 125 message (text) (port_api.md); None for an unknown text.
pub fn message_text(text: i32) -> Option<&'static str> {
    Some(match text {
        1 => "Can't move there!",
        2 => "No one there",
        3 => "Can't get there!",
        4 => "Can't bury mine there!",
        6 => "Home sector LOST!",
        7 => "Better put that down first!",
        8 => "Can only drink from one item at a time!",
        9 => "Can only use one camouflage kit at a time!",
        10 => "Vest is not empty!",
        11 => "Can't traverse here!",
        12 => "Can't traverse during an interrupt!",
        13 => "Crowbar breaks!",
        14 => "Mine buried",
        15 => "No enemies visible!",
        16 => "Recipient too far!",
        17 => "Recipient not healthy enough!",
        18 => "Recipient doesn't have enough Action Points!",
        _ => return None,
    })
}

/// The merc text of host op 57 merc_text (slot, text, ms) (panel.md "Tactical messages"): "NAME TEXT" with NAME =
/// +0x16E up to the zero byte (at most 9 bytes); None for a soldier that is not a merc (+0x17A >= 8) or an unknown text.
pub fn merc_text(rec: &[u8], text: i32) -> Option<String> {
    let tail = match text {
        1 => "doesn't have enough Action Points!",
        2 => "has reloaded.",
        3 => "is shot retreating!",
        4 => "dies retreating!",
        5 => "abandoned first aid",
        6 => "is all bandaged!",
        7 => "ran out of bandages!",
        8 => "is already performing first aid! Press ESC to abort that first.",
        9 => "doesn't need first aid",
        10 => "doesn't have enough Action Points to traverse!",
        11 => "not healthy enough!",
        // TacticalLoop 0x108C2, the head hit of g_lossAmount -1 (loss_pass).
        12 => "hit in head, causing extra damage!",
        _ => return None,
    };
    if rec[0x17A] >= 8 {
        return None;
    }
    let nick = &rec[0x16E..0x16E + 9];
    let len = nick.iter().position(|&b| b == 0).unwrap_or(9);
    Some(format!("{} {}", String::from_utf8_lossy(&nick[..len]), tail))
}

/// Host op 183 loss_msg (slot, amount, part): HitLocationDamage 0x38EEC.. and StageEffects? 0x63630 arm the message.
/// Part 0 = a head hit with no stat loss (no text), 1 head, 2 arm, 3 leg, 4 the home warning of stage 0.
pub fn loss_set(w: &mut World, slot: i32, amount: i32, part: i32) {
    if part == 4 {
        w.loss_slot = 0;
        w.loss_timer = 7500;
        w.accounts[0x8F] = 0xFE;
        return;
    }
    let o = (slot as u8 as usize) * 512 + 0x17A;
    w.loss_slot = w.soldiers.get(o).copied().unwrap_or(0);
    w.loss_timer = 500;
    w.accounts[0x8F] = amount as u8;
    let (p, t): (&[u8], &[u8]) = match part {
        1 => (b"head", b"wisdom"),
        2 => (b"arm", b"dexterity"),
        3 => (b"leg", b"agility"),
        _ => return,
    };
    // strcpy: the bytes behind the zero stay (the quick save holds them).
    w.loss_part[..p.len()].copy_from_slice(p);
    w.loss_part[p.len()] = 0;
    w.loss_stat[..t.len()].copy_from_slice(t);
    w.loss_stat[t.len()] = 0;
}

/// The ops 183 of the library calls since the last flush, in order.
fn loss_flush(host: &mut PlayHost, w: &mut World) {
    for (k, a, p) in std::mem::take(&mut host.loss_new) {
        loss_set(w, k, a, p);
    }
}

/// TacticalTimer? 0x5EFDE..0x5EFFD: 10 less per tick while above 0, floor 0; -1 stays.
pub fn loss_timer_tick(t: &mut i32) {
    if *t > 0 {
        *t = (*t - 10).max(0);
    }
}

/// What TacticalLoop 0x10800..0x108D8 does in a pass.
#[derive(Debug, PartialEq)]
pub enum LossShow {
    Box(String, u32),
    /// TeamQuote? 0x61688 (0x1C, 0xE, 0): op 202.
    Quote,
}

/// TacticalLoop 0x10800..0x108D8, each pass: the message of `loss_set` when its timer ended.
pub fn loss_pass(w: &mut World) -> Option<LossShow> {
    if w.loss_timer != 0 || w.loss_slot >= 0x40 {
        return None;
    }
    w.loss_timer = -1;
    let amount = w.accounts[0x8F] as i8;
    if amount < -1 {
        return Some(LossShow::Quote);
    }
    let k = w.loss_slot as usize * 512;
    let rec = w.soldiers.get(k..k + 512)?;
    if amount == -1 {
        return merc_text(rec, 12).map(|t| LossShow::Box(t, 800));
    }
    let cs = |b: &[u8]| String::from_utf8_lossy(&b[..b.iter().position(|&c| c == 0).unwrap_or(b.len())]).into_owned();
    let t = format!("{} receives a critical {} injury, losing {} {} {}", cs(&rec[0x16E..0x16E + 9]), cs(&w.loss_part),
                    amount, if amount == 1 { "point of" } else { "points of" }, cs(&w.loss_stat));
    Some(LossShow::Box(t, 2000))
}

/// Op 186's text (shooting.md "Merc dies"): "NAME is dead!" as op 57 merc_text; None for a non-merc.
pub fn dead_text(rec: &[u8]) -> Option<String> {
    if rec[0x17A] >= 8 {
        return None;
    }
    let nick = &rec[0x16E..0x16E + 9];
    let len = nick.iter().position(|&b| b == 0).unwrap_or(9);
    Some(format!("{} is dead!", String::from_utf8_lossy(&nick[..len])))
}

/// The "sees enemies" line of soldier record `rec` (MsgSeesEnemies 0x11448; panel.md "Tactical messages"):
/// "NAME sees N enemies!" or
/// "NAME sees 1 enemy!" with NAME = +0x16E up to the zero byte and N = +0x1D5 (signed).
pub fn sees_text(rec: &[u8]) -> Vec<u8> {
    // SPEC GAP: a nickname with no zero byte before the record end stops at the record end.
    let name = &rec[0x16E..];
    let name = &name[..name.iter().position(|&b| b == 0).unwrap_or(name.len())];
    let n = rec[0x1D5] as i8;
    let mut t = name.to_vec();
    if n == 1 {
        t.extend_from_slice(b" sees 1 enemy!");
    } else {
        t.extend_from_slice(format!(" sees {} enemies!", n).as_bytes());
    }
    t
}

/// Host op 213 hears_msg (slot, level, kind, direction): the text of MsgHearsNoise? 0x72A60, `rec` = the listener.
pub fn hears_text(rec: &[u8], level: i32, kind: i32, dir: i32) -> String {
    const LEVELS: [&str; 4] = ["FAINT", "DEFINITE", "LOUD", "VERY LOUD"];
    const KINDS: [&str; 10] =
        ["UNKNOWN", "MOVEMENT", "CREAKING", "SPLASHING", "IMPACT", "GUNFIRE", "EXPLOSION", "SCREAM", "IMPACT", "IMPACT"];
    // Table 0x9AEBB, 10 bytes per direction; DirectionTo 0 reads the "-" in front of the table.
    const DIRS: [&str; 9] = ["-", "north", "northeast", "east", "southeast", "south", "southwest", "west", "northwest"];
    let pick = |t: &[&'static str], i: i32| t.get(i as usize).copied().unwrap_or("?");
    let nick = &rec[0x16E..];
    let len = nick.iter().position(|&b| b == 0).unwrap_or(nick.len());
    // sprintf "%s %s %s %s %s %s" (0x9161C) with "hears a " (0x9AAC5): two spaces in front of the level.
    format!("{} hears a  {} {} noise coming from the {}", String::from_utf8_lossy(&nick[..len]), pick(&LEVELS, level),
            pick(&KINDS, kind), pick(&DIRS, dir))
}

/// Byte size of one `fx_anims` entry.
pub const FX_REC: usize = 0x12;

/// QueueQuote? 0x1B2A4 (soldier_update.md "Queue a merc quote"), arguments slot, event, delay, tile, focus: a pending
/// 0xFE quote of the queued slot plays first with delay 0, then the queue takes the arguments. Tile 0xC41 keeps the
/// old tile.
///
/// # Safety
/// `w` must be valid.
pub unsafe fn queue_quote(w: *mut Ja1World, a: &[i32], play: impl FnOnce(*mut Ja1World, i32)) {
    if (*w).quote_slot < 0x40 && (*w).quote_event == 0xFE {
        (*w).quote_delay = 0;
        play(w, (*w).quote_slot as i32);
    }
    (*w).quote_slot = a[0] as u8;
    (*w).quote_event = a[1] as u8;
    (*w).quote_focus = a[4] as u8;
    (*w).quote_delay = a[2] as u8 as i8;
    if a[3] as i16 != 0xC41 {
        (*w).quote_tile = a[3] as u16;
    }
}

/// Host op 178 puff (tile, x, y, look, count, kind, power) on the effect records `fx` (combat.md The explosion
/// effect record): the first entry 1..15 with count 0 gets count − 1, x, y, tile, look, kind, power and hit flag 0.
/// Layout (the front-end's own beyond byte 0): +0 count, +5 x, +7 y, +9 tile (i16), +0xB kind, +0xC look (i32),
/// +0x10 hit flag, +0x11 power. Returns the entry number, or −1 when all are in use.
pub fn puff(fx: &mut [u8], a: &[i32]) -> i32 {
    let Some(k) = (1..FX_ANIM_COUNT).find(|&k| fx[k * FX_REC] == 0) else { return -1 };
    let e = &mut fx[k * FX_REC..(k + 1) * FX_REC];
    e[0] = (a[4] as u8).wrapping_sub(1);
    e[5..7].copy_from_slice(&(a[1] as i16).to_le_bytes());
    e[7..9].copy_from_slice(&(a[2] as i16).to_le_bytes());
    e[9..11].copy_from_slice(&(a[0] as i16).to_le_bytes());
    e[0xB] = a[5] as u8;
    e[0xC..0x10].copy_from_slice(&a[3].to_le_bytes());
    e[0x10] = 0;
    e[0x11] = a[6] as u8;
    k as i32
}

/// Entry k hits in a pass with count limit `limit` (blast.rs effect_pass): (tile, kind, power), the flag unchanged.
pub fn effect_hit_due(fx: &[u8], k: usize, limit: i8) -> Option<(i32, i32, i32)> {
    let e = fx.get(k * FX_REC..(k + 1) * FX_REC)?;
    if e[0] == 0 || e[0xB] == 0 || e[0] as i8 > limit || e[0x10] != 0 {
        return None;
    }
    Some((i16::from_le_bytes([e[9], e[10]]) as i32, e[0xB] as i8 as i32, e[0x11] as i8 as i32))
}

/// combat.md "Drawing the effect records", the fire: entry k of kind 3 with count 5 whose tile has byte 3 = 0 and
/// byte 0 below 0x6C gives its tile (the fire starts there).
pub fn fire_tile(fx: &[u8], k: usize, tiles: &[u8]) -> Option<i32> {
    let e = fx.get(k * FX_REC..(k + 1) * FX_REC)?;
    if e[0] != 5 || e[0xB] != 3 {
        return None;
    }
    let t = i16::from_le_bytes([e[9], e[10]]) as i32;
    // SPEC GAP: a tile off the map (not 0..3135) starts no fire.
    let b = tiles.get(t.max(0) as usize * 8..t.max(0) as usize * 8 + 8).filter(|_| t >= 0)?;
    (b[3] == 0 && b[0] < 0x6C).then_some(t)
}

/// Number of tile animations (soldier_update.md: up to 10).
pub const TILE_ANIMS: usize = 10;
/// Number of effect animations (the 16 `fx_anims` entries).
pub const FX_ANIM_COUNT: usize = 16;
/// The tile value for "no tile".
pub const NO_TILE: i16 = 0xC41;

/// One tile animation: a count, a timer and a tile (the front-end sets the tile's byte 3 to the frame).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct TileAnim {
    pub count: i32,
    pub timer: i32,
    pub tile: i16,
}

impl Default for TileAnim {
    fn default() -> TileAnim {
        TileAnim { count: 0, timer: 0, tile: NO_TILE }
    }
}

/// soldier_update.md "The front-end's animation timers": the tile animations and the timers of the `fx_anims`
/// entries (their counts are byte 0 of each entry in the world).
#[derive(Clone, Debug, Default)]
pub struct AnimTimers {
    pub tiles: [TileAnim; TILE_ANIMS],
    /// The frame table of each tile animation (the front-end's own id: `RIPPLE_FRAMES` = the water ripple).
    pub frames: [u8; TILE_ANIMS],
    /// SPEC GAP: the spec does not say where in an `fx_anims` entry the timer is; the front-end keeps it here.
    pub fx_timers: [i32; FX_ANIM_COUNT],
}

/// The front-end's id of the water ripple's frame table (soldier_update.md "Update end": count 0x12).
pub const RIPPLE_FRAMES: u8 = 1;
/// The count of the water ripple's frame table.
pub const RIPPLE_TABLE_COUNT: i32 = 0x12;
/// The front-end's id of the fire's frame table (combat.md "Drawing the effect records": count 0x2B).
pub const FIRE_FRAMES: u8 = 2;
/// The count of the fire's frame table.
pub const FIRE_TABLE_COUNT: i32 = 0x2B;
/// The frame tables 0x95659 and 0x9562E as the file holds them (image address + DOS/4GW's load base).
const RIPPLE_TABLE_PTR: u32 = 0x95659 + 0x1F1000;
const FIRE_TABLE_PTR: u32 = 0x9562E + 0x1F1000;
/// soldier_update.md: the water ripple's frames (indexed by the count).
pub const RIPPLE_TABLE: [u8; 0x12] = [0, 0xC0, 0xC1, 0xC2, 0xC3, 0xC4, 0xC5, 0xC6, 0xC7, 0xC8, 0xC9, 0xCA, 0xCB, 0xCC, 0xCD, 0xCE, 0xCF, 0xD0];
/// combat.md "Drawing the effect records": the fire's frames (indexed by the count).
pub const FIRE_TABLE: [u8; 0x2C] = [
    0, 0x89, 0x88, 0x87, 0x86, 0x85, 0x84, 0x83, 0x82, 0x81, 0x80, 0x83, 0x82, 0x81, 0x80, 0x83, 0x82, 0x81, 0x80, 0x83,
    0x82, 0x81, 0x80, 0x83, 0x82, 0x81, 0x80, 0x83, 0x82, 0x81, 0x80, 0x83, 0x82, 0x81, 0x80, 0x83, 0x82, 0x81, 0x80,
    0x83, 0x82, 0x81, 0x80, 0,
];

impl AnimTimers {
    /// Host op 205 tile_anim (tile, kind) (soldier_update.md "Update end", the front-end's part): kind 0 = the
    /// water ripple. The first free tile animation of 1..9 (free = count 0; animation 0 is never used) gets count
    /// 0x11, the ripple's frames and the tile (its timer stays as it is); the tile's byte 3 = the animation's
    /// number. Returns the animation's number, or None (no free animation, or a kind other than 0).
    pub fn start(&mut self, tile: i32, kind: i32, tiles: &mut [u8]) -> Option<usize> {
        if kind != 0 {
            // SPEC GAP: only kind 0 (the water ripple) is described; another kind starts nothing.
            return None;
        }
        self.start_table(tile, RIPPLE_FRAMES, RIPPLE_TABLE_COUNT, tiles)
    }

    /// Starts a tile animation with frame table `table` (`RIPPLE_FRAMES`, `FIRE_FRAMES`) and its count: the
    /// first free animation of 1..9 gets count - 1, the frames and the tile; the tile's byte 3 = its number.
    pub fn start_table(&mut self, tile: i32, table: u8, count: i32, tiles: &mut [u8]) -> Option<usize> {
        let k = (1..TILE_ANIMS).find(|&k| self.tiles[k].count == 0)?;
        let a = &mut self.tiles[k];
        a.count = count - 1;
        a.tile = tile as i16;
        self.frames[k] = table;
        // SPEC GAP: a tile off the map (not 0..3135) gets no byte 3 (the library only asks for map tiles).
        // StartTileAnim 0x15754: byte 3 only when count - 1 != 0 (the 3 call sites pass 0x12 or 0x2B: always).
        let o = (tile as i16 as isize) * 8 + 3;
        if a.count != 0 && o >= 3 && (o as usize) < tiles.len() {
            tiles[o as usize] = k as u8;
        }
        Some(k)
    }

    /// Quick save block 78: 10 entries of 11 bytes as 0x229E24 holds them: s8 count, i32 timer, i16 tile, u32 pointer to the
    /// frame table. An entry that never ran is all 0 (the original's bss); one that ended keeps tile 0xC41 and its pointer.
    pub fn to_block(&self) -> [u8; 110] {
        let mut b = [0u8; 110];
        for (k, e) in b.chunks_exact_mut(11).enumerate() {
            let table = match self.frames[k] {
                FIRE_FRAMES => FIRE_TABLE_PTR,
                RIPPLE_FRAMES => RIPPLE_TABLE_PTR,
                _ => continue,
            };
            let a = &self.tiles[k];
            e[0] = a.count as u8;
            e[1..5].copy_from_slice(&a.timer.to_le_bytes());
            e[5..7].copy_from_slice(&a.tile.to_le_bytes());
            e[7..11].copy_from_slice(&table.to_le_bytes());
        }
        b
    }

    /// QuickRestore 0x2C898 reads the tile animations back, so a ripple or a fire goes on and its end clears the tile's byte 3.
    pub fn from_block(&mut self, b: &[u8; 110]) {
        for (k, e) in b.chunks_exact(11).enumerate() {
            self.tiles[k] = TileAnim {
                count: e[0] as i8 as i32,
                timer: i32::from_le_bytes([e[1], e[2], e[3], e[4]]),
                tile: i16::from_le_bytes([e[5], e[6]]),
            };
            self.frames[k] = match u32::from_le_bytes([e[7], e[8], e[9], e[10]]) {
                FIRE_TABLE_PTR => FIRE_FRAMES,
                RIPPLE_TABLE_PTR => RIPPLE_FRAMES,
                _ => 0,
            };
        }
    }

    /// TacticalTimer 0x5F11x keeps an effect record's timer at its +1, so QuickSave 0x2E0B0 writes it inside block 79 and
    /// QuickRestore 0x2C898 reads it back. The port keeps the timers here and copies them at the save and the restore.
    pub fn fx_timers_to(&self, fx: &mut [u8]) {
        for (t, e) in self.fx_timers.iter().zip(fx.chunks_exact_mut(FX_REC)) {
            e[1..5].copy_from_slice(&t.to_le_bytes());
        }
    }

    pub fn fx_timers_from(&mut self, fx: &[u8]) {
        for (t, e) in self.fx_timers.iter_mut().zip(fx.chunks_exact(FX_REC)) {
            *t = i32::from_le_bytes([e[1], e[2], e[3], e[4]]);
        }
    }

    /// tactical_map.md "Animated tiles": the frame (sprite 413 + it) of each tile animation 0..9, or None when
    /// its count is 0 or less.
    pub fn sprite_frames(&self) -> [Option<u8>; TILE_ANIMS] {
        let mut f = [None; TILE_ANIMS];
        for (k, a) in self.tiles.iter().enumerate() {
            if a.count > 0 {
                let t: &[u8] = if self.frames[k] == FIRE_FRAMES { &FIRE_TABLE } else { &RIPPLE_TABLE };
                // SPEC GAP: a count past its table reads frame 0.
                f[k] = Some(t.get(a.count as usize).copied().unwrap_or(0));
            }
        }
        f
    }

    /// One 10 ms step. `tiles` = the world's tile bytes (3136 x 8), `fx` = the `fx_anims` buffer (16 x 0x12).
    pub fn step(&mut self, tiles: &mut [u8], fx: &mut [u8]) {
        for a in self.tiles.iter_mut() {
            if a.count <= 0 {
                continue;
            }
            a.timer = a.timer.wrapping_sub(10);
            if a.timer > 0 {
                continue;
            }
            a.count = a.count.wrapping_sub(1);
            if a.count > 0 {
                a.timer = 140;
            } else if a.tile < NO_TILE {
                // SPEC GAP: a negative tile (below 0xC41 as s16) is off the map; nothing is written for it.
                let o = a.tile as isize * 8 + 3;
                if a.tile >= 0 && (o as usize) < tiles.len() {
                    tiles[o as usize] = 0;
                }
                a.tile = NO_TILE;
            }
        }
        for (k, t) in self.fx_timers.iter_mut().enumerate() {
            let Some(c) = fx.get_mut(k * 0x12) else { break };
            if *c == 0 {
                continue;
            }
            *t = t.wrapping_sub(10);
            if *t <= 0 {
                *t = 140;
                *c = c.wrapping_sub(1);
            }
        }
    }
}

impl PlayHost {
    /// The original's timer (TacticalTimer? 0x5EEA4) runs while ShowMercMessage? 0x299B8 waits inside a pass, so the
    /// rest of that pass reads its values (show timer, busy and fast timers, fuses). Inside a host op the library's
    /// world is `self.world`: its Timer tick runs `n` times there (`locked` = the balloon's Lock: only the ticks under
    /// round_lock run). The Timer tick makes no host call. The front-end's timers of the same ticks run after the
    /// pass (`Play::drag_ticks_ui`, `Play::locked_ticks_ui`).
    pub fn timer_in_pass(&mut self, n: u64, locked: bool) {
        let w = self.world;
        if w.is_null() || n == 0 {
            return;
        }
        let h = Ja1Host { ctx: self as *mut PlayHost as *mut c_void, random: cb_random, call: cb_call, host_ops: 0 };
        // SAFETY: `w` is the library world of the running pass (set by Session::bind), and the Timer tick makes no
        // host call, so nothing else reads or writes it while this runs.
        unsafe {
            let old = (*w).round_lock;
            if locked && old == 0 {
                (*w).round_lock = 1;
            }
            for _ in 0..n {
                // TacticalTimer? 0x5EEA4 steps 1-2 under the gates: the tile and effect animation timers run in the
                // wait too, and the frame steps after it in the same pass read the effect counts.
                let gates_open = timer_gates_open((*w).round_lock2, (*w).round_lock);
                ja1_tick(w, &h);
                if gates_open && !(*w).tiles.is_null() && !(*w).fx_anims.is_null() {
                    let tiles = std::slice::from_raw_parts_mut((*w).tiles, 3136 * 8);
                    let fx = std::slice::from_raw_parts_mut((*w).fx_anims as *mut u8, crate::world::FX_ANIMS);
                    self.anims.step(tiles, fx);
                }
            }
            (*w).round_lock = old;
        }
    }

    /// A status text: the original shows every one in the message box with the 25 000 ms limit (StatusMessage;
    /// panel.md "Tactical messages"); the window title keeps a copy.
    pub fn status_msg(&mut self, t: String) {
        self.status = Some(t.clone());
        self.msg_box = Some((t, STATUS_MS));
    }

    /// sound.md "Merc quotes": a reaction with a speech balloon starts the merc's voice.
    /// soldier_update.md "Merc message" step 2: the merc's "BAD" cry (profile p) at rate 0x2B11, volume 50, once.
    pub fn spotted_cry(&mut self, p: Option<u8>) {
        let Some(p) = p else { return };
        if self.world.is_null() || unsafe { (*self.world).soldiers.is_null() } {
            return;
        }
        // SAFETY: the world's soldier buffer holds 48 records.
        let mercs = unsafe { std::slice::from_raw_parts((*self.world).soldiers, 8 * 512) };
        self.sound.op(100, &[p as i32, -1, 0, 1, 1], mercs);
        self.sound.op(101, &[0x2B11, 50, 1], mercs);
        eprintln!("spotted cry profile {} BAD", p);
    }

    /// The voice of a reaction's balloon: NAT quote q (the sector line) or DAY quote q of profile p.
    pub fn reaction_voice(&mut self, p: u8, q: i32, nat: bool) {
        if nat {
            self.sound.cat_quote("NAT", p, q);
        } else {
            self.sound.quote(p, q);
        }
    }

    pub fn say(&mut self, r: &crate::sighting::Reaction) {
        if let Some(rec) = r.balloon.as_ref() {
            // A balloon that waits behind another one of the same pass gets its voice when it shows (main.rs).
            if !self.reactions.iter().any(|o| o.balloon.is_some()) {
                self.reaction_voice(rec[0x179], r.how as i32, r.nat);
            }
            // world_load.md "The last quote": the merc message's DAY quote; the sector line's NAT quote (row 0).
            if r.nat {
                self.last_quote = [rec[0x179], 8, 0, 0, r.how as u8, 0];
            } else {
                set_quote(&mut self.last_quote, rec[0x179], 1, r.how as i32);
            }
        }
    }

    pub fn new(seed: u32) -> PlayHost {
        PlayHost {
            rng: Rng::new(seed),
            clock_seed: None,
            setup_seed: None,
            frame_free: None,
            rng_log: std::env::var_os("JA1_RNGLOG").is_some(),
            seen: BTreeSet::new(),
            order: Vec::new(),
            log: true,
            ev: crate::events::EvLog::new(),
            anims: AnimTimers::default(),
            banner: None,
            noise_msg: None,
            npc_said: Vec::new(),
            guide_said: Vec::new(),
            guide_say: None,
            voice_later: false,
            end_of_day: false,
            select: SelectUi::default(),
            marker: None,
            cursor_reset: false,
            marker_clear: false,
            inv_box_end: None,
            click_panel: None,
            pal_rot: Vec::new(),
            world: std::ptr::null_mut(),
            squad_play: std::ptr::null_mut(),
            squad_world: std::ptr::null_mut(),
            inv: std::ptr::null_mut(),
            eod: None,
            sunrise: None,
            day_phase: false,
            sunrise_shown: false,
            day_ask: None,
            day_ask_args: None,
            ask_show: None,
            rating: None,
            home_lost: false,
            gone: Vec::new(),
            brenda: Vec::new(),
            last_quote: [0; 6],
            last_member: 0,
            guide_yes: false,
            guide_ask: None,
            offers: Vec::new(),
            pay_polls: Default::default(),
            pay_ms: 0,
            pay_view: Default::default(),
            pay_quote: 0,
            pay_said: Vec::new(),
            pay_say: None,
            merc_say: None,
            merc_said: Vec::new(),
            pay_end_shot: None,
            pay_blink_shot: None,
            office_polls: Default::default(),
            aim_debt: None,
            office_win: None,
            office_shot: None,
            ledger_shot: None,
            switchbox_shot: None,
            slots_shot: None,
            aim_shot: None,
            office_lines: vec![],
            office_quit: false,
            pay_answer: false,
            pay_win: None,
            pay_ask: None,
            pay_shot: None,
            payroll_show: None,
            dayend_win: None,
            dayend_fade: None,
            pointer: None,
            mouse_limit: None,
            mouse_place: None,
            payroll_lines: Vec::new(),
            xp_op: None,
            pay_msgs: Vec::new(),
            pay_asks: Vec::new(),
            pay_redraw: false,
            pay_timer: None,
            quests: None,
            explode: None,
            explode_pos: None,
            saved_tmp: None,
            squad_lost: false,
            status: None,
            msg_box: None,
            morph: Vec::new(),
            msg_more: Vec::new(),
            debug_pages: None,
            battle_show: Vec::new(),
            replay_events: Vec::new(),
            replay_fresh: true,
            replay_show: Vec::new(),
            replay_pic: None,
            replay_mid: None,
            replay_last: None,
            quote_under: None,
            replay_start: None,
            pick_inv: None,
            pick_info: false,
            mouse_to: None,
            mouse_now: (0, 0),
            turn_warp: None,
            turn_back: false,
            mouse_hold: false,
            weapon_due: false,
            wait_up: 0,
            disarm_answers: Default::default(),
            disarm_win: None,
            fade_win: None,
            fade_held: None,
            scene_win: None,
            balloon_win: None,
            balloon_owed: (0, 0),
            balloon_shown: false,
            story_scene: false,
            scene_closed: false,
            disarm_last: None,
            corpse_answers: Default::default(),
            corpse_win: None,
            corpse_last: None,
            raise_presses: Default::default(),
            raise_win: None,
            raise_last: None,
            sight: Default::default(),
            reactions: Vec::new(),
            loss_new: Vec::new(),
            wipes: Vec::new(),
            door_films: Vec::new(),
            popups: Vec::new(),
            sound: Default::default(),
            sees_text: None,
            day_bar: 0,
            stash: Vec::new(),
        }
    }

    /// "Host calls during play": every op that comes to the host gives result 0 and changes nothing; each new op
    /// number is written once to stderr.
    pub fn call(&mut self, op: i32) -> u32 {
        if self.seen.insert(op) {
            self.order.push(op);
            if self.log {
                eprintln!("host op {}", op);
            }
        }
        // SPEC GAP: "result 0 and no world change" does not say what goes into an op's out buffer; it is not written.
        0
    }
}

unsafe extern "C" fn cb_random(ctx: *mut c_void, n: i32) -> i32 {
    let host = &mut *(ctx as *mut PlayHost);
    host.ev.poll(host.world);
    if let (Some((base, last)), false) = (host.clock_seed, host.world.is_null()) {
        let now = ja1_clock_steps();
        if now != last {
            host.rng.seed = base.wrapping_add(((*host.world).day as u32) << 16).wrapping_add((*host.world).day_minutes as u32);
            host.clock_seed = Some((base, now));
        }
    }
    if let (Some(frame), true) = (host.frame_free, ja1_frame_draw() != 0) {
        let seed = std::mem::replace(&mut host.rng.seed, frame);
        let r = host.rng.random(n);
        host.frame_free = Some(std::mem::replace(&mut host.rng.seed, seed));
        return r;
    }
    if host.rng_log {
        eprintln!("rng {:08x} {}", host.rng.seed, n);
    }
    host.rng.random(n)
}

/// world_load.md section 9 step 1: the host ops of the morning's "Not enough funds" dialog (strategic.md), or None
/// for any other op. Op 221 inv_poll: the next poll of the window (`pay_win`) or of the script (`pay_polls`); none
/// left: 1 (stop, as before the dialog had a front-end). Op 270 pd_message keeps its text, 271 / 272 the blink
/// timer on the game clock (crate::dostime; the script's elapsed time is always 0), 275 pd_ask the answer, 268 island_map asks for a new picture.
///
/// # Safety
/// `host.world` is the world of the running library call; `args` holds `nargs` values.
unsafe fn pay_op(host: &mut PlayHost, op: i32, args: *const i32, nargs: i32) -> Option<u32> {
    let arg = |i: i32| if !args.is_null() && i < nargs { *args.add(i as usize) } else { 0 };
    Some(match op {
        221 => {
            let p = match host.pay_win.as_mut() {
                Some(f) => {
                    let redraw = std::mem::take(&mut host.pay_redraw);
                    f(&host.pay_msgs, &host.pay_view, redraw)
                }
                None => host.pay_polls.pop_front().map(|(x, y, b, a, ms)| {
                    if let Some(f) = host.pay_shot.as_mut() {
                        f(&host.pay_msgs, &host.pay_view, None);
                    }
                    host.pay_answer = a;
                    host.pay_ms = ms;
                    (x, y, b)
                }),
            };
            let Some((x, y, b)) = p else { return Some(1) };
            if !host.world.is_null() {
                let w = &mut *host.world;
                w.mouse_x = x;
                w.mouse_y = y;
                w.mouse_buttons = b as _;
            }
            0
        }
        268 => {
            host.pay_redraw = true;
            0
        }
        // strategic.md "Not enough funds", Dismiss: op 269 puts the panel buffer (fresh payroll numbers) on the screen.
        269 => {
            host.pay_view.fields = true;
            host.pay_redraw = true;
            0
        }
        233 => {
            host.pay_view.tip = 0;
            0
        }
        234 => {
            host.pay_view.tip = arg(0);
            0
        }
        274 => {
            host.pay_view.blink = arg(0) != 0;
            if let Some(f) = host.pay_blink_shot.as_mut() {
                f(&host.pay_view);
            }
            0
        }
        276 => {
            host.pay_quote = arg(0);
            0
        }
        277 => {
            let (k, x0, p) = (arg(0), arg(1), host.pay_quote);
            let sub = !host.world.is_null() && crate::ctlpanel::sub_byte((*host.world).opt_step_text);
            host.pay_said.push((k, x0, p));
            if let Some(f) = host.pay_say.as_mut() {
                f(k, x0, p, sub);
            } else if let Some(f) = host.pay_end_shot.as_mut() {
                f(&host.pay_view, Some((k, x0, p, sub)));
            }
            0
        }
        278 => {
            host.pay_view.end = true;
            host.pay_view.fields = true;
            host.pay_view.tip = 0;
            0
        }
        279 => {
            host.pay_view.frames.push(arg(0));
            0
        }
        // Op 280 pd_panel_show: the end picture goes to the screen (the window polls once for it).
        280 => {
            if let Some(f) = host.pay_win.as_mut() {
                f(&host.pay_msgs, &host.pay_view, true);
            } else if let Some(f) = host.pay_end_shot.as_mut() {
                f(&host.pay_view, None);
            }
            0
        }
        270 => {
            host.pay_msgs.push(arg(0));
            0
        }
        271 => {
            host.pay_timer = Some(crate::dostime::now_ms());
            0
        }
        272 => match (host.pay_win.is_some(), host.pay_timer) {
            (true, Some(t)) => (crate::dostime::now_ms() - t).clamp(0, i32::MAX as i64) as u32,
            _ => host.pay_ms as u32,
        },
        275 => {
            let (k, family) = (arg(0), arg(1));
            let a = match host.pay_ask.as_mut() {
                Some(f) => f(k, family),
                None => {
                    if let Some(f) = host.pay_shot.as_mut() {
                        f(&host.pay_msgs, &host.pay_view, Some((k, family)));
                    }
                    host.pay_answer
                }
            };
            host.pay_asks.push((k, family, a));
            a as u32
        }
        _ => return None,
    })
}

/// The window's `balloon_win` for the merc balloons of a pass that wait now (not in the story scene).
/// # Safety: `host` is the session's PlayHost, in a host op.
unsafe fn balloon_show(host: *mut PlayHost) {
    if (*host).story_scene || !(*host).reactions.iter().any(|r| r.balloon.is_some()) {
        return;
    }
    if let Some(mut f) = (*host).balloon_win.take() {
        f();
        (*host).balloon_win = Some(f);
        (*host).balloon_shown = true;
    }
}

/// The window's `scene_win` in the story scene. # Safety: `host` is the session's PlayHost, in a host op.
unsafe fn scene_show(host: *mut PlayHost) {
    if (*host).story_scene {
        if let Some(mut f) = (*host).scene_win.take() {
            f();
            (*host).scene_win = Some(f);
        }
    }
}

/// World_load.md section 3 "The last quote": host ops 302 guide_say, 317 brenda_say and 38 santino_speech record
/// the quote they play in `lq` (speaker, category, row u16, quote u16).
pub fn npc_quote(lq: &mut [u8; 6], op: i32, a: &[i32]) {
    let arg = |i: usize| a.get(i).copied().unwrap_or(0);
    let mut put = |who: u8, cat: i32, quote: i32| {
        if matches!(cat as i8, 4 | 5 | 10) {
            return;
        }
        *lq = [who, cat as u8, 0, 0, quote as u8, (quote >> 8) as u8];
    };
    match op {
        302 => {
            let (q, c) = (arg(0), arg(1));
            put(0x40, c, q);
            if q < 0x28 && c == 6 {
                put(0x40, 6, q + 1);
            }
        }
        317 => {
            let q = arg(0);
            put(0x41, 7, q);
            if q < 10 {
                put(0x41, 7, q + 1);
            }
        }
        38 => put(0x42, 9, 0),
        _ => {}
    }
}

/// One merc quote of the morning: soldier k (profile p, face x x0) speaks about profile m. q = 6 is HatedCheck?
/// 0x44C10 (op 291), else AimComplainQuote 0x44D28 (op 289; `first` = quote 7 before it). `sub` = Subtitles.
#[derive(Clone, Copy, Debug)]
pub struct MercSay {
    pub k: i32,
    pub x0: i32,
    pub p: i32,
    pub m: i32,
    pub q: i32,
    pub first: bool,
    pub sub: bool,
}

/// The name voice before box quote q of a merc's morning quote, as (profile, member): PlayMercVoice 0x24D14 runs in
/// AimComplainQuote 0x44D28 and RaiseReason 0x44634 (member = the grudge profile), never for quotes 6 and 7, and
/// never in the raise answer (op 298, member -1) or HatedCheck? (op 291).
pub fn name_voice(s: &MercSay, q: i32) -> Option<(u8, u8)> {
    (q != 7 && q != 6 && s.m >= 0).then_some((s.p as u8, s.m as u8))
}

/// EncodeStringPlus1? 0x35810 in AimComplainQuote 0x44D28: g_textBuf gets the nickname, +1 encoded, and its end mark.
pub fn encode_name(text_buf: &mut [u8], nick: &[u8]) {
    let n = nick.len().min(text_buf.len().saturating_sub(1));
    for (d, &b) in text_buf.iter_mut().zip(&nick[..n]) {
        *d = b.wrapping_add(1);
    }
    text_buf[n] = 0;
}

/// The arguments of a merc's morning box from its host op, as (member, quote, first): op 289 complaint, op 291 hate quote,
/// op 296 raise_ask (reason quote, grudge g or -1), op 298 raise_answer (quote 13 for answer 2, else 14).
pub fn say_args(op: i32, a: &[i32]) -> (i32, i32, bool) {
    match op {
        289 => (a[2], a[1], a[3] != 0),
        296 => (a[2], a[1], false),
        298 => (-1, if a[1] == 2 { 13 } else { 14 }, false),
        _ => (a[1], 6, false),
    }
}

/// World_load.md "The last quote": speaker p (a profile < 64), category c (0 AIM, 1 DAY, 2 PST), quote q; the row
/// is 14 p, 33 p or 15 p by the category.
pub fn set_quote(lq: &mut [u8; 6], p: u8, c: u8, q: i32) {
    let row = (p as u16).wrapping_mul([14, 33, 15][c as usize]);
    *lq = [p, c, row as u8, (row >> 8) as u8, q as u8, (q >> 8) as u8];
}

/// World_load.md "The last quote", the merc quotes of the morning and of the merc screens: `prof(k)` = the profile
/// of soldier k.
pub fn merc_quote(lq: &mut [u8; 6], member: &mut u8, prof: impl Fn(i32) -> u8, op: i32, a: &[i32]) {
    let arg = |i: usize| a.get(i).copied().unwrap_or(0);
    match op {
        252 => set_quote(lq, arg(0) as u8, 1, 6),
        267 => set_quote(lq, arg(2) as u8, 2, 12),
        276 => set_quote(lq, arg(0) as u8, 2, 0),
        // AimComplainQuote 0x44D28: PlayQuote takes the speaker of the slot in AL, byte 0x22A1F0 the other merc's profile.
        289 => {
            set_quote(lq, prof(arg(0)), 2, arg(1));
            *member = arg(2) as u8;
        }
        291 => {
            set_quote(lq, prof(arg(0)), 2, 6);
            *member = arg(1) as u8;
        }
        295 => *member = arg(1) as u8,
        296 => set_quote(lq, prof(arg(0)), 2, arg(1)),
        298 => set_quote(lq, prof(arg(0)), 2, if arg(1) == 2 { 13 } else { 14 }),
        328 => set_quote(lq, arg(3) as u8, 0, 9),
        _ => {}
    }
}

/// cursor.md "Tile marker": Set, with the script line `marker kind K tile T`.
fn put_marker(st: &mut crate::cursor::CursorState, lines: &mut Vec<String>, kind: i32, tile: i32) {
    crate::cursor::marker_set(st, kind, tile);
    lines.push(format!("marker kind {:#X} tile {}", kind, tile));
}

/// panel.md "Cursor item back" (host op 85 cursor_back; key F10): an item that waits on the cursor under the panel
/// goes back into strip box `strip_box` of the selected merc, the cursor is emptied and the mouse is free again.
/// Returns whether it acted.
///
/// # Safety
/// `w` points to a bound world whose `soldiers` and `held` buffers are valid.
pub unsafe fn cursor_back_raw(w: *mut Ja1World) -> bool {
    cursor_back_host(w, std::ptr::null())
}

/// cursor_back_raw with the host. Port fix P22 (#2 row 64): when the shown strip entry holds an item, the held item goes to
/// the first empty entry in the strip order (a vest pocket only when the merc has it), else to the ground under the merc
/// (the inventory screen's put-down: b6 2). Without a host (`h` null) the ground step is left out (the old copy).
///
/// # Safety
/// As cursor_back_raw; `h` is null or a valid host.
pub unsafe fn cursor_back_host(w: *mut Ja1World, h: *const Ja1Host) -> bool {
    const SHOWN: [usize; 7] = [0, 7, 8, 9, 10, 11, 1];
    let wp = w;
    let w = &mut *w;
    // CursorItemBack? 0x53660: first 0x92DCC. In the enemies' turn the screen is off: the item stays on the cursor.
    if w.screen_drawn == 0 || w.cursor_drop == 0 || w.held.is_null() || w.soldiers.is_null() || *w.held == 0 || w.selected >= 0x30 {
        return false;
    }
    let mut e = SHOWN[(w.strip_box as usize).min(6)];
    let rec = w.soldiers.add(0x200 * w.selected as usize);
    let mut ground = false;
    if ja1logic::fanfix::port_on(22) && *rec.add(12 * e) != 0 {
        let r = std::slice::from_raw_parts(rec, 0x200);
        match SHOWN.iter().find(|&&k| r[12 * k] == 0 && !crate::invdraw::pocket_missing(r, k)) {
            Some(&k) => e = k,
            None => ground = !h.is_null(),
        }
    }
    if ground {
        let tile = i16::from_le_bytes([*rec.add(0x114), *rec.add(0x115)]) as i32;
        ja1_put_item(wp, h, tile, w.held, 2, 0, 0, 0);
    } else {
        std::ptr::copy_nonoverlapping(w.held, rec.add(12 * e), 12);
    }
    std::ptr::write_bytes(w.held, 0, 12);
    w.drop_answer = 0;
    w.cursor_drop = 0;
    eprintln!("cursor back slot {} {}", w.selected, if ground { "ground".to_string() } else { format!("entry {}", e) });
    true
}

/// RedrawMercPortrait? 0x336B4 (host op 139 with the soldier's slot): the answer (the portrait panel redraws, the cursor
/// tile 0x229D2C clears). The test is on the slot as a byte, as in the original.
pub fn stop_redraw(opt_onscreen: u8, slot: i32, selected: u8) -> (bool, bool) {
    let draw = opt_onscreen < 2 && (0..8).contains(&slot);
    (draw, draw && slot as u8 == selected)
}

unsafe extern "C" fn cb_call(ctx: *mut c_void, op: i32, args: *const i32, nargs: i32, out: *mut u8, out_size: i32) -> u32 {
    {
        let host = &mut *(ctx as *mut PlayHost);
        host.ev.host_op(host.world, op, args, nargs);
        host.ev.poll(host.world);
    }
    // Check aid (window_held_story): the first pass with an item on the cursor plays a quote of Brenda, as StoryFind does. A natural road
    // is not practical: a held item keeps the mouse in the strip, so no walk starts, and a strip press does not act in a walk.
    {
        static AID: std::sync::atomic::AtomicI8 = std::sync::atomic::AtomicI8::new(-1);
        use std::sync::atomic::Ordering::Relaxed;
        if AID.load(Relaxed) < 0 {
            AID.store(std::env::var_os("JA1_HELD_BRENDA").is_some() as i8, Relaxed);
        }
        let w = (*(ctx as *mut PlayHost)).world;
        if AID.load(Relaxed) == 1 && !w.is_null() && !(*w).held.is_null() && *(*w).held != 0 {
            AID.store(0, Relaxed);
            let q = [12];
            cb_call(ctx, 317, q.as_ptr(), 1, std::ptr::null_mut(), 0);
        }
    }
    // The shaded buttons: 0x10454 (op 144: EndTurn 0x121D8, RedrawTacticalPanel? 0x12CBC), 0x10398 (op 148).
    if op == 144 && !args.is_null() && nargs >= 1 {
        crate::strips::set_shade_last(*args);
    } else if op == 148 {
        crate::strips::set_shade_last(0);
    }
    // soldier_update.md "Queue a merc quote" (host op 103 quote_event: slot, event, delay, tile, focus).
    if op == 103 {
        let host = &mut *(ctx as *mut PlayHost);
        let w = host.world;
        if !w.is_null() && !args.is_null() && nargs >= 5 {
            let a = std::slice::from_raw_parts(args, 5);
            queue_quote(w, a, |w, slot| {
                let h = Ja1Host { ctx, random: cb_random, call: cb_call, host_ops: 0 };
                ja1_merc_quote(w, &h, slot);
            });
            eprintln!("quote queued {} event {:#X} delay {}", a[0] as u8, a[1] as u8, a[2] as u8 as i8);
        }
        return 0;
    }
    if matches!(op, 267 | 276 | 289 | 291 | 295 | 296 | 298 | 328) {
        let a = if args.is_null() || nargs <= 0 { &[][..] } else { std::slice::from_raw_parts(args, nargs as usize) };
        let host = &mut *(ctx as *mut PlayHost);
        let w = host.world;
        let prof = |k: i32| -> u8 {
            if (0..48).contains(&k) && !w.is_null() && !(*w).soldiers.is_null() {
                *(*w).soldiers.add(k as usize * 512 + 0x179)
            } else {
                0xFF
            }
        };
        merc_quote(&mut host.last_quote, &mut host.last_member, prof, op, a);
        if matches!(op, 289 | 291 | 296 | 298) && !w.is_null() && !(*w).soldiers.is_null() && (0..8).contains(&a[0]) {
            let rec = std::slice::from_raw_parts((*w).soldiers.add(a[0] as usize * 512), 512);
            let (m, q, first) = say_args(op, a);
            let say = MercSay { k: a[0], x0: rec[0x17A] as i32 * 0x25 + 0xB, p: rec[0x179] as i32, m, q, first,
                                sub: crate::ctlpanel::sub_byte((*w).opt_step_text) };
            if matches!(op, 289 | 296) && !(*w).text_buf.is_null() && !(*w).profiles.is_null() && ((0..64).contains(&m) || op == 296) {
                // 0x448A8 encodes its stack name: empty for the reasons 1..4.
                let nick: &[u8] = if m < 0 { &[] } else { std::slice::from_raw_parts((*w).profiles.add(m as usize * 0x5A), 10) };
                let nick = &nick[..nick.iter().position(|&b| b == 0).unwrap_or(nick.len())];
                encode_name(std::slice::from_raw_parts_mut((*w).text_buf, 128), nick);
            }
            host.merc_said.push(say);
            if let Some(f) = host.merc_say.as_mut() {
                f(say);
            }
        }
    }
    if matches!(op, 38 | 302 | 317) {
        let a = if args.is_null() || nargs <= 0 { &[][..] } else { std::slice::from_raw_parts(args, nargs as usize) };
        npc_quote(&mut (*(ctx as *mut PlayHost)).last_quote, op, a);
        let w = (*(ctx as *mut PlayHost)).world;
        if !w.is_null() && !(*w).held.is_null() {
            crate::pointer::speech_arrow(*(*w).held);
        }
        // speech.md "Quote categories": Santino's speech (ai.md Think step 3) = SAN quote 0 of speaker 0x42, voice
        // SAN\66 (the guide's and Brenda's quotes have no sound or text yet).
        if op == 38 {
            let host = &mut *(ctx as *mut PlayHost);
            host.sound.cat_quote("SAN", 0x42, 0);
            host.npc_said.push((9, 0));
        } else {
            // panel.md "The guide's and Brenda's balloons": the voices on the screen sound (these quotes come on
            // the island map and the day's screens), stderr `speech C Q TEXT` per part; guide_said keeps the call
            // for the face balloon.
            let host = &mut *(ctx as *mut PlayHost);
            if host.guide_say.is_some() {
                // The window's balloon at the call: the original waits inside the say, so the quote shows before
                // the guide offer (op 319) of the same day steps (#36 D).
                let under = host.quote_under.clone().map(|(mut s, p)| {
                    if let (Some(r), Some((iso, exe))) = (host.rating, host.sound.files()) {
                        crate::dayloop::rating_word_on(&iso, &exe, &mut s, r);
                    }
                    (s, p)
                });
                if let Some(f) = host.guide_say.as_mut() {
                    f(op, a, under.as_ref().map(|(s, p)| (s.as_slice(), p)));
                }
            } else {
                host.guide_said.push((op, a.to_vec()));
            }
            if !host.voice_later {
                let files = host.sound.files();
                for l in crate::sound::npc_say(files.as_ref().map(|(i, e)| (i.as_str(), e.as_str())), op, a) {
                    eprintln!("{}", l);
                }
            }
            if op == 317 {
                scene_show(ctx as *mut PlayHost);
            }
        }
    }
    // Op 344 cursor_to_soldier (port_api.md): with an item on the cursor, the soldier's inventory screen runs as
    // key I does. SPEC GAP (front-end): the screen runs after the pass, so the story-item test of the pick-up (combat.md
    // "Picking up a ground item" step 5) sees the cursor before the screen.
    if op == 344 {
        let host = &mut *(ctx as *mut PlayHost);
        let slot = if args.is_null() || nargs <= 0 { -1 } else { *args };
        let w = host.world;
        let held = if w.is_null() || (*w).held.is_null() { 0 } else { *(*w).held };
        eprintln!("pick up slot {} cursor {}", slot, held);
        if held != 0 && (0..8).contains(&slot) {
            host.pick_inv = Some(slot);
            host.pick_info = true;
        }
        return 0;
    }
    // Strip click ops (panel.md "Clicks on the bottom strip"; port_api.md): op 349 inv_open (slot) = the soldier's
    // inventory screen, run after the pass as key I; op 350 mouse_to (x, y, _) = the window warps the mouse;
    // op 351 wait_up (button) = the window waits for the release (a script's press is already up); op 352 weapon_cursor (slot) = the
    // cursor value from the hand item after the click.
    if op == 85 {
        let host = &mut *(ctx as *mut PlayHost);
        host.call(op);
        if !host.world.is_null() {
            let h = Ja1Host { ctx, random: cb_random, call: cb_call, host_ops: 0 };
            cursor_back_host(host.world, &h);
        }
        return 0;
    }
    if matches!(op, 146 | 147 | 157) {
        let host = &mut *(ctx as *mut PlayHost);
        host.call(op);
        let a = |i: usize| if !args.is_null() && nargs as usize > i { *args.add(i) } else { 0 };
        let now = host.turn_warp.unwrap_or(host.mouse_now);
        let mut saved = crate::pointer::mouse_saved();
        if let Some(p) = crate::pointer::turn_mouse(op, a(0), a(1), now, &mut saved) {
            eprintln!("turn mouse {} {}", p.0, p.1);
            host.turn_warp = Some(p);
            // MouseSetPos? 0x6820B also writes the game's place.
            crate::pointer::mouse_poll(p);
        }
        crate::pointer::mouse_save(saved);
        match op {
            147 => host.mouse_hold = true,
            157 => {
                host.mouse_hold = false;
                host.turn_back = host.balloon_shown || host.reactions.iter().any(|r| r.balloon.is_some());
                if host.turn_back {
                    eprintln!("op 157 behind a balloon");
                }
            }
            _ => {}
        }
        return 0;
    }
    if (349..=352).contains(&op) {
        let host = &mut *(ctx as *mut PlayHost);
        let a = |i: usize| if !args.is_null() && nargs as usize > i { *args.add(i) } else { 0 };
        match op {
            349 => {
                let slot = a(0);
                eprintln!("strip open slot {}", slot);
                if (0..8).contains(&slot) {
                    host.pick_inv = Some(slot);
                }
            }
            350 => host.mouse_to = Some((a(0), a(1))),
            351 => host.wait_up = a(0),
            352 => host.weapon_due = true,
            _ => {}
        }
        return 0;
    }
    // Op 345 disarm_ask (slot, kind; port_api.md): the merc's yes / no question with the face of S[0x179] and his
    // nickname S[0x16E] (panel.md "Merc-face question box", PANEL_P.PCC on the tactical screen): kind 0 "Attempt to
    // disarm explosive?", kind 1 "Remove blue flag?". The answer: the script's next `disarm` line, else the window's
    // box, else no. stderr `disarm ask SLOT KIND ANSWER`.
    if op == 345 {
        let host = &mut *(ctx as *mut PlayHost);
        let a = |i: usize| if !args.is_null() && nargs as usize > i { *args.add(i) } else { 0 };
        let (slot, kind) = (a(0), a(1));
        let w = host.world;
        let text: &[u8] = if kind == 1 { b"Remove blue flag?" } else { b"Attempt to disarm explosive?" };
        let who = if (0..48).contains(&slot) && !w.is_null() && !(*w).soldiers.is_null() {
            let rec = std::slice::from_raw_parts((*w).soldiers.add(slot as usize * 512), 512);
            let nick = &rec[0x16E..0x16E + 9];
            Some((rec[0x179], nick[..nick.iter().position(|&b| b == 0).unwrap_or(9)].to_vec()))
        } else {
            None
        };
        if let Some((p, n)) = who.as_ref() {
            host.disarm_last = Some((text.to_vec(), *p, n.clone()));
        }
        let yes = if let Some(y) = host.disarm_answers.pop_front() {
            y
        } else if let (Some(f), Some((p, n))) = (host.disarm_win.as_mut(), who.as_ref()) {
            f(text, *p, n)
        } else {
            false
        };
        eprintln!("disarm ask {} {} {}", slot, kind, yes as u8);
        return yes as u32;
    }
    // Op 297 raise_offer (slot, start; strategic.md "Salary raise" step 3): the salary box of SalaryDialog? 0x29074
    // with the face of S[0x179], the nickname S[0x16E] and the start offer (salary.rs). The answer: the box of the
    // script's next `raise` line, else the window's box, else NO (0). stderr `raise offer SLOT START RESULT`.
    if op == 297 {
        let host = &mut *(ctx as *mut PlayHost);
        let a = |i: usize| if !args.is_null() && nargs as usize > i { *args.add(i) } else { 0 };
        let (slot, start) = (a(0), a(1) as i16);
        let w = host.world;
        let who = if (0..48).contains(&slot) && !w.is_null() && !(*w).soldiers.is_null() {
            let rec = std::slice::from_raw_parts((*w).soldiers.add(slot as usize * 512), 512);
            let nick = &rec[0x16E..0x16E + 9];
            Some((rec[0x179], nick[..nick.iter().position(|&b| b == 0).unwrap_or(9)].to_vec()))
        } else {
            None
        };
        let r = if let Some(keys) = host.raise_presses.pop_front() {
            if let Some((p, n)) = who.as_ref() {
                host.raise_last = Some((*p, n.clone(), start, keys.clone()));
            }
            crate::salary::script_offer(start, &keys)
        } else if let (Some(f), Some((p, n))) = (host.raise_win.as_mut(), who.as_ref()) {
            f(*p, n, start)
        } else {
            0
        };
        eprintln!("raise offer {} {} {}", slot, start, r);
        return r as i32 as u32;
    }
    // Ops 180 hit_location, 182 interrupt_check (shooting.md batch 3) are library ops now (host_ops 0). Ops 181
    // knockdown_anim and 184-188 (merc_sees, squad_lost; death_show: above; 187 sector_won is a library op): nothing
    // (result 0, as every other op). Op 185 click_panel goes to `PlayHost::click_panel` (CursorModeSet 0x1C438).
    // Op 190 reloaded: 0 (no reload message). Ops 191 first_aid (0: no first aid),
    // 192 attack_cursor, 193 voc (click.md): nothing. Op 194 banner (turns.md Turn start): the host keeps the
    // banner's text and colour (`PlayHost::banner`); op 123 turn_banner is a library op now (Turn start).
    // sound.md "Requests": the sound ops start or load sounds (the result stays that of the host call).
    {
        let host = &mut *(ctx as *mut PlayHost);
        let a = if args.is_null() || nargs <= 0 { &[][..] } else { std::slice::from_raw_parts(args, nargs as usize) };
        let w = host.world;
        // CursorModeSet? 0x1C438 (op 185 click_panel of TacticalClick, SoldierDies and StoryFind): Play sets the
        // cursor mode and runs MoveCursorState on that slot after the call.
        if op == 185 && a.len() >= 2 {
            host.click_panel = Some((a[0], a[1]));
        }
        if op == 65 && !w.is_null() && !(*w).soldiers.is_null() {
            let all = std::slice::from_raw_parts_mut((*w).soldiers, 48 * 512);
            let (k, c) = (a.first().copied().unwrap_or(0), a.get(1).copied().unwrap_or(0));
            if let Some((fx, rate, vol)) = crate::sound::effect_slot(all, k, c) {
                eprintln!("merc sound {} {} effect {} rate {}", k, c, fx, rate);
                host.sound.op(60, &[fx, rate, vol, 1], &[]);
            }
        }
        if op == 210 && a.len() >= 5 && !w.is_null() {
            if let Some((m, d)) = host.sound.door_film(a[0], a[1], a[2], (*w).tileset) {
                host.door_films.push((m, a[3], a[4], d));
            }
        }
        // LoadPopups? 0x288DC (op 71, anim words 0x1D5 / 0x1D6): the picture grows over the merc's slot after the
        // pass. The original does nothing for a slot above 7 (only the player's team has the pictures).
        if op == 71 && a.len() >= 2 && (0..8).contains(&a[0]) && !w.is_null() {
            host.popups.push((a[0] as u8, a[1], (*w).selected));
        }
        // RedrawMercPortrait? 0x336B4 (StopAnim's step sound off, host op 139 with the soldier's slot): the portrait
        // panel of team slot < 8 redraws while opt_onscreen < 2 and the selected slot's stop clears the cursor tile
        // 0x229D2C = 0xC41 (the animated tile marker 0x229CAC keeps its life). The portrait is the front-end's
        // whole-frame draw; the cursor tile clear goes to Play (PlayHost::marker_clear).
        if op == 139 && a.len() >= 1 && !w.is_null() && stop_redraw((*w).opt_onscreen, a[0], (*w).selected).1 {
            host.marker_clear = true;
        }
        let soldiers = if w.is_null() || (*w).soldiers.is_null() { &[][..] } else { std::slice::from_raw_parts((*w).soldiers, 8 * 512) };
        host.sound.op(op, a, soldiers);
    }
    let r = (*(ctx as *mut PlayHost)).call(op);
    // Section 8 "End-of-day screen": ops 254..260, 206, 207, 37, 57 and 215 while the screen is open.
    {
        let a = if args.is_null() || nargs <= 0 { &[][..] } else { std::slice::from_raw_parts(args, nargs as usize) };
        if let Some(v) = crate::eod::host_op(&mut *(ctx as *mut PlayHost), op, a) {
            return v;
        }
        // The screen is not open: FadePalette? on the tactical picture (TacticalLoop 0x1136B, TraverseMove 0x3164E, StoryFind).
        if op == 215 {
            scene_show(ctx as *mut PlayHost);
        }
        let host = &mut *(ctx as *mut PlayHost);
        if let (215, true, [_, n, from, to, ..]) = (op, ja1logic::fanfix::on(28) && !host.battle_show.is_empty(), a) {
            // Fan fix F28: the battle show of the leave cleanup came first; it plays lit after the pass, then the fade.
            host.fade_held = Some((*n, *from, *to));
        } else if let (215, Some(f), [_, n, from, to, ..]) = (op, host.fade_win.as_mut(), a) {
            f(*n, *from, *to);
        }
    }
    // Section 9: during the morning op 221 inv_poll answers 1 (stop: no "Not enough funds" dialog yet); op 301
    // payroll_show: the payroll screen of the front-end; op 304 sunrise_show: the title shows "Day N"; op 303 day_ask: the answer of the
    // front-end (the window waits for Y / N, the script gives it).
    {
        let host = &mut *(ctx as *mut PlayHost);
        // panel.md "The mouse pointer": the morning's shapes (payroll 201, the funds dialog 0 then 201, 0 after the
        // stat changes).
        let shape = match op {
            301 | 278 => Some(201),
            226 if host.day_phase && host.inv.is_null() && (args.is_null() || nargs < 1 || *args == 0) => Some(0),
            287 => Some(0),
            _ => None,
        };
        if let (Some(s), Some(f)) = (shape, host.pointer.as_mut()) {
            f(s);
        }
        if let (Some((x, y)), Some(f)) = (crate::mouselimit::place_of_op(op), host.mouse_place.as_mut()) {
            f(x, y);
        }
        if let (Some(l), Some(f)) = (crate::mouselimit::of_op(op), host.mouse_limit.as_mut()) {
            f(l);
        }
        if host.day_phase && host.inv.is_null() {
            if let Some(v) = pay_op(host, op, args, nargs) {
                return v;
            }
        }
        if (282..=287).contains(&op) {
            let a = if args.is_null() || nargs <= 0 { &[][..] } else { std::slice::from_raw_parts(args, nargs as usize) };
            if let Some(l) = host.xp_op.as_mut().and_then(|f| f(op, a)) {
                eprintln!("{}", l);
                host.payroll_lines.push(l);
            }
            return 0;
        }
        if op == 301 {
            // strategic.md "The payroll screen": the screen before it is the replay screen at its end.
            let under = host.replay_last.as_ref().map(|(s, _)| s.as_slice()).or(host.replay_start.as_deref());
            if let Some(l) = host.payroll_show.as_mut().and_then(|f| f(under)) {
                eprintln!("{}", l);
                host.payroll_lines.push(l);
            }
            return 0;
        }
        if op == 304 {
            host.sunrise_shown = true;
            crate::sunrise::host_op(host);
            return 0;
        }
        // The strategic screen opening: op 305 merc_gone keeps the profile; ops 306 map_show and 302 guide_say:
        // nothing.
        // Story events (step 1 of the opening): op 307 quest_record (n) gives QUESTS.DAT bytes 250 (n − 5) .. + 249
        // (zero past the end of the file); op 308 stash_take (type, take) with the store records (port_api.md):
        // the first record of that type; take ≠ 0 takes it out.
        if op == 307 {
            let n = if !args.is_null() && nargs >= 1 { *args } else { 5 };
            if !out.is_null() && out_size > 0 {
                quest_record(host.quests.as_deref(), n, std::slice::from_raw_parts_mut(out, out_size as usize));
            }
            return 0;
        }
        // Op 206 pal_cycle outside the end-of-day screen: the tactical palette rotates (first .. last toward the
        // lower index); the window applies it to its palette.
        if op == 206 && !args.is_null() && nargs >= 2 {
            host.pal_rot.push((*args, *args.add(1)));
            return 0;
        }
        // The crater file (library op 336 explode_art): op 86 open_patch kind 5 opens DAT\EXPLODE.DAT, op 96 seek,
        // op 87 read (n) copies from the position, op 88 close. SPEC GAP: no file (or another kind): -1, reads give
        // 0 bytes.
        if matches!(op, 86 | 87 | 88 | 96) {
            let a0 = if !args.is_null() && nargs >= 1 { *args } else { 0 };
            match op {
                86 => {
                    host.explode_pos = (a0 == 5 && host.explode.is_some()).then_some(0);
                    return if host.explode_pos.is_some() { 0 } else { u32::MAX };
                }
                96 => {
                    if host.explode_pos.is_some() {
                        host.explode_pos = Some(a0.max(0) as usize);
                    }
                    return 0;
                }
                87 => {
                    let (Some(f), Some(pos)) = (host.explode.as_ref(), host.explode_pos) else { return 0 };
                    if out.is_null() || out_size <= 0 {
                        return 0;
                    }
                    let o = std::slice::from_raw_parts_mut(out, out_size as usize);
                    let k = (a0.max(0) as usize).min(o.len()).min(f.len().saturating_sub(pos));
                    o[..k].copy_from_slice(&f[pos..pos + k]);
                    host.explode_pos = Some(pos + k);
                    return k as u32;
                }
                _ => {
                    host.explode_pos = None;
                    return 0;
                }
            }
        }
        if op == 308 {
            let t = if !args.is_null() && nargs >= 1 { *args as u8 } else { 0 };
            let take = !args.is_null() && nargs >= 2 && *args.add(1) != 0;
            return stash_take(&mut host.stash, t, take);
        }
        // Op 264 stash_put (type, count, condition): the sorted insert of the item store (strategic.md).
        if op == 264 {
            let a = |i: usize| if !args.is_null() && nargs as usize > i { *args.add(i) as u8 } else { 0 };
            return stash_put(&mut host.stash, a(0), a(1), a(2));
        }
        // Host ops of the day clock (section 8): 309 day_bar keeps the bar length; 310 enemy_pick and 311
        // enemy_move, 312 guard_enter and 329 enemy_walk_in are library ops (strategic.md Enemy moves, Walk-ins);
        // 313 mode_banner, 316 refresh: the window draws anyway; 314 guards_msg: the status line text; 315
        // mouse_release: nothing to wait for.
        if op == 309 {
            let w = host.world;
            if !w.is_null() {
                host.day_bar = (*w).day_bar;
            }
            return 0;
        }
        if op == 314 {
            let count = if !args.is_null() && nargs >= 1 { *args } else { 0 };
            let number = if !args.is_null() && nargs >= 2 { *args.add(1) } else { 0 };
            host.status_msg(guards_text(count, number));
            return 0;
        }
        if matches!(op, 311 | 312 | 313 | 315 | 316) {
            return 0;
        }
        // The guide's advice: 317 brenda_say keeps the quote (no sound or text yet, as op 302 guide_say);
        // 318 panel_draw: the map draws its panel when it opens; 319 guide_offer answers guide_yes; 320 merc_join
        // calls ja1_merc_join (strategic.md "A merc joins the team"); the screens load the new face themselves.
        if op == 317 {
            let q = if !args.is_null() && nargs >= 1 { *args } else { 0 };
            host.brenda.push(q);
            eprintln!("brenda says quote {}", q);
            return 0;
        }
        if op == 318 {
            return 0;
        }
        // Op 292 corpse_ask (cost, can_pay; strategic.md "Corpse export" step 2): "Dish out $cost for corpse
        // export?" in the question box over the island map. The answer: the script's next `corpse` line, else the
        // window's box, else no. With can_pay = 0 the answer does not count (only the box's yes row differs). stderr
        // `corpse ask COST CAN_PAY ANSWER`.
        if op == 292 {
            let a = |i: usize| if !args.is_null() && nargs as usize > i { *args.add(i) } else { 0 };
            let (cost, can) = (a(0), a(1) != 0);
            let yes = if let Some(y) = host.corpse_answers.pop_front() {
                y
            } else if let Some(f) = host.corpse_win.as_mut() {
                f(cost, can)
            } else {
                false
            };
            host.corpse_last = Some((cost, can, yes));
            eprintln!("corpse ask {} {} {}", cost, can as u8, yes as u8);
            return yes as u32;
        }
        // Op 294 corpse_end: the grudges at the end of the corpse export (smoke corpse_grudge reads them).
        let w = host.world;
        if op == 294 && !w.is_null() && !(*w).profiles.is_null() {
            for p in 0..60usize {
                let m = *(*w).profiles.add(p * 0x5A + 0x3B);
                if m < 60 {
                    eprintln!("corpse grudge {} hates {}", p, m);
                }
            }
        }
        if op == 319 {
            let p = if !args.is_null() && nargs >= 1 { *args } else { 0 };
            host.offers.push(p);
            return match host.guide_ask.as_mut() { Some(f) => f(p) as u32, None => host.guide_yes as u32 };
        }
        // Op 105 stat_practice (slot, stat, n, flag): ja1_stat_practice on the world of the running call
        // (world_load.md section 8 "Stat practice"; combat.md Stat practice). Op 281 practice (team slot, stat,
        // amount, flag) of the morning duties is the same work with a 16-bit amount (strategic.md Morning duties).
        if op == 105 || op == 281 {
            let a = |i: usize| if !args.is_null() && nargs as usize > i { *args.add(i) } else { 0 };
            let w = host.world;
            if !w.is_null() {
                let h = Ja1Host { ctx, random: cb_random, call: cb_call, host_ops: 0 };
                ja1_stat_practice(w, &h, a(0), a(1), a(2), a(3));
            }
            return 0;
        }
        if op == 320 {
            let p = if !args.is_null() && nargs >= 1 { *args } else { 0 };
            let w = host.world;
            if !w.is_null() {
                let h = Ja1Host { ctx, random: cb_random, call: cb_call, host_ops: 0 };
                let k = ja1_merc_join(w, &h, p);
                eprintln!("profile {} joins the team in slot {}", p, k);
            }
            return 0;
        }
        // The day's verdict: op 330 home_lost_msg and op 331 rating_word (score) keep what came (the day loop shows
        // it).
        if op == 330 {
            host.home_lost = true;
            return 0;
        }
        if op == 331 {
            host.rating = Some(if !args.is_null() && nargs >= 1 { *args } else { 0 });
            return 0;
        }
        if op == 305 {
            host.gone.push(if !args.is_null() && nargs >= 1 { *args } else { 0 });
            return 0;
        }
        if op == 303 {
            let mut v = [0i32; 4];
            for (i, x) in v.iter_mut().enumerate() {
                if !args.is_null() && (i as i32) < nargs {
                    *x = *args.add(i);
                }
            }
            host.day_ask_args = Some(v);
            if let Some(f) = host.ask_show.as_mut() {
                f(v);
            }
            return match host.day_ask.as_mut() {
                Some(f) => f(v),
                None => 0,
            };
        }
    }
    // Section 8 "Inventory screen": the screen's ops while it is open.
    let inv = (*(ctx as *mut PlayHost)).inv;
    if !inv.is_null() {
        let a = if args.is_null() || nargs <= 0 { &[][..] } else { std::slice::from_raw_parts(args, nargs as usize) };
        let h = Ja1Host { ctx, random: cb_random, call: cb_call, host_ops: 0 };
        let w = (*(ctx as *mut PlayHost)).world;
        if !w.is_null() {
            if let Some(v) = (*inv).op(w, &h, op, a) {
                return v;
            }
        }
    }
    // Ops 55 ui_refresh (force) and 199 cursor_state (slot): Hide path marks (force; 0 for op 199) on the world of
    // the running call (select.md). Ops 195..200: the front-end's selection state (`SelectUi`).
    if op == 55 || op == 199 {
        let w = (*(ctx as *mut PlayHost)).world;
        if !w.is_null() {
            let force = if op == 55 && !args.is_null() && nargs >= 1 { *args } else { 0 };
            ja1_hide_path_marks(w, std::ptr::null(), force);
        }
    }
    // soldier_update.md "Merc sighting": op 184 keeps its bytes for the loop head; op 54 merc_message is a Merc
    // message with an empty text.
    if op == 184 {
        let a = if args.is_null() || nargs <= 0 { &[][..] } else { std::slice::from_raw_parts(args, nargs as usize) };
        crate::sighting::merc_sees(&mut (*(ctx as *mut PlayHost)).sight, a);
        return 0;
    }
    // soldier_update.md "Team quote": op 202 won_quote (q, range, 1).
    if op == 202 && !args.is_null() && nargs >= 2 {
        let host = &mut *(ctx as *mut PlayHost);
        let h = Ja1Host { ctx, random: cb_random, call: cb_call, host_ops: 0 };
        let (q, range) = (*args as u8 as i8, *args.add(1));
        let w = host.world;
        if w.is_null() || (*w).soldiers.is_null() {
            return 0;
        }
        let recs = std::slice::from_raw_parts((*w).soldiers, 8 * 512);
        crate::sighting::team_quote(recs, range, |n| cb_random(ctx, n), |k| {
            if host.log {
                eprintln!("team quote {} {}", k, q);
            }
            let mut cry = None;
            let m = crate::sighting::merc_message(w, &h, k, q, 0x40, "", &mut host.msg_box, &mut cry);
            host.spotted_cry(cry);
            if let Some(m) = m {
                host.say(&m);
                host.reactions.push(m);
            }
        });
        balloon_show(ctx as *mut PlayHost);
        return 0;
    }
    // Check aid (window_story_right): the mercs of the scene stand close together, so a merc's view move shows only from a far view.
    if op == 54 && (*(ctx as *mut PlayHost)).story_scene && std::env::var_os("JA1_STORY_FAR").is_some() {
        static DONE: std::sync::atomic::AtomicBool = std::sync::atomic::AtomicBool::new(false);
        let w = (*(ctx as *mut PlayHost)).world;
        if !w.is_null() && !DONE.swap(true, std::sync::atomic::Ordering::Relaxed) {
            ((*w).view_x, (*w).view_y) = (0, 0);
        }
    }
    if op == 54 && !args.is_null() && nargs >= 3 {
        let host = &mut *(ctx as *mut PlayHost);
        let h = Ja1Host { ctx, random: cb_random, call: cb_call, host_ops: 0 };
        let (k, how, near) = (*args as u8, *args.add(1) as u8 as i8, *args.add(2));
        let mut cry = None;
        let r = crate::sighting::merc_message(host.world, &h, k, how, near, "", &mut host.msg_box, &mut cry);
        host.spotted_cry(cry);
        if let Some(r) = r {
            host.say(&r);
            host.reactions.push(r);
        }
        balloon_show(ctx as *mut PlayHost);
        // StoryFind 0x6179C waits inside each quote: a right click on one balloon keeps the view for the next quote.
        scene_show(ctx as *mut PlayHost);
        return 0;
    }
    // turns.md step 19 "Sector line": op 167 speech_ready = the count of live mercs (0x410C4); op 168 sector_speech
    // (slot, sector) = the native guide's sector line (0x297D4).
    if op == 164 || op == 165 {
        let host = &mut *(ctx as *mut PlayHost);
        host.wipes.push((op - 164) as u8);
        if host.log {
            eprintln!("wipe {}", if op == 164 { "close" } else { "open" });
        }
        return 0;
    }
    if op == 167 {
        return crate::sighting::live_mercs((*(ctx as *mut PlayHost)).world) as u32;
    }
    if op == 168 && !args.is_null() && nargs >= 2 {
        let host = &mut *(ctx as *mut PlayHost);
        let h = Ja1Host { ctx, random: cb_random, call: cb_call, host_ops: 0 };
        if let Some(r) = crate::sighting::sector_line(host.world, &h, *args, *args.add(1)) {
            if host.log {
                eprintln!("sector line {} {}", r.slot, r.how);
            }
            host.say(&r);
            host.reactions.push(r);
        }
        return 0;
    }
    // Check aid (window_balloon_new_turn): a merc quote written at the start of NewTurn?, so that its step 6 plays it, and the
    // MouseRestorePos2? behind SelectMerc? stands behind the balloon. Value "new": the aid of op 99 below has no effect then.
    // An enemy is selected at that point only behind the enemies' turn, not at the combat start of the first pass.
    if op == 158 && std::env::var("JA1_TURN_QUOTE").map_or(false, |v| v == "new") {
        let host = &*(ctx as *mut PlayHost);
        if !host.world.is_null() && (*host.world).combat != 0 && (*host.world).selected >= 8 && (*host.world).merc_to_show < 8 {
            let slot = (*host.world).merc_to_show;
            eprintln!("new turn quote {}", slot);
            (*host.world).quote_slot = slot;
            (*host.world).quote_event = 1;
            (*host.world).quote_delay = 0;
        }
    }
    // Op 99 focus (slot, flag): the view goes to the soldier (world_load.md section 8 "Focus").
    if op == 99 && !args.is_null() && nargs >= 2 {
        let a = std::slice::from_raw_parts(args, 2);
        // Check aid (window_balloon_back): a merc quote for the merc that NewTurn? selects next (focus, then SelectMerc?), the same
        // write that the original got at 0x12A17 (live on #31, build/probe/live_turnquote2.py).
        let host = &*(ctx as *mut PlayHost);
        if host.mouse_hold && a[0] < 8 && !host.world.is_null() && std::env::var("JA1_TURN_QUOTE").map_or(false, |v| v != "new") {
            eprintln!("turn quote {} interrupter {}", a[0], (*host.world).interrupter);
            (*host.world).quote_slot = a[0] as u8;
            (*host.world).quote_event = 1;
            (*host.world).quote_delay = 0;
        }
        crate::focus::focus_world((*(ctx as *mut PlayHost)).world, a[0], a[1]);
        return 0;
    }
    // Op 357 slide_view (tile): fan fix F15 slides the view to a down enemy that bled (EnemiesTurn? step 6).
    if op == 357 && !args.is_null() && nargs >= 1 {
        crate::focus::slide_world((*(ctx as *mut PlayHost)).world, *args);
        return 0;
    }
    // Op 205 tile_anim (tile, kind): the front-end's animation timers start a water ripple (kind 0, Update end
    // step 2) or a fire (kind 1, effect_pass).
    if op == 205 && !args.is_null() && nargs >= 2 {
        let host = &mut *(ctx as *mut PlayHost);
        let a = std::slice::from_raw_parts(args, 2);
        let w = host.world;
        if !w.is_null() && !(*w).tiles.is_null() {
            let tiles = std::slice::from_raw_parts_mut((*w).tiles, crate::world::size::TILES);
            if a[1] == 1 {
                // effect_pass: the fire of a kind-3 record (DrawProjectilePass? 0x1769B StartTileAnim table 0x9562E).
                if host.anims.start_table(a[0], FIRE_FRAMES, FIRE_TABLE_COUNT, tiles).is_some() {
                    eprintln!("fire {}", a[0]);
                }
            } else {
                host.anims.start(a[0], a[1], tiles);
            }
        }
        return 0;
    }
    // Op 183 loss_msg: the stat-loss message starts its timer (`loss_flush` gives it to the world, loss_pass shows it).
    if op == 183 && !args.is_null() && nargs >= 3 {
        (*(ctx as *mut PlayHost)).loss_new.push((*args, *args.add(1), *args.add(2)));
    }
    // Op 57 merc_text (slot, text, ms): a message box "NAME TEXT" for ms (panel.md "Tactical messages").
    if op == 57 && !args.is_null() && nargs >= 3 {
        let host = &mut *(ctx as *mut PlayHost);
        let w = host.world;
        let k = *args as u8 as usize;
        if host.inv.is_null() && !w.is_null() && !(*w).soldiers.is_null() && k < 48 {
            let rec = std::slice::from_raw_parts((*w).soldiers.add(k * 512), 512);
            if let Some(t) = merc_text(rec, *args.add(1)) {
                host.msg_box = Some((t, *args.add(2) as u32));
            }
        }
    }
    // Op 186 death_show (slot, quiet) (shooting.md "Merc dies"): a loud death shows "NAME is dead!" for 800 ms
    // (the sound is in sound.rs).
    if op == 186 && !args.is_null() && nargs >= 2 {
        let host = &mut *(ctx as *mut PlayHost);
        let w = host.world;
        let (k, quiet) = (*args as u8 as usize, *args.add(1));
        if !w.is_null() && !(*w).soldiers.is_null() && k < 8 {
            let rec = std::slice::from_raw_parts((*w).soldiers.add(k * 512), 512);
            eprintln!("death show {} quiet {}", k, quiet);
            if quiet == 0 {
                let text = if host.inv.is_null() { dead_text(rec) } else { None };
                host.morph.push((k as u8, text));
            }
        }
    }
    // Op 125 message (text): the text in a message box (port_api.md op 125, panel.md "Tactical messages").
    if op == 125 && !args.is_null() && nargs >= 1 {
        if let Some(t) = message_text(*args) {
            (*(ctx as *mut PlayHost)).status_msg(t.to_string());
        }
    }
    // Op 129 show_msg (port_api.md op 129 step_text): a subtitle in the message box for 500 ms: 0..5 the loud
    // step-noise texts, 6 the trigger's "click" (ShowMessage 0x27B98 of PlayEmptyClick 0x1ABAC, Subtitles on).
    if op == 129 && !args.is_null() && nargs >= 1 {
        const TEXTS: [&str; 7] = ["splash", "SPLASH!", "clunk", "CLUNK!", "snap", "SNAP!", "click"];
        if let Some(t) = TEXTS.get(*args as usize) {
            (*(ctx as *mut PlayHost)).msg_box = Some((t.to_string(), 500));
        }
    }
    // Ops 203 enemies_msg (MsgEnemiesNearby 0x1B0EC sets the flag 0x22A0CF after the box) and 326 guard_sees_msg:
    // a message box (panel.md "Tactical messages").
    if op == 203 {
        (*(ctx as *mut PlayHost)).msg_box = Some(nearby_box());
    }
    // Op 204 music (theme, stop first) and op 216 music_stop: music.md "Theme state" (theme -1 = stop).
    if op == 204 && !args.is_null() && nargs >= 2 {
        let (t, f) = (*args, *args.add(1));
        crate::music::with(|m| m.play(t, f != 0));
    }
    if op == 216 {
        crate::music::with(|m| m.stop_theme());
    }
    // Op 188 squad_lost: music.md "Who plays what", theme 6.
    if op == 188 {
        crate::music::with(|m| m.play(6, false));
        let ph = ctx as *mut PlayHost;
        if !(*ph).squad_play.is_null() && !(*ph).world.is_null() {
            squad_lost_now(ph);
        } else {
            (*ph).squad_lost = true;
        }
    }
    if op == 326 {
        (*(ctx as *mut PlayHost)).msg_box = Some(("Guard sees a new enemy!".to_string(), 25000));
    }
    // Op 348 walter_say (q) (combat.md "walter_say"): the listening device's clip on the speech channel and, with
    // Subtitles on, its caption in a message box until the clip ends; stderr `walter Q TEXT`.
    // Op 72 stop_walk (slot, flag): StopWalk 0x104C8, the busy display of the selected merc's walk in a fight.
    if op == 72 && !args.is_null() && nargs >= 2 {
        let host = &mut *(ctx as *mut PlayHost);
        let w = host.world;
        if !w.is_null() {
            crate::strips::stop_walk(*args as u8, *args.add(1), (*w).combat, (*w).selected, &mut (*w).view_hold, &mut (*w).turn_shown);
        }
        return 0;
    }
    if op == 348 && !args.is_null() && nargs >= 1 {
        let host = &mut *(ctx as *mut PlayHost);
        // ClearMarksBeforeQuote? 0x11418, called by PlayWalterQuote 0x3C574 before the quote's wait loop: path
        // marks off, AP number and cursor tile none, over_map / mouse wobble 0.
        let w = host.world;
        if !w.is_null() {
            unsafe { ja1_hide_path_marks(w, std::ptr::null(), 0) };
        }
        host.cursor_reset = true;
        let q = *args;
        let files = host.sound.files();
        let (clip, text) = files.map(|(iso, exe)| walter_files(&iso, &exe, q)).unwrap_or_default();
        // PlayWalterQuote 0x3C574: with Sound FX off SpeechStream plays nothing, and the caption box gets its 2000 ms.
        let clip = clip.filter(|_| !host.sound.fx_off);
        let ms = clip.as_ref().map(|d| d.len() as u64 * 1000 / 0x2B11).unwrap_or(0);
        if let Some(d) = clip {
            host.sound.speech_raw(&format!("wp{}", q), 0x2B11, 0x7F, &d);
        }
        if host.log {
            eprintln!("walter {} {}", q, text);
        }
        if crate::iteminfo::subtitles() && !text.is_empty() {
            host.msg_box = Some((text, ms.max(2000) as u32));
        }
        return 0;
    }
    // strategic.md "The day replay": op 89 replay_event adds to the day's list; ops 353..356 of `ja1_day_replay`
    // answer from it (no Escape: the window plays the sounds after the library call, a key ends them).
    if op == 89 || (353..=356).contains(&op) {
        let host = &mut *(ctx as *mut PlayHost);
        let a = if args.is_null() { &[][..] } else { std::slice::from_raw_parts(args, nargs.max(0) as usize) };
        let arg = |i: usize| a.get(i).copied().unwrap_or(0);
        return match op {
            89 => {
                record_replay_event(&mut host.replay_events, arg(0), arg(1), arg(2));
                0
            }
            353 => replay_count(&host.replay_events),
            354 => {
                let e = host.replay_events.get(arg(0).max(0) as usize).copied().unwrap_or([0; 3]);
                e[0] as u32 | (e[1] as u32) << 8 | (e[2] as u32) << 16
            }
            355 => {
                if host.log {
                    eprintln!("replay anim {} {} {} {}", arg(0), arg(1), arg(2), arg(3));
                }
                let r = replay_table(host);
                match (host.replay_pic.as_mut(), r) {
                    (Some(p), Some(r)) => {
                        p.carry = (*host.world).replay_carry;
                        p.anim(arg(0), [arg(1), arg(2), arg(3), arg(4), arg(5)], &r);
                        host.replay_show.append(&mut p.out);
                    }
                    _ => host.replay_show.extend(replay_steps(arg(0), [arg(1), arg(2), arg(3)])),
                }
                0
            }
            _ => {
                // strategic.md "The replay pictures": op 356 replay_draw (k, a).
                if let (Some(r), true) = (replay_table(host), host.replay_pic.is_some()) {
                    let p = host.replay_pic.as_mut().unwrap();
                    p.draw(arg(0), arg(1), &r);
                    host.replay_show.append(&mut p.out);
                }
                0
            }
        };
    }
    // Ops 332 battle_ui, 333 battle_wait, 334 battle_box (strategic.md "The battle show"): steps of the show, played
    // after the pass. SPEC GAP (front-end): the waits come after the pass, so op 333 always answers "not skipped".
    if (332..=334).contains(&op) && !args.is_null() && nargs >= 1 {
        let host = &mut *(ctx as *mut PlayHost);
        let a = std::slice::from_raw_parts(args, nargs as usize);
        let arg = |i: usize| a.get(i).copied().unwrap_or(0);
        let ev = match (op, arg(0)) {
            (332, 2) => Some(BattleEv::Sound("ATTACK.8".to_string())),
            (332, 3) => Some(BattleEv::Sound(format!("DIE{}.8", arg(1)))),
            (332, 4) => Some(BattleEv::Sound(format!("MINE{}.8", arg(1)))),
            (332, 5) => Some(BattleEv::WaitSound),
            (332, 6) => Some(BattleEv::Stop),
            (332, 9) => Some(BattleEv::ClearKey),
            (332, _) => None,
            (333, ms) => Some(BattleEv::Wait(ms.max(0) as u32)),
            _ => Some(BattleEv::Box(arg(0), arg(1) as u8, arg(2) as u8, arg(3) as u8, arg(4))),
        };
        if let Some(ev) = ev {
            if host.log {
                match &ev {
                    BattleEv::Sound(n) => eprintln!("battle sound {}", n),
                    BattleEv::Box(s, e, g, t, k) => eprintln!("battle box {} {} {} {} {}", s, e, g, t, k),
                    _ => {}
                }
            }
            host.battle_show.push(ev);
        }
        return 0;
    }
    // Op 335 battle_msg (t, n) (strategic.md Sector battles): t = 0 the mine line on the status line, else the
    // result as a 1500 ms message box; a play script writes the line `battle TEXT`.
    if op == 335 && !args.is_null() && nargs >= 2 {
        let host = &mut *(ctx as *mut PlayHost);
        let t = battle_text(*args, *args.add(1));
        if host.log {
            eprintln!("battle {}", t);
        }
        if *args == 0 {
            host.status = Some(t);
        } else {
            host.msg_box = Some((t, 1500));
        }
        return 0;
    }
    // Op 197 sees_msg (slot): sees_slot = the soldier's +0x17A and the "sees enemies" line (panel.md).
    if op == 197 && !args.is_null() && nargs >= 1 {
        let host = &mut *(ctx as *mut PlayHost);
        let w = host.world;
        let k = *args as u8 as usize;
        if !w.is_null() && !(*w).soldiers.is_null() && k < 48 {
            let rec = std::slice::from_raw_parts((*w).soldiers.add(k * 512), 512);
            (*w).sees_slot = rec[0x17A];
            host.sees_text = Some(sees_text(rec));
        }
    }
    if op == 130 && !args.is_null() && nargs >= 2 {
        let a = std::slice::from_raw_parts(args, 2);
        (*(ctx as *mut PlayHost)).marker = Some((a[0], a[1]));
    }
    if (195..=200).contains(&op) {
        let a = if args.is_null() || nargs <= 0 { &[][..] } else { std::slice::from_raw_parts(args, nargs as usize) };
        return (*(ctx as *mut PlayHost)).select.op(op, a);
    }
    if op == 213 && !args.is_null() && nargs >= 4 {
        // Op 213 hears_msg (ai.md Hearing a noise): the host keeps the text line (`PlayHost::noise_msg`).
        let a = std::slice::from_raw_parts(args, 4);
        // MsgHearsNoise? 0x72BE0..0x72C3B: sprintf, then StatusMessage? 0x27D24 (a box of 25 s).
        let host = &mut *(ctx as *mut PlayHost);
        let w = host.world;
        let k = a[0] as u8 as usize;
        if !w.is_null() && !(*w).soldiers.is_null() && k < 48 {
            let rec = std::slice::from_raw_parts((*w).soldiers.add(k * 512), 512);
            let t = hears_text(rec, a[1], a[2], a[3]);
            host.noise_msg = Some(t.clone());
            host.status_msg(t);
        }
    }
    if op == 217 {
        // Op 217 end_of_day (soldier_update.md Leaving the sector): no end-of-day screen; the title says so.
        // Ops 216 music_stop, 218 resolve_battle, 220 timer_off: nothing (result 0).
        (*(ctx as *mut PlayHost)).end_of_day = true;
    }
    if op == 219 {
        // Op 219 save_tmp (section 8): the tmp file of sector cur_sector from the tiles and the ground items.
        let host = &mut *(ctx as *mut PlayHost);
        let w = host.world;
        if !w.is_null() && !(*w).tiles.is_null() && !(*w).ground_items.is_null() {
            let tiles = std::slice::from_raw_parts((*w).tiles, crate::world::size::TILES);
            let ground = std::slice::from_raw_parts((*w).ground_items, 0x960);
            host.saved_tmp = Some(((*w).cur_sector, crate::host::make_tmp_file(tiles, ground)));
        }
    }
    if op == 194 && !args.is_null() && nargs >= 1 {
        (*(ctx as *mut PlayHost)).banner = Banner::from_kind(*args);
    }
    if op == 178 && !args.is_null() && nargs >= 7 {
        // Op 178 puff (combat.md The explosion effect record): the front-end's effect record in fx_anims.
        let w = (*(ctx as *mut PlayHost)).world;
        if !w.is_null() && !(*w).fx_anims.is_null() {
            let fx = std::slice::from_raw_parts_mut((*w).fx_anims as *mut u8, FX_ANIM_COUNT * FX_REC);
            return puff(fx, std::slice::from_raw_parts(args, 7)) as u32;
        }
    }
    if op == 189 && !args.is_null() && nargs >= 4 {
        // Op 189 aim_dir (shooting.md batch 5): the direction by arc tangent; the out word (soldier +0x10A) is 0.
        let a = std::slice::from_raw_parts(args, 4);
        if !out.is_null() && out_size >= 2 {
            out.write(0);
            out.add(1).write(0);
        }
        return crate::host::aim_dir(a[0], a[1], a[2], a[3]);
    }
    // Debug aid: JA1_ZERO_OPS=1 writes `zero op N` to stderr for each op that gets the generic answer (and a few
    // handled ops that fall through: 194, 197, 203, 213, 217, 219, 326).
    if std::env::var_os("JA1_ZERO_OPS").is_some() {
        eprintln!("zero op {}", op);
    }
    r
}

/// TileDistance in the library.
pub(crate) fn tile_dist(a: i32, b: i32) -> i32 {
    unsafe { ja1_tile_dist(a, b) }
}

extern "C" {
    fn ja1_merc_quote(w: *mut Ja1World, h: *const Ja1Host, slot: i32);
    fn ja1_tile_dist(a: i32, b: i32) -> i32;
    fn ja1_move_order(w: *mut Ja1World, h: *const Ja1Host, slot: i32, tile: i32, marker: i32, sneak: i32, byte177: i32,
                      alt: i32, squad: i32) -> i32;
    fn ja1_frame(w: *mut Ja1World, h: *const Ja1Host) -> i32;
    fn ja1_story_find(w: *mut Ja1World, h: *const Ja1Host);
    fn ja1_merc_join(w: *mut Ja1World, h: *const Ja1Host, profile: i32) -> i32;
    fn ja1_stat_practice(w: *mut Ja1World, h: *const Ja1Host, slot: i32, stat: i32, n: i32, flag: i32);
    fn ja1_effect_hit(w: *mut Ja1World, h: *const Ja1Host, tile: i32, kind: i32, power: i32);
    fn ja1_effect_pass(w: *mut Ja1World, h: *const Ja1Host, limit: i32);
    fn ja1_explode(w: *mut Ja1World, h: *const Ja1Host, thrower: i32, tile: i32, kind: i32, hit: i32);
    fn ja1_put_item(w: *mut Ja1World, h: *const Ja1Host, tile: i32, item: *const u8, b6: i32, exact: i32, note: i32,
                    near: i32);
    fn ja1_update_end(w: *mut Ja1World, h: *const Ja1Host) -> i32;
    fn ja1_fuses(w: *mut Ja1World, h: *const Ja1Host);
    fn ja1_loop_head(w: *mut Ja1World, h: *const Ja1Host) -> i32;
    fn ja1_leave_tests(w: *mut Ja1World, h: *const Ja1Host) -> i32;
    fn ja1_hit_location(w: *mut Ja1World, h: *const Ja1Host, slot: i32, height: i32, damage: i32) -> i32;
    fn ja1_leave_path(w: *mut Ja1World, h: *const Ja1Host);
    fn ja1_leave_sector(w: *mut Ja1World, h: *const Ja1Host, flag: i32);
    fn ja1_first_merc(w: *mut Ja1World, h: *const Ja1Host) -> i32;
    fn ja1_click(w: *mut Ja1World, h: *const Ja1Host, slot: i32, tile: i32, item: i32) -> i32;
    fn ja1_strip_click(w: *mut Ja1World, h: *const Ja1Host, slot: i32, mode: i32);
    fn ja1_select_merc(w: *mut Ja1World, h: *const Ja1Host, slot: i32, force: i32, quote: i32) -> i32;
    fn ja1_tick(w: *mut Ja1World, h: *const Ja1Host);
    fn ja1_end_turn(w: *mut Ja1World, h: *const Ja1Host);
    fn ja1_new_turn(w: *mut Ja1World, h: *const Ja1Host, focus: i32) -> i32;
    fn ja1_hide_path_marks(w: *mut Ja1World, h: *const Ja1Host, force: i32);
    fn ja1_escape_stop(w: *mut Ja1World, h: *const Ja1Host);
    fn ja1_next_merc(w: *mut Ja1World, h: *const Ja1Host, slot: i32);
    fn ja1_next_enemy(w: *mut Ja1World, h: *const Ja1Host, slot: i32);
    fn ja1_swap_places(w: *mut Ja1World, h: *const Ja1Host, slot: i32, tile: i32);
    fn ja1_reserve_ap(w: *mut Ja1World, h: *const Ja1Host, slot: i32);
    fn ja1_traverse(w: *mut Ja1World, h: *const Ja1Host, slot: i32, dir: i32, out: *mut i16) -> i32;
    fn ja1_traverse_edge(w: *mut Ja1World, h: *const Ja1Host, slot: i32, dir: i32) -> i32;
    fn ja1_traverse_out(w: *mut Ja1World, h: *const Ja1Host, dest: i32);
    fn ja1_traverse_in(w: *mut Ja1World, h: *const Ja1Host, theme: i32, playing: i32) -> i32;
    fn ja1_time_compress(w: *mut Ja1World, h: *const Ja1Host, on: i32);
    fn ja1_take_damage(w: *mut Ja1World, h: *const Ja1Host, slot: i32, height: i32, damage: i32, kind: i32, attacker: i32,
                       tile: i32);
    fn ja1_break_object(w: *mut Ja1World, h: *const Ja1Host, tile: i32, object: i32, snd: i32);
    fn ja1_teleport(w: *mut Ja1World, h: *const Ja1Host, slot: i32, tile: i32);
    fn ja1_debug_heal(w: *mut Ja1World, h: *const Ja1Host, slot: i32);
    fn ja1_replace_soldier(w: *mut Ja1World, h: *const Ja1Host, slot: i32);
    fn ja1_turn_sightings(w: *mut Ja1World, h: *const Ja1Host, both: i32);
    fn ja1_next_turn(w: *mut Ja1World, h: *const Ja1Host, mode: i32);
    fn ja1_morning(w: *mut Ja1World, h: *const Ja1Host) -> i32;
    fn ja1_clock_steps() -> u64;
    fn ja1_frame_draw() -> i32;
    fn ja1_sunrise(w: *mut Ja1World, h: *const Ja1Host);
    fn ja1_strategic_open(w: *mut Ja1World, h: *const Ja1Host);
    fn ja1_map_click(w: *mut Ja1World, h: *const Ja1Host, sector: i32, mode: i32, buttons: i32, warned: *mut u8) -> i32;
    fn ja1_map_start(w: *const Ja1World, button: i32, hinted: *mut u8) -> i32;
    fn ja1_day_start(w: *mut Ja1World, h: *const Ja1Host) -> i32;
    fn ja1_day_verdict(w: *mut Ja1World, h: *const Ja1Host) -> i32;
    fn ja1_day_bonus(w: *mut Ja1World, h: *const Ja1Host);
    fn ja1_day_end(w: *mut Ja1World, h: *const Ja1Host) -> i32;
    fn ja1_day_end_accounts(w: *mut Ja1World, h: *const Ja1Host);
    fn ja1_day_replay(w: *mut Ja1World, h: *const Ja1Host);
    fn ja1_win_vars(w: *mut Ja1World, h: *const Ja1Host, out: *mut i16);
    fn ja1_performance_rating(w: *mut Ja1World, h: *const Ja1Host) -> i32;
    fn ja1_inventory_screen(w: *mut Ja1World, h: *const Ja1Host, slot: i32, redraw: i32) -> i32;
    fn ja1_guard_close(w: *mut Ja1World, h: *const Ja1Host, g: *mut crate::mapui::guard::GuardScreen);
}

/// One library call session on `w`: the C world view is made at the start and stored back at the end.
pub struct Session<'a> {
    w: &'a mut World,
    cw: Ja1World,
    h: Ja1Host,
    /// cursor.md "ALL-move": the next press's walk moves the squad.
    pub squad: i32,
}

impl<'a> Session<'a> {
    pub fn new(w: &'a mut World, host: &'a mut PlayHost) -> Session<'a> {
        let cw = Ja1World::view(w);
        host.stash = std::mem::take(&mut w.stash_records);
        host.last_quote = w.last_quote;
        host.last_member = w.quote_member;
        // host_ops 0: every library op runs in the library.
        let h = Ja1Host { ctx: host as *mut PlayHost as *mut c_void, random: cb_random, call: cb_call, host_ops: 0 };
        Session { w, cw, h, squad: 0 }
    }

    /// CursorModeSet? 0x1C438 of slot `slot` with mode `mode`.
    pub fn cursor_mode_set(&mut self, st: &mut crate::cursor::CursorState, slot: i32, mode: i32) {
        self.bind();
        crate::cursor::click_panel(st, &mut self.cw, &self.h, slot, mode);
    }

    /// `cursor::move_cursor_state` for record `slot` (MoveCursorState 0x1C464).
    pub fn cursor_state_move(&mut self, st: &mut crate::cursor::CursorState, slot: usize) {
        self.bind();
        crate::cursor::move_cursor_state(st, &mut self.cw, &self.h, slot);
    }

    /// Points the host at this session's world (the address of `cw` can change when the session moves).
    fn bind(&mut self) {
        // SAFETY: `ctx` is the session's PlayHost; no library call runs now.
        unsafe { (*(self.h.ctx as *mut PlayHost)).world = &mut self.cw };
    }

    /// One cursor.md "Hover" step at tile `t`.
    pub fn hover(&mut self, st: &mut crate::cursor::CursorState, hs: &mut crate::cursor::HoverState, gv_t3: &[u8; 256],
                 t: i32, shift: bool, alt: bool) {
        self.bind();
        let mut hv = crate::cursor::Hover { w: &mut self.cw, h: &self.h, st, hs, gv_t3, shift, alt,
                                                  confirm_move: self.w.opt_confirm_move != 0, fast_fire: fast_fire_on(self.w), drag: 0, squad: 0 };
        hv.step(t);
    }

    /// TacticalKeyHandler 0x13490, the mouse off the map (cursor::off_map): the unlight, the path marks and the
    /// cursor tile go, over_map = 0, once after the mouse left the map.
    pub fn off_map(&mut self, st: &mut crate::cursor::CursorState, hs: &mut crate::cursor::HoverState) {
        self.bind();
        crate::cursor::off_map(st, hs, &mut self.cw, &self.h);
    }

    /// panel.md "The overview mouse step" at the overview point of tile `t` (mode 0); see `Hover::overview`.
    #[allow(clippy::too_many_arguments)]
    pub fn overview_mouse(&mut self, st: &mut crate::cursor::CursorState, hs: &mut crate::cursor::HoverState, gv_t3: &[u8; 256],
                          ov: &mut crate::viewmode::OvMouse, t: i32, buttons: i32, shift: bool, alt: bool, drag: i32)
                          -> (crate::cursor::ClickOut, bool) {
        self.bind();
        // OverviewMouse 0x24645: the overview step counts as the mouse on the map (0x14308 reads it when the mouse leaves).
        st.over_map = 1;
        let mut hv = crate::cursor::Hover { w: &mut self.cw, h: &self.h, st, hs, gv_t3, shift, alt,
                                                  confirm_move: self.w.opt_confirm_move != 0, fast_fire: fast_fire_on(self.w), drag, squad: self.squad };
        let (x, y) = crate::viewmode::overview_point(t);
        hv.overview(ov, x, y, buttons)
    }

    /// `Hover::press_walks`: does a left press at `t` start the edge drag?
    #[allow(clippy::too_many_arguments)]
    pub fn press_walks(&mut self, st: &mut crate::cursor::CursorState, hs: &mut crate::cursor::HoverState, gv_t3: &[u8; 256],
                       t: i32, shift: bool, alt: bool) -> bool {
        self.bind();
        let drag = 0;
        let hv = crate::cursor::Hover { w: &mut self.cw, h: &self.h, st, hs, gv_t3, shift, alt,
                                                  confirm_move: self.w.opt_confirm_move != 0, fast_fire: fast_fire_on(self.w), drag, squad: self.squad };
        hv.press_walks(t)
    }

    /// `viewmode::press_walks` for overview tile `t`.
    pub fn overview_press_walks(&mut self, t: i32) -> bool {
        let o = if (0..3136).contains(&t) { self.w.tiles[8 * t as usize + 5] & 0x7F } else { 0x40 };
        let k = self.w.selected as usize;
        let hp = if k < 48 { self.w.soldiers[k * 512 + 0x183] as i8 as i32 } else { 0 };
        // SPEC GAP: occupants 0x30..0x3F / 0x80.. have no record here; read their visible byte as 0 (walk, as 1.13b).
        let vis = if (o as usize) < 48 { self.w.soldiers[o as usize * 512 + 0x9C] as i8 as i32 } else { 0 };
        crate::viewmode::press_walks(o, hp, vis)
    }

    /// One cursor.md "Clicks" press at tile `t` (left, or right when `right`: the hold starts, mouse y = `y`).
    #[allow(clippy::too_many_arguments)]
    pub fn press(&mut self, st: &mut crate::cursor::CursorState, hs: &mut crate::cursor::HoverState, gv_t3: &[u8; 256],
                 t: i32, shift: bool, alt: bool, right: bool, y: i32, drag: i32) -> crate::cursor::ClickOut {
        self.bind();
        let mut hv = crate::cursor::Hover { w: &mut self.cw, h: &self.h, st, hs, gv_t3, shift, alt,
                                                  confirm_move: self.w.opt_confirm_move != 0, fast_fire: fast_fire_on(self.w), drag, squad: self.squad };
        if right { hv.right(t, y) } else { hv.left(t) }
    }

    /// The answer to an open aim or confirm (cursor.md "Clicks").
    pub fn answer(&mut self, st: &mut crate::cursor::CursorState, hs: &mut crate::cursor::HoverState, gv_t3: &[u8; 256],
                  wait: crate::cursor::ClickWait, a: crate::cursor::Answer) -> crate::cursor::ClickOut {
        self.bind();
        let mut hv = crate::cursor::Hover { w: &mut self.cw, h: &self.h, st, hs, gv_t3, shift: false, alt: false,
                                                  confirm_move: self.w.opt_confirm_move != 0, fast_fire: fast_fire_on(self.w), drag: 0, squad: 0 };
        hv.answer(wait, a)
    }

    /// strategic.md "The guard screen", Close: the guards set off (op 82 merc_dies runs in the library).
    pub fn guard_close(&mut self, g: &mut crate::mapui::guard::GuardScreen) {
        self.bind();
        // SAFETY: as in move_order; `g` is the open screen's state.
        unsafe { ja1_guard_close(&mut self.cw, &self.h, g) }
    }

    /// `ja1_move_order (slot, tile, marker, sneak, byte177, alt, squad)`.
    #[allow(clippy::too_many_arguments)]
    /// `ja1_explode` (combat.md "Explosion, exact steps").
    /// Script line `mine TILE COND`: a set mine (item 0x11, count 1, condition COND, byte 6 0xFF) on TILE, as the
    /// sector maps lay them (sector_start.md Put item, exact 1).
    pub fn put_mine(&mut self, tile: i32, cond: i32) {
        self.put_ground(tile, 0x11, cond, 0xFF)
    }

    /// One item (type, count 1, condition) put on the tile with byte 6 = b6.
    pub fn put_ground(&mut self, tile: i32, ty: i32, cond: i32, b6: i32) {
        let mut item = [0u8; 12];
        item[0] = ty as u8;
        item[1] = 1;
        item[2] = cond as u8;
        // SAFETY: as in move_order.
        self.bind();
        unsafe { ja1_put_item(&mut self.cw, &self.h, tile, item.as_ptr(), b6, 1, 0, 0) }
    }

    pub fn explode(&mut self, thrower: i32, tile: i32, kind: i32, hit: i32) {
        // SAFETY: as in move_order.
        self.bind();
        unsafe { ja1_explode(&mut self.cw, &self.h, thrower, tile, kind, hit) }
    }

    pub fn move_order(&mut self, slot: i32, tile: i32, marker: i32, sneak: i32, byte177: i32, alt: i32, squad: i32) -> i32 {
        // SAFETY: `cw` points into the buffers of `w` (borrowed mutably for the session's life); the host
        // callbacks do not touch the world.
        self.bind();
        unsafe { ja1_move_order(&mut self.cw, &self.h, slot, tile, marker, sneak, byte177, alt, squad) }
    }

    /// The library's Timer tick, then (under the same gates as its steps 1-2: round_lock2 = 0 and round_lock = 0)
    /// the front-end's animation timers.
    pub fn tick(&mut self) {
        // Check aid (window_mark_balloon): 150 ticks after the first hover marks, a merc quote of the selected merc comes over them.
        if self.cw.marks_shown != 0 && self.cw.marks_n > 0 && std::env::var_os("JA1_MARK_QUOTE").is_some() {
            static WAIT: std::sync::atomic::AtomicI32 = std::sync::atomic::AtomicI32::new(150);
            let left = WAIT.fetch_sub(1, std::sync::atomic::Ordering::Relaxed);
            if left == 1 {
                eprintln!("mark quote {} marks {}", self.cw.selected, self.cw.marks_n);
                self.cw.quote_slot = self.cw.selected as u8;
                self.cw.quote_event = 1;
                self.cw.quote_delay = 0;
            }
        }
        let gates_open = timer_gates_open(self.cw.round_lock2, self.cw.round_lock);
        // SAFETY: as in move_order.
        self.bind();
        unsafe { ja1_tick(&mut self.cw, &self.h) }
        if gates_open {
            self.anims_step();
        }
    }

    /// The front-end's animation timers of one tick (under the gates of the Timer tick's steps 1-2).
    fn anims_tick(&mut self) {
        if timer_gates_open(self.cw.round_lock2, self.cw.round_lock) {
            self.anims_step();
        }
    }

    fn anims_step(&mut self) {
        {
            // SAFETY: `ctx` is the session's PlayHost (no library call runs now); `tiles` and `fx_anims` point
            // into `w`'s buffers (fx_anims made from the mutable buffer), which keep their sizes.
            unsafe {
                let host = &mut *(self.h.ctx as *mut PlayHost);
                let tiles = std::slice::from_raw_parts_mut(self.cw.tiles, self.w.tiles.len());
                let fx = std::slice::from_raw_parts_mut(self.cw.fx_anims as *mut u8, self.w.fx_anims.len());
                host.anims.step(tiles, fx);
                self.w.tile_frames = host.anims.sprite_frames();
            }
        }
    }

    /// world_load.md section 8 "Frames", Santino's step: after his quote (santino_spoke = 1) he uses tile 291 by
    /// hand once (santino_spoke = 2, op 99 focus (0x28, 1), `ja1_click (0x28, 291, 0)`).
    /// soldier_update.md "Fuse tick": when fuse_beep is set, effect 0x2C plays (rate 11025, volume 0x68, once) and
    /// fuse_beep = 0 (TacticalLoop 0x10BD1, before One frame).
    pub fn fuse_beep_step(&mut self) {
        if self.cw.fuse_beep == 0 {
            return;
        }
        self.cw.fuse_beep = 0;
        // SAFETY: ctx is the PlayHost of this session.
        let host = unsafe { &mut *(self.h.ctx as *mut PlayHost) };
        eprintln!("fuse beep");
        host.sound.op(60, &[0x2C, 0x2B11, 0x68, 1], &[]);
    }

    /// sound.md "Click sound" (the original's 0x5C880): effect 0 (IN) at 11025, volume 6, once.
    pub fn click_sound(&mut self) {
        // SAFETY: ctx is the PlayHost of this session.
        let host = unsafe { &mut *(self.h.ctx as *mut PlayHost) };
        eprintln!("click sound");
        host.sound.op(60, &[0, 0x2B11, 6, 1], &[]);
    }

    /// soldier_update.md "Merc view": TacticalLoop 0x10BFB, before the Santino step. A merc saw a story tile
    /// (story_finder < 0x40) and has hp >= 15: the story-object find scene (`ja1_story_find`), then story_finder =
    /// 0x40. Returns the duty-7 merc when the scene ran (the caller clears the lit face slot 0x284F0E for it).
    pub fn story_step(&mut self) -> Option<u8> {
        let k = self.cw.story_finder as usize;
        if !finder_fit(&self.w.soldiers, k) {
            return None;
        }
        self.bind();
        // SAFETY: as in move_order; `ctx` is the session's PlayHost.
        unsafe {
            (*(self.h.ctx as *mut PlayHost)).story_scene = true;
            ja1_story_find(&mut self.cw, &self.h);
            (*(self.h.ctx as *mut PlayHost)).story_scene = false;
        }
        self.cw.story_finder = 0x40;
        eprintln!("story find {} duty7 {} view {} {}", k, self.cw.duty7_merc, self.cw.view_x, self.cw.view_y);
        Some(self.cw.duty7_merc)
    }

    pub fn santino_step(&mut self) {
        if self.cw.santino_spoke != 1 {
            return;
        }
        self.cw.santino_spoke = 2;
        self.bind();
        // SAFETY: cw is this session's world, its soldier buffer holds 48 records.
        unsafe { crate::focus::focus_world(&mut self.cw, 0x28, 1) };
        self.click(0x28, 0x123, 0);
    }

    pub fn frame(&mut self) -> i32 {
        // SAFETY: as in move_order.
        self.bind();
        unsafe { ja1_frame(&mut self.cw, &self.h) }
    }

    /// The explosion effect records of this frame: `ja1_effect_pass` (blast.rs effect_pass, DrawProjectilePass?
    /// 0x17548; view mode 0 = OverviewRedraw's copy with limit 0x16). The hit, the fire (op 205 kind 1) and the
    /// fire sound run in the library; the log names the records that hit at this level (not the ones a merc death
    /// re-hits inside the pass, #70 row 30).
    pub fn effect_hits(&mut self) {
        let limit = if self.w.opt_onscreen == 0 { 0x16 } else { 0x13 };
        // SAFETY: fx_anims points into `w`'s buffer, which keeps its size; no library call runs while it is read.
        let hits: Vec<(i32, i32, i32)> = unsafe {
            let fx = std::slice::from_raw_parts(self.cw.fx_anims, self.w.fx_anims.len());
            (1..FX_ANIM_COUNT).filter_map(|k| effect_hit_due(fx, k, limit)).collect()
        };
        for (t, kind, power) in hits {
            eprintln!("effect_hit {} {} {}", t, kind, power);
        }
        // SAFETY: as in move_order.
        self.bind();
        unsafe { ja1_effect_pass(&mut self.cw, &self.h, limit as i32) }
        // SAFETY: as in tick (no library call runs now).
        unsafe {
            let host = &mut *(self.h.ctx as *mut PlayHost);
            self.w.tile_frames = host.anims.sprite_frames();
        }
    }

    /// `ja1_update_end` (soldier_update.md "Update end").
    pub fn update_end(&mut self) -> i32 {
        // SAFETY: as in move_order.
        self.bind();
        unsafe { ja1_update_end(&mut self.cw, &self.h) }
    }

    /// `ja1_fuses` (soldier_update.md "Fuses").
    pub fn fuses(&mut self) {
        // SAFETY: as in move_order.
        self.bind();
        unsafe { ja1_fuses(&mut self.cw, &self.h) }
    }

    /// `ja1_loop_head` (soldier_update.md "Loop head").
    pub fn loop_head(&mut self) -> i32 {
        // SAFETY: as in move_order.
        self.bind();
        // TacticalLoop 0x10800: the stat-loss message comes first in each pass.
        unsafe { loss_flush(&mut *(self.h.ctx as *mut PlayHost), self.w) };
        match loss_pass(self.w) {
            Some(LossShow::Box(t, ms)) => unsafe { (*(self.h.ctx as *mut PlayHost)).msg_box = Some((t, ms)) },
            Some(LossShow::Quote) => unsafe {
                let a = [0x1C, 0xE, 0];
                cb_call(self.h.ctx, 202, a.as_ptr(), 3, std::ptr::null_mut(), 0);
            },
            None => {}
        }
        // soldier_update.md "Merc sighting": the front-end's part, before the library's loop head.
        unsafe {
            let host = &mut *(self.h.ctx as *mut PlayHost);
            let mut cry = None;
            let r = crate::sighting::loop_head(&mut host.sight, &mut self.cw, &self.h, &mut host.msg_box, &mut cry);
            host.spotted_cry(cry);
            if let Some(r) = r {
                host.say(&r);
                host.reactions.push(r);
            }
        }
        unsafe { ja1_loop_head(&mut self.cw, &self.h) }
    }

    /// `ja1_hit_location` (script line `hit K HEIGHT DAMAGE`).
    pub fn hit_location(&mut self, k: i32, height: i32, damage: i32) -> i32 {
        // SAFETY: as in move_order.
        self.bind();
        unsafe { ja1_hit_location(&mut self.cw, &self.h, k, height, damage) }
    }

    /// TacticalTimer? 0x5EFDE: g_lossTimer goes down to 0.
    fn loss_tick(&mut self) {
        loss_timer_tick(&mut self.w.loss_timer);
    }

    /// `ja1_leave_tests` (soldier_update.md "Leave tests").
    /// Host op 344 asked for the inventory screen in this pass and the screen did not run yet.
    pub fn pick_screen_due(&self) -> bool {
        // SAFETY: `ctx` is the session's PlayHost; no library call runs now.
        unsafe { (*(self.h.ctx as *mut PlayHost)).pick_inv.is_some() }
    }

    pub fn leave_tests(&mut self) -> i32 {
        // SAFETY: as in move_order.
        self.bind();
        unsafe { ja1_leave_tests(&mut self.cw, &self.h) }
    }

    /// `ja1_leave_path` (soldier_update.md "Leaving the sector").
    pub fn leave_path(&mut self) {
        // SAFETY: as in move_order.
        self.bind();
        unsafe { ja1_leave_path(&mut self.cw, &self.h) }
    }

    /// `ja1_leave_sector (flag)` (soldier_update.md "Leaving the sector", host op 214).
    pub fn leave_sector(&mut self, flag: i32) {
        // SAFETY: as in move_order.
        self.bind();
        unsafe { ja1_leave_sector(&mut self.cw, &self.h, flag) }
    }

    /// `ja1_first_merc` (turns.md "First merc"; 0x40 when no merc is on duty).
    pub fn first_merc(&mut self) -> i32 {
        // SAFETY: as in move_order.
        self.bind();
        unsafe { ja1_first_merc(&mut self.cw, &self.h) }
    }

    /// `ja1_morning` (strategic.md The morning).
    pub fn morning(&mut self) -> i32 {
        // SAFETY: as in move_order.
        self.bind();
        unsafe { ja1_morning(&mut self.cw, &self.h) }
    }

    /// `ja1_day_replay` (strategic.md The day replay): the day's events again on the replay table.
    pub fn day_replay(&mut self) {
        // SAFETY: as in move_order.
        self.bind();
        unsafe { ja1_day_replay(&mut self.cw, &self.h) }
    }

    /// `ja1_day_verdict` (strategic.md The day's verdict): 0 = the game goes on; 2, 3, 4 = the game ends.
    pub fn day_verdict(&mut self) -> i32 {
        // SAFETY: as in move_order.
        self.bind();
        unsafe { ja1_day_verdict(&mut self.cw, &self.h) }
    }

    /// `ja1_win_vars` (ending.md "Game end", win sequence step 1): script variables v0..v3 of WINGAME.SAB.
    pub fn win_vars(&mut self) -> [i16; 4] {
        let mut v = [0i16; 4];
        // SAFETY: as in move_order; `v` holds the 4 values written.
        self.bind();
        unsafe { ja1_win_vars(&mut self.cw, &self.h, v.as_mut_ptr()) };
        v
    }

    /// `ja1_performance_rating` (ending.md "The performance rating"): the score 0..100.
    pub fn performance_rating(&mut self) -> i32 {
        // SAFETY: as in move_order.
        self.bind();
        unsafe { ja1_performance_rating(&mut self.cw, &self.h) }
    }

    /// `ja1_day_bonus` (strategic.md The day's verdict), only after a verdict 0.
    pub fn day_bonus(&mut self) {
        // SAFETY: as in move_order.
        self.bind();
        unsafe { ja1_day_bonus(&mut self.cw, &self.h) }
    }

    /// `ja1_day_end_accounts` (strategic.md "The day-end accounts").
    pub fn day_end_accounts(&mut self) {
        // SAFETY: as in move_order.
        self.bind();
        unsafe { ja1_day_end_accounts(&mut self.cw, &self.h) }
    }

    /// `ja1_day_end` (strategic.md "The day-end screen", day-end steps 1 and 2): 0, or 3 = the game ends.
    pub fn day_end(&mut self) -> i32 {
        // SAFETY: as in move_order.
        self.bind();
        unsafe { ja1_day_end(&mut self.cw, &self.h) }
    }

    /// `ja1_sunrise` (strategic.md Sunrise).
    pub fn sunrise(&mut self) {
        // music.md "Who plays what": the sunrise screen stops the music.
        crate::music::with(|m| m.stop_theme());
        // SAFETY: as in move_order.
        self.bind();
        unsafe { ja1_sunrise(&mut self.cw, &self.h) }
    }

    /// `ja1_strategic_open` (strategic.md Strategic screen opening).
    pub fn strategic_open(&mut self) {
        // SAFETY: as in move_order.
        self.bind();
        // SAFETY: as in bind: `h.ctx` is the PlayHost of this session.
        let host = unsafe { &mut *(self.h.ctx as *mut PlayHost) };
        if let Some((base, _)) = host.clock_seed {
            host.rng.seed = base.wrapping_add(0x7F00_0000).wrapping_add(self.cw.day as u32);
        }
        unsafe { ja1_strategic_open(&mut self.cw, &self.h) }
    }

    /// `ja1_map_click (sector, mode, buttons, warned)` (strategic.md Sector click on the island map).
    pub fn map_click(&mut self, sector: i32, mode: i32, buttons: i32, warned: &mut u8) -> i32 {
        // SAFETY: as in move_order; `warned` is a valid byte.
        self.bind();
        unsafe { ja1_map_click(&mut self.cw, &self.h, sector, mode, buttons, warned) }
    }

    /// `ja1_map_start (button, hinted)` (button 1 = the start button, 0 = the Enter key).
    pub fn map_start(&mut self, button: i32, hinted: &mut u8) -> i32 {
        // SAFETY: as in move_order.
        unsafe { ja1_map_start(&self.cw, button, hinted) }
    }

    /// `ja1_day_start` (strategic.md Day start).
    pub fn day_start(&mut self) -> i32 {
        // SAFETY: as in move_order.
        self.bind();
        unsafe { ja1_day_start(&mut self.cw, &self.h) }
    }

    /// `ja1_click (slot, tile, item)` (click.md).
    pub fn click(&mut self, slot: i32, tile: i32, item: i32) -> i32 {
        // SAFETY: as in move_order.
        self.bind();
        unsafe { ja1_click(&mut self.cw, &self.h, slot, tile, item) }
    }

    /// `ja1_select_merc (slot, force, quote)` (select.md).
    pub fn select_merc(&mut self, slot: i32, force: i32, quote: i32) -> i32 {
        // SAFETY: as in move_order.
        self.bind();
        unsafe { ja1_select_merc(&mut self.cw, &self.h, slot, force, quote) }
    }

    /// The cursor of the last op 200 cursor_select, once.
    /// cursor.md "Tile marker": the op 130 of the last library calls (kind, tile).
    pub fn take_marker(&mut self) -> Option<(i32, i32)> {
        // SAFETY: h.ctx is the PlayHost this session was made with (Session::new).
        unsafe { (*(self.h.ctx as *mut PlayHost)).marker.take() }
    }

    /// ClearMarksBeforeQuote? 0x11418: host op 348 walter_say came since the last call.
    pub fn take_cursor_reset(&mut self) -> bool {
        // SAFETY: h.ctx is the PlayHost this session was made with (Session::new).
        unsafe { std::mem::take(&mut (*(self.h.ctx as *mut PlayHost)).cursor_reset) }
    }

    /// RedrawMercPortrait? 0x336B4: host op 139 step_sound_stop cleared the cursor tile 0x229D2C since the last call.
    pub fn take_marker_clear(&mut self) -> bool {
        // SAFETY: h.ctx is the PlayHost this session was made with (Session::new).
        unsafe { std::mem::take(&mut (*(self.h.ctx as *mut PlayHost)).marker_clear) }
    }

    /// CursorModeSet? 0x1C438: host op 185 click_panel (slot, mode) came since the last call.
    pub fn take_click_panel(&mut self) -> Option<(i32, i32)> {
        // SAFETY: h.ctx is the PlayHost this session was made with (Session::new).
        unsafe { (*(self.h.ctx as *mut PlayHost)).click_panel.take() }
    }

    /// MoveMercTo 0x1C7A8: a library TacticalClick with the detector 0x4D left its move order (Ja1World detector_slot /
    /// detector_tile) since the last call.
    pub fn take_detector_wait(&mut self) -> Option<(i32, i32)> {
        let t = std::mem::replace(&mut self.cw.detector_tile, -1);
        (t >= 0).then_some((self.cw.detector_slot as i32, t as i32))
    }

    /// The move order of `take_detector_wait` for the selected merc: the confirm wait (option on) or the move.
    pub fn detector_pass(&mut self, st: &mut crate::cursor::CursorState, hs: &mut crate::cursor::HoverState, gv_t3: &[u8; 256],
                         t: i32) -> crate::cursor::ClickOut {
        self.bind();
        let mut hv = crate::cursor::Hover { w: &mut self.cw, h: &self.h, st, hs, gv_t3, shift: false, alt: false,
                                                  confirm_move: self.w.opt_confirm_move != 0, fast_fire: fast_fire_on(self.w), drag: 0, squad: 0 };
        hv.detector_pass(t)
    }

    /// Applies host op 185 click_panel to the front-end cursor state (CursorModeSet 0x1C438 / MoveCursorState
    /// 0x1C464).
    pub fn apply_click_panel(&mut self, st: &mut crate::cursor::CursorState, slot: i32, mode: i32) {
        crate::cursor::click_panel(st, &mut self.cw, &self.h, slot, mode);
    }

    /// The timer gates (soldier_update.md "Timer tick"): (round_lock2 = 0, round_lock2 = 0 and round_lock = 0).
    pub fn round_gates(&self) -> (bool, bool) {
        (self.cw.round_lock2 == 0, self.cw.round_lock2 == 0 && self.cw.round_lock == 0)
    }

    pub fn take_cursor_select(&mut self) -> Option<(i32, i32)> {
        // SAFETY: h.ctx is the PlayHost this session was made with (Session::new).
        unsafe { (*(self.h.ctx as *mut PlayHost)).select.cursor.take() }
    }

    /// select.md Select soldier step 3: op 200 cursor_select puts the cursor in walk mode (1, 0x8E), also when the
    /// merc was chosen by key or the hand emptied in weapon mode. An interrupt's Select merc raises it during a
    /// pass's ticks; the value must land before the next input. Applied after a click that starts an aim it replaced
    /// the aim cursor and left the aim open (#36 A: the walk reticule with a shot still attached).
    fn cursor_select_apply(&mut self, cursor: &mut crate::cursor::CursorState) {
        if let Some((m, v)) = self.take_cursor_select() {
            cursor.mode = m as u8;
            cursor.value = v as u8;
        }
    }

    /// Section 8 "Choose a merc": key 1..8 or a portrait click = merc k = 0..7. Returns whether `ja1_select_merc`
    /// ran. TacticalKeyHandler 0x136BD: SelectMerc (k, force = Alt held (0x284F20), quote = 1), so a merc off the
    /// view is only focused by the first press and selected by the second.
    pub fn choose_merc(&mut self, k: usize, alt: bool) -> bool {
        if !can_choose(&self.w.soldiers, k) {
            return false;
        }
        self.select_merc(k as i32, alt as i32, 1);
        true
    }

    /// OnScreen 0x1FC34 of merc k (crate::cursor::slot_on_screen).
    pub fn slot_on_screen(&mut self, k: u8) -> bool {
        self.bind();
        crate::cursor::slot_on_screen(&mut self.cw, &self.h, k)
    }

    /// One pass of the mouse on the merc slots with no button (crate::slotmouse). `pt` = the mouse point off the map,
    /// None = on the map.
    pub fn slot_mouse(&mut self, sm: &mut crate::slotmouse::SlotMouse, pt: Option<(i32, i32)>) {
        if !crate::slotmouse::pass_runs(self.w.update.screen_drawn) {
            return;
        }
        match pt {
            Some((x, y)) => {
                let sel = self.w.selected;
                let on = self.slot_on_screen(sel);
                let mode = self.w.opt_onscreen;
                sm.hover_pass(x, y, mode, &mut self.w.soldiers, sel, on);
            }
            None => sm.on_map(&mut Vec::new()),
        }
    }

    /// SlotMouse 0x119CC: a left click on the face that shows "cont": the selected merc goes on with its order.
    pub fn cont(&mut self, st: &mut crate::cursor::CursorState, hs: &mut crate::cursor::HoverState, gv_t3: &[u8; 256])
        -> crate::cursor::ClickOut {
        self.bind();
        let mut hv = crate::cursor::Hover { w: &mut self.cw, h: &self.h, st, hs, gv_t3, shift: false, alt: false,
                                                  confirm_move: self.w.opt_confirm_move != 0, fast_fire: fast_fire_on(self.w), drag: 0, squad: 0 };
        hv.cont()
    }

    /// Sets the key fields before a move order (section 8 "Click to walk").
    /// cursor.md "Debug keys (level 4)", Shift + left click: the inventory screen of soldier `o` after the pass,
    /// then its debug pages.
    pub fn debug_open(&mut self, o: usize) {
        // SAFETY: `ctx` is the session's PlayHost; no library call runs now.
        let host = unsafe { &mut *(self.h.ctx as *mut PlayHost) };
        host.pick_inv = Some(o as i32);
        host.debug_pages = Some(o);
    }

    pub fn set_keys(&mut self, shift: bool, alt: bool) {
        self.cw.key_shift = shift as u8;
        self.cw.key_alt = alt as u8;
    }

    /// panel.md "Cursor item back".
    pub fn cursor_back(&mut self) -> bool {
        self.bind();
        // SAFETY: as in move_order.
        unsafe { cursor_back_host(&mut self.cw, &self.h) }
    }

    /// select.md "Hide path marks" (`force`).
    pub fn hide_path_marks(&mut self, force: i32) {
        self.bind();
        // SAFETY: as in move_order.
        unsafe { ja1_hide_path_marks(&mut self.cw, &self.h, force) };
    }

    /// Section 8 "End turn": `ja1_end_turn`, only while combat, screen_drawn and turn_shown are all ≠ 0.
    /// Returns whether it ran.
    pub fn end_turn(&mut self) -> bool {
        if self.cw.combat == 0 || self.cw.screen_drawn == 0 || self.cw.turn_shown == 0 {
            return false;
        }
        // SAFETY: as in move_order.
        self.bind();
        unsafe { ja1_end_turn(&mut self.cw, &self.h) };
        true
    }

    /// Ctrl+F (0x13BA3): with selected >= 8, NewTurn (1), then turn_shown = 1.
    pub fn force_turn(&mut self) {
        if !ctrl_f_runs(self.cw.selected) {
            return;
        }
        self.bind();
        // SAFETY: as in move_order.
        unsafe { ja1_new_turn(&mut self.cw, &self.h, 1) };
        self.cw.turn_shown = 1;
    }

    /// Section 8 "Quit": Escape stops the selected merc (`ja1_escape_stop` checks selected, combat, screen_drawn).
    /// panel.md "Clicks on the bottom strip": a press at strip x `x` (y 190) for the selected merc; then the
    /// weapon cursor when op 352 came.
    pub fn strip_click(&mut self, st: &mut crate::cursor::CursorState, hs: &mut crate::cursor::HoverState,
                       gv_t3: &[u8; 256], x: i32, right: bool, ctrl: bool) {
        self.bind();
        let slot = self.cw.selected as usize;
        if slot >= 48 {
            return;
        }
        // SAFETY: as in move_order; `bind` set `soldiers` and slot < 48.
        let (in_play, hp) = unsafe {
            let so = self.cw.soldiers.add(slot * crate::soldier::REC);
            (*so.add(0x138), *so.add(0x183) as i8)
        };
        if strip_press_shapes(right, in_play, hp) {
            crate::pointer::shape_from_held();
        }
        (self.cw.mouse_x, self.cw.mouse_y, self.cw.mouse_buttons) = (x, 190, if right { 2 } else { 1 });
        let latch = std::mem::replace(&mut self.cw.key_ctrl, ctrl as u8);
        // SAFETY: as in move_order.
        unsafe { ja1_strip_click(&mut self.cw, &self.h, slot as i32, st.mode as i32) };
        (self.cw.mouse_buttons, self.cw.key_ctrl) = (0, latch);
        // SAFETY: `ctx` is the session's PlayHost.
        let host = unsafe { &mut *(self.h.ctx as *mut PlayHost) };
        if std::mem::take(&mut host.weapon_due) {
            let mut hv = crate::cursor::Hover { w: &mut self.cw, h: &self.h, st, hs, gv_t3, shift: false, alt: false,
                                                confirm_move: self.w.opt_confirm_move != 0, fast_fire: fast_fire_on(self.w), drag: 0, squad: 0 };
            hv.weapon();
        }
    }

    /// sector_start.md "Reserve AP" for merc `slot`.
    pub fn reserve_ap(&mut self, slot: i32) {
        self.bind();
        // SAFETY: as in move_order.
        unsafe { ja1_reserve_ap(&mut self.cw, &self.h, slot) };
    }

    pub fn escape_stop(&mut self) {
        // SAFETY: as in move_order.
        self.bind();
        unsafe { ja1_escape_stop(&mut self.cw, &self.h) };
    }

    pub fn selected(&self) -> u8 {
        self.cw.selected
    }

    /// Space (TacticalKeyHandler case 0x20): the next merc, `ja1_next_merc` from the selected one, only while
    /// combat = 0 or (screen_drawn != 0 and view_hold = 0), and with no item on the cursor.
    pub fn next_merc(&mut self) {
        let held = !self.cw.held.is_null() && unsafe { *self.cw.held } != 0;
        if space_blocked(self.cw.combat != 0, self.cw.screen_drawn != 0, self.cw.view_hold != 0, held) {
            return;
        }
        self.bind();
        let sel = self.cw.selected as i32;
        // SAFETY: as in move_order.
        unsafe { ja1_next_merc(&mut self.cw, &self.h, sel) };
    }

    /// Enter (TacticalKeyHandler case 0x0D): the view goes to the next enemy the selected merc sees,
    /// `ja1_next_enemy` (NextEnemy 0x3F7C8), only with a selected merc, in a fight, and with screen_drawn != 0.
    pub fn next_enemy(&mut self) {
        if !enter_runs(self.cw.selected, self.cw.combat, self.cw.screen_drawn) {
            return;
        }
        self.bind();
        let sel = self.cw.selected as i32;
        // SAFETY: as in move_order.
        unsafe { ja1_next_enemy(&mut self.cw, &self.h, sel) };
    }

    /// X (TacticalKeyHandler case 0x58): the selected merc swaps places with the merc or guard on `tile` (the
    /// mouse tile), `ja1_swap_places` (SwapPlaces 0x40568); with a selected merc, and in a fight only with
    /// screen_drawn != 0.
    pub fn swap_places(&mut self, tile: i32) {
        if !swap_runs(self.cw.selected, self.cw.combat, self.cw.screen_drawn) {
            return;
        }
        self.bind();
        let sel = self.cw.selected as i32;
        // SAFETY: as in move_order.
        unsafe { ja1_swap_places(&mut self.cw, &self.h, sel, tile) };
    }

    /// Host op 188 squad_lost came (soldier_update.md "Squad lost").
    pub fn squad_lost(&self) -> bool {
        // SAFETY: `ctx` is the session's PlayHost.
        unsafe { (*(self.h.ctx as *const PlayHost)).squad_lost }
    }

    /// soldier_update.md "Traverse to a neighbour sector", the popup's condition: selected < 8, byte 2 (mercs) of
    /// sector_state entry cur_sector >= 1 and hp (selected) > 15.
    pub fn can_traverse(&self) -> bool {
        traverse_ok(self.w, self.cw.selected as usize)
    }

    /// The popup's pick `dir` (10 north, 11 east, 12 south, 13 west): `ja1_traverse (selected, dir, out)`.
    /// Returns the result and out (entry tile, new sector, facing).
    pub fn traverse(&mut self, dir: i32) -> (i32, [i16; 3]) {
        let mut out = [0i16; 3];
        // SAFETY: as in move_order; `out` holds the 3 int16 values the call writes.
        self.bind();
        let v = unsafe { ja1_traverse(&mut self.cw, &self.h, self.cw.selected as i32, dir, out.as_mut_ptr()) };
        (v, out)
    }

    /// The field resets at the start of AllMoveTraverse? 0x1CFEC (with ClearTask 0x3DDB8, DL = 1).
    fn drag_resets(&mut self) {
        let k = self.w.selected as usize;
        if k < 48 {
            crate::drag::press_fields(&mut self.w.soldiers[k * 512..(k + 1) * 512]);
        }
    }

    /// The edge test of a drag toward `dir` (cursor.md "Drag to an edge"): `ja1_traverse_edge (selected, dir)`.
    pub fn traverse_edge(&mut self, dir: i32) -> i32 {
        // SAFETY: as in move_order.
        self.bind();
        unsafe { ja1_traverse_edge(&mut self.cw, &self.h, self.cw.selected as i32, dir) }
    }

    /// The move step 1: `ja1_traverse_out (dest)`.
    pub fn traverse_out(&mut self, dest: i32) {
        // SAFETY: as in move_order.
        self.bind();
        unsafe { ja1_traverse_out(&mut self.cw, &self.h, dest) }
    }

    /// The move step 4: `ja1_traverse_in (theme, playing)`; the music theme to change to (0 = none).
    pub fn traverse_in(&mut self, theme: i32, playing: i32) -> i32 {
        // SAFETY: as in move_order.
        self.bind();
        unsafe { ja1_traverse_in(&mut self.cw, &self.h, theme, playing) }
    }

    /// `ja1_time_compress (on)` (turns.md "Time compression").
    pub fn time_compress(&mut self, on: i32) {
        // SAFETY: as in move_order.
        self.bind();
        unsafe { ja1_time_compress(&mut self.cw, &self.h, on) }
    }

    /// Key C (turns.md "Time compression", the key): toggles time compression out of combat; in combat the
    /// status line says why not.
    /// Key F9 (panel.md "Tactical messages"): with selected < 8, the "sees enemies" line of the selected merc
    /// when none shows (as host op 197), else sees_slot = 0x40.
    pub fn sees_key(&mut self) {
        let k = self.cw.selected as usize;
        if k >= 8 {
            return;
        }
        if self.cw.sees_slot == 0x40 {
            let rec = &self.w.soldiers[k * 512..(k + 1) * 512];
            self.cw.sees_slot = rec[0x17A];
            self.w.sees_text = sees_text(rec);
        } else {
            self.cw.sees_slot = 0x40;
        }
    }

    /// cursor.md "Debug keys (level 4)": a level key, or a level-4 key with `t` = the mouse tile. Returns the
    /// messages it showed (the first in the message box, the others after it).
    pub fn debug_key(&mut self, code: u16, t: Option<i32>) -> Vec<String> {
        use crate::debugkeys as d;
        let mut msgs = Vec::new();
        if d::level_key(code) || d::level() != 4 {
            return msgs;
        }
        self.bind();
        let sel = self.cw.selected as usize;
        let tb = |w: &World, t: i32, i: usize| w.tiles.get(t as usize * 8 + i).copied().unwrap_or(0);
        let occ = |w: &World, t: i32| {
            let o = tb(w, t, 5) & 0x7F;
            if o < 0x30 { o } else { 0x40 }
        };
        match (code, t) {
            (d::CTRL_R, _) => d::full_clips(&mut self.w.soldiers, &self.w.item_types),
            // SAFETY: the session's world and host; no other library call runs now.
            (d::ALT_L, _) if sel < 48 => unsafe { ja1_debug_heal(&mut self.cw, &self.h, sel as i32) },
            (d::CTRL_F1, _) => d::flip_save_anywhere(),
            // The toggle texts: the name, then " on" or " off" (0x3F994).
            (d::ALT_Q, _) => msgs.push(format!("Show Roofs {}", if d::flip_roofs() != 0 { " on" } else { " off" })),
            (d::ALT_E, _) => msgs.push(toggle_text("Enemies", &mut self.cw.follow_all)),
            (d::CTRL_T, Some(t)) if sel < 48 => {
                // SAFETY: as above.
                unsafe {
                    ja1_teleport(&mut self.cw, &self.h, sel as i32, t);
                    ja1_replace_soldier(&mut self.cw, &self.h, sel as i32);
                    ja1_turn_sightings(&mut self.cw, &self.h, 1);
                    if ctrl_t_next_turn(self.cw.turn_moves) {
                        if sel < 8 {
                            self.cw.kept_merc = sel as u8;
                        }
                        ja1_next_turn(&mut self.cw, &self.h, 1);
                    }
                }
            }
            (0x4B, Some(t)) => {
                let o = occ(self.w, t);
                if o < 0x40 {
                    // SAFETY: as above.
                    unsafe { ja1_take_damage(&mut self.cw, &self.h, o as i32, 0x1356, 99, 1, 0x40, 0xC41) }
                }
            }
            (0x40, Some(t)) => {
                let obj = tb(self.w, t, 1) as i32;
                // SAFETY: as above.
                unsafe { ja1_break_object(&mut self.cw, &self.h, t, obj, 1) }
            }
            (0x45, Some(t)) => {
                msgs.push(format!("overlay is {}", tb(self.w, t, 2)));
                msgs.push(format!("overlay2 is {}", tb(self.w, t, 3)));
            }
            (0x46, Some(t)) => msgs.push(format!("struc is {}", tb(self.w, t, 1))),
            (0x47, Some(t)) => msgs.push(format!("gridno is {}", t)),
            (d::CTRL_P, _) => {
                // SAFETY: as above.
                unsafe { crate::focus::focus_world(&mut self.cw, sel as i32, 0) };
                msgs.push(format!("CHARACTER SELECTED = {}", sel));
            }
            (d::CTRL_O, Some(t)) => {
                let o = occ(self.w, t);
                if o < 0x30 {
                    msgs = crate::debugpage::opp_lists_of(self.w, o as usize);
                }
            }
            _ => {}
        }
        // SAFETY: `ctx` is the session's PlayHost; no library call runs now.
        let host = unsafe { &mut *(self.h.ctx as *mut PlayHost) };
        let mut it = msgs.iter();
        if let Some(m) = it.next() {
            host.status_msg(m.clone());
            host.msg_more.extend(it.cloned());
        }
        // Ctrl+P: the debug pages of the selected soldier after the message box.
        if code == d::CTRL_P && sel < 0x30 {
            host.debug_pages = Some(sel);
        }
        msgs
    }

    pub fn compress_key(&mut self) {
        if self.cw.combat == 0 {
            let on = (self.cw.time_compress == 0) as i32;
            self.time_compress(on);
        } else {
            // SAFETY: `ctx` is the session's PlayHost; no library call runs now.
            unsafe { (*(self.h.ctx as *mut PlayHost)).status_msg(NO_COMPRESS.to_string()) };
        }
    }

    /// panel.md "Pause (key P) and the clock": Pause (on) with the world's round_lock.
    pub fn pause(&mut self, p: &mut crate::pause::Pause, on: bool) {
        p.set(on, &mut self.cw.round_lock);
    }

    /// panel.md "Other users of Lock": Lock (clock switch `clock`) and Unlock with the world's round_lock.
    pub fn lock(&mut self, p: &mut crate::pause::Pause, clock: bool) {
        p.lock(clock, &mut self.cw.round_lock);
    }

    pub fn unlock(&mut self, p: &mut crate::pause::Pause) {
        p.unlock(&mut self.cw.round_lock);
    }

    /// The world's time_compress byte now.
    pub fn compressing(&self) -> bool {
        self.cw.time_compress != 0
    }

    /// Section 8 "Inventory screen" steps 1-3 for the selected merc (the caller tested Key I's rule). Returns the
    /// result of `ja1_inventory_screen` (1 = the input said stop).
    /// `info`: CursorItemToSoldier 0x54EC8 opens the screen with DL = 1 (0x54D50 -> 0x53A9C): the item info box of
    /// the cursor item (and its voice) on the first pass; key I and the others pass 0.
    pub fn inventory(&mut self, screen: &mut InvScreen, who: usize, info: bool) -> i32 {
        self.bind();
        let old_opt = self.cw.opt_onscreen;
        self.cw.opt_onscreen = 1;
        let sel = who;
        // SAFETY: as in move_order; `screen` outlives the call (the host pointer is cleared before return).
        let r = unsafe {
            let _ = screen.draw(&mut self.cw, &self.h, sel);
            let host = &mut *(self.h.ctx as *mut PlayHost);
            host.inv = screen as *mut InvScreen as *mut InvScreen<'static>;
            let r = ja1_inventory_screen(&mut self.cw, &self.h, sel as i32, info as i32);
            (*(self.h.ctx as *mut PlayHost)).inv = std::ptr::null_mut();
            crate::invscreen::after_screen(&mut self.cw, &self.h, who);
            r
        };
        self.cw.opt_onscreen = old_opt;
        r
    }
}

impl Drop for Session<'_> {
    fn drop(&mut self) {
        // SAFETY: as in bind.
        unsafe { session_out(&self.cw, self.h.ctx as *mut PlayHost, self.w) };
    }
}

/// The end of a session: the host forgets the library's world, and the world gets the session's values.
///
/// # Safety
/// `host` is the session's PlayHost; no library call reads `cw` while this runs.
unsafe fn session_out(cw: &Ja1World, host: *mut PlayHost, w: &mut World) {
    loss_flush(&mut *host, w);
    (*host).ev.poll(cw);
    (*host).world = std::ptr::null_mut();
    cw.store(w);
    w.stash_records = std::mem::take(&mut (*host).stash);
    w.last_quote = (*host).last_quote;
    w.quote_member = (*host).last_member;
    if let Some(t) = (*host).sees_text.take() {
    loss_flush(&mut *host, w);
        w.sees_text = t;
    }
}

/// soldier_update.md "Squad lost" inside host op 188, as in the original: the session of the running library call
/// gives its values to the world, the move to sector 59 runs, and the same session (`cw`, at the same address)
/// then views the new world, so the library goes on in sector 59.
///
/// # Safety
/// `host` is the running Play's PlayHost with `squad_play` / `squad_world` set by `Play::pass` and `world` set by
/// the session of the running library call.
unsafe fn squad_lost_now(host: *mut PlayHost) {
    let (play, w, cw) = ((*host).squad_play, &mut *(*host).squad_world, (*host).world);
    session_out(&*cw, host, w);
    (*host).squad_play = std::ptr::null_mut();
    eprintln!("squad lost inside op 188");
    if let Err(m) = (*play).squad_lost_move(w) {
        eprintln!("squad lost: {}", m);
    }
    *cw = Ja1World::view(w);
    (*host).stash = std::mem::take(&mut w.stash_records);
    (*host).last_quote = w.last_quote;
    (*host).last_member = w.quote_member;
    (*host).world = cw;
    (*host).squad_play = play;
}

/// Section 8 "Choose a merc": merc k (0..7) can be chosen when +0x182 != 0, +0x138 != 0, s8 +0x183 > 0 and
/// +0x188 = 0.
/// SelectMerc 0x1148C with the end of its code at 0x116E2: when it takes the open order (+0x16D) of the merc it leaves,
/// the "cont" goes too.
/// The Fast Fire switch (Control Panel switch 13) for the cursor's Hover.
fn fast_fire_on(w: &World) -> bool {
    crate::ctlpanel::get(w, 13) != 0
}

/// Ctrl+F (0x13BA3): NewTurn runs only with no merc selected (selected >= 8).
fn ctrl_f_runs(selected: u8) -> bool {
    !(selected < 8)
}

/// Enter (TacticalKeyHandler case 0x0D): NextEnemy only with a selected merc, in a fight, with screen_drawn != 0.
fn enter_runs(selected: u8, combat: u8, screen_drawn: u8) -> bool {
    !(selected >= 8 || combat == 0 || screen_drawn == 0)
}

/// The hover steps of a pass: one (the script's rule, `per_tick` = 0) or `per_tick` for each 10 ms tick of the pass,
/// at least one (the original's loop makes ~20 passes per tick at cycles=max, see Play::hover_per_tick).
fn hover_steps(per_tick: u32, ticks: u32) -> u32 {
    if per_tick == 0 { 1 } else { (per_tick * ticks).max(1) }
}

/// The walk-end detector click (MoveMercTo 0x1C7A8) opens its confirm wait only for the selected merc (the confirm
/// is the selected merc's cursor) and only while no other wait is open.
fn detector_wait_opens(slot: i32, selected: u8, wait_open: bool) -> bool {
    slot == selected as i32 && !wait_open
}

/// Key X (TacticalKeyHandler case 0x58): a selected merc (< 8), and in a fight only with screen_drawn != 0.
fn swap_runs(selected: u8, combat: u8, screen_drawn: u8) -> bool {
    !(selected >= 8 || (combat != 0 && screen_drawn == 0))
}

/// StripClick 0x54F3C: a left press of a merc on the map with hp above 14 takes the pointer shape of the held item.
fn strip_press_shapes(right: bool, in_play: u8, hp: i8) -> bool {
    !right && in_play != 0 && hp > 14
}

/// Debug key Ctrl+T (cursor.md "Debug keys (level 4)"): after the teleport, NextTurn runs only in a turn-based round.
fn ctrl_t_next_turn(turn_moves: u8) -> bool {
    turn_moves != 0
}

fn choose_merc_slots(s: &mut Session, sm: &mut crate::slotmouse::SlotMouse, k: usize, alt: bool) {
    let before = open_orders(&s.w.soldiers);
    s.choose_merc(k, alt);
    if order_taken(&before, &open_orders(&s.w.soldiers)) {
        sm.cont = crate::slotmouse::NONE;
    }
}

/// The story scene of the finder in slot `k` starts only for a soldier slot (< 0x40) whose health (s8 +0x183) is 15
/// or more (the test of 0x92DE3 in the tactical loop).
fn finder_fit(soldiers: &[u8], k: usize) -> bool {
    k < 0x40 && (soldiers[k * 512 + 0x183] as i8) >= 15
}

/// The inventory screen after an op 344 pick-up (`info`, CursorItemToSoldier 0x54EC8) opens with the cursor item's
/// info box; a strip click (op 349 inv_open) also sets `inv` but not `info`.
fn pick_shows_info(info: bool, inv: Option<i32>) -> bool {
    info && inv.is_some()
}

/// The open order byte (+0x16D) of the 8 team slots.
fn open_orders(soldiers: &[u8]) -> Vec<u8> {
    (0..8).map(|j| soldiers[j * crate::soldier::REC + 0x16D]).collect()
}

/// True when a merc had an open order `before` and has none `after` (SelectMerc 0x1148C took it at 0x116E2).
fn order_taken(before: &[u8], after: &[u8]) -> bool {
    before.iter().zip(after).any(|(a, b)| *a != 0 && *b == 0)
}

pub fn can_choose(soldiers: &[u8], k: usize) -> bool {
    use crate::soldier::REC;
    if k >= 8 {
        return false;
    }
    let r = &soldiers[k * REC..(k + 1) * REC];
    r[0x182] != 0 && r[0x138] != 0 && (r[0x183] as i8) > 0 && r[0x188] == 0
}

/// One input of a play pass (section 8).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Input {
    /// A left click on tile `tile` with the Shift / Alt keys at that moment.
    Click { tile: i32, shift: bool, alt: bool },
    /// A left press on tile `tile` dragged before its release so far that it points at the sector edge `dir` (10
    /// north, 11 east, 12 south, 13 west; cursor.md "Drag to an edge"). With result 1 play stops at once and the
    /// caller runs `traverse_move`.
    Drag { tile: i32, dir: i32, shift: bool, alt: bool },
    /// The release of a left press on `tile` during which the right button came (cursor.md "ALL-move"): the walk
    /// with the squad.
    AllMove { tile: i32, shift: bool, alt: bool },
    /// A right click on tile `tile` (press and release, no drag, no left button): a hold of one poll.
    RClick { tile: i32, shift: bool, alt: bool },
    /// The right button goes down on tile `tile` with the mouse at y (cursor.md "Clicks": the hold starts).
    RHold { tile: i32, shift: bool, alt: bool, y: i32 },
    /// One mouse poll while the right button is held (the hold ends when `right` is false).
    Poll { left: bool, right: bool, y: i32 },
    /// Key Escape: answers an open aim or confirm; else it stops the selected merc (section 8 "Quit").
    Escape,
    /// Space: the next merc (NextMerc? 0x1315C).
    NextMerc,
    /// Enter: the view on the next enemy in sight (NextEnemy 0x3F7C8).
    NextEnemy,
    /// X: swap places with the merc at the mouse tile (SwapPlaces 0x40568).
    Swap,
    /// `+` `-` `.` `,`: music or sound volume up / down (sound::volume_key).
    Volume(u8),
    /// V: the version text as a status message (TacticalKeyHandler case V 0x13B94: StatusMessage 0x90334).
    Version,
    /// An arrow key (scan code 0x48 up, 0x4B left, 0x4D right, 0x50 down): the view scroll (viewmode::Scroll).
    Arrow(u8),
    /// Ctrl+F with no merc selected: the player's turn starts now (TacticalKeyHandler 0x13BA3).
    ForceTurn,
    /// Key D: end turn.
    EndTurn,
    /// Key 1..8: merc 0..7 (Alt held).
    Merc(u8, bool),
    /// A left click on the face of slot 0..7 (Alt held).
    Portrait(u8, bool),
    /// The mouse over the face of slot 0..7 (0xFF: off the faces).
    Face(u8),
    /// Key C: time compression on / off (turns.md "Time compression").
    Compress,
    /// Key F9: the "sees enemies" line on / off (panel.md "Tactical messages").
    SeesLine,
    /// Any other key or mouse button press of the window: only turns time compression off (while it is on).
    Other,
    /// Key A (or the abort button) answered Y to "ABORT this day?", then `restart` = the answer to "RESTART this
    /// day?" (section 8 "Abort or restart the day"): play stops at once.
    Abort { restart: bool },
    /// The traverse popup's pick (soldier_update.md "Traverse to a neighbour sector"): 10 north, 11 east, 12 south,
    /// 13 west. With result 1 play stops at once and the caller runs `traverse_move`.
    Traverse(i32),
    /// panel.md "Clicks on the bottom strip": a left (or right) press at strip x `x` (mouse y 182..198), Ctrl held.
    Strip { x: i32, right: bool, ctrl: bool },
    /// panel.md "View modes": key 1 PgUp, 2 PgDn, 3 Insert, 4 Tab (run after the pass's other inputs).
    View(u8),
    /// panel.md "Pause (key P) and the clock": key P.
    Pause,
    /// cursor.md "Tile popups": key Z (the popup of the mouse tile).
    Popup,
    /// panel.md "Say it again (key L)": the last quote's text again.
    SayAgain,
    /// cursor.md "Debug keys (level 4)": a key code of debugkeys.
    Debug(u16),
}

impl Input {
    /// An input of the mouse part of TacticalKeyHandler (0x141EC on); the rest are keys.
    fn is_mouse(&self) -> bool {
        matches!(self, Input::Click { .. } | Input::RClick { .. } | Input::Drag { .. } | Input::AllMove { .. } | Input::RHold { .. }
            | Input::Poll { .. } | Input::Portrait(..) | Input::Face(..) | Input::Strip { .. })
    }
}

/// cursor.md "Drag to an edge": the loop state of AllMoveTraverse? 0x1CFEC + the keys of the press and the last point.
#[derive(Clone, Copy, Debug)]
pub struct EdgeDrag {
    pub loop_: crate::drag::Drag,
    pub shift: bool,
    pub alt: bool,
    /// The mouse point of the last poll.
    pub at: (i32, i32),
}

/// The drag loop's calls on the running game.
struct DragHost<'a, 'b> {
    s: &'a mut Session<'b>,
    refused: bool,
}

impl crate::drag::Host for DragHost<'_, '_> {
    fn edge(&mut self, dir: u8) -> bool {
        self.s.traverse_edge(dir as i32) != 0
    }
    fn redraw(&mut self, _value: u8, _x: i32, _y: i32) {}
    fn refuse(&mut self) {
        self.refused = true;
    }
}

/// The play loop's state (section 8): the host, whether the sector was left, the count of passes.
pub struct Play {
    pub host: PlayHost,
    /// The 8-bit sum of a pass's results (loop head, frame, update end, leave tests) was != 0: the sector was
    /// left, no more passes run.
    pub left: bool,
    /// The sector ended in the pass of a pick-up whose inventory screen is still to run: the leave path waits for
    /// it (`leave_late`).
    pub leave_held: bool,
    pub passes: u64,
    /// The inventory screen's blink byte (kept between drawings) and the last frame it showed.
    pub inv_blink: u8,
    pub inv_last: Option<Vec<u8>>,
    /// The script's last message box text (JA1_MSG_PNG); a script line `msgbox TEXT` per box.
    pub last_box: Option<String>,
    /// The script's last battle box (JA1_BATTLE_PNG): sector, (enemies, guards, tappers), kind.
    pub last_battle: Option<(i32, (u8, u8, u8), i32)>,
    /// strategic.md "The battle show", At the end of a sector: the last box came after the Leave path's fade
    /// out (black palette: nothing of it shows).
    pub last_battle_dark: bool,
    /// The stat popups of key S when the script's last line is `statkey` (JA1_STATS_PNG).
    pub stat_shown: Option<Vec<u8>>,
    /// The script's open Control Panel (`opt` lines) and the drawing steps of the last one (JA1_PANEL_PNG).
    pub ctl: Option<crate::ctlpanel::Panel>,
    pub ctl_log: Option<Vec<crate::ctlpanel::Draw>>,
    /// The script's last speech balloon (soldier record, quote) for JA1_BALLOON_PNG.
    pub last_balloon: Option<(Vec<u8>, i32, String, bool)>,
    /// The script's last speech balloon was Santino's (panel.md "Santino's balloon").
    pub last_santino: bool,
    /// The script's last door film (member, x, y, FLC bytes) for JA1_DOOR_PNG.
    pub last_door_film: Option<(String, i32, i32, Vec<u8>)>,
    /// The script's last drown / injection popup (slot, flag, selected slot) for JA1_ANIMPOP_PNG.
    pub last_anim_popup: Option<(u8, i32, u8)>,
    /// The last guide or Brenda balloon of the script (op, arguments), for JA1_BALLOON_PNG (None when a merc or
    /// Santino balloon came after it).
    pub last_guide: Option<(i32, Vec<i32>, bool)>,
    /// The day loop (world_load.md section 9).
    pub day: crate::dayloop::DayLoop,
    /// The CD image and JA.EXE paths for the day loop's sector entry (None: a `start` line cannot enter).
    pub files: Option<(String, String)>,
    /// JA1_SCREEN_PNG: keep a copy of the world (and the tileset) before each pass while the sector is not left,
    /// so that the tactical frame before a leave can be drawn after the script.
    pub keep_frame: bool,
    pub frame_world: Option<(Box<World>, u8)>,
    /// An `Input::Abort` stopped the last pass (the answer to "RESTART this day?"); the caller runs
    /// `abort_day` with it.
    pub aborted: Option<bool>,
    /// The script's Control Panel closed with Quick Save (100) or Quick Restore (101): the next `quick Y|N` line
    /// answers its question box (savegame.md "Quick Save and Quick Restore: the steps").
    pub quick: Option<i32>,
    /// A traverse with result 1 stopped the last pass: out of `ja1_traverse` (entry tile, new sector, facing); the
    /// caller runs `traverse_move` with it.
    pub traversed: Option<[i16; 3]>,
    /// soldier_update.md "Squad lost" loaded sector 59 in the last pass: the window loads the new tileset,
    /// palette and strips (as after a traverse).
    pub moved: bool,
    /// `moved` comes from Quick Restore, not from Squad lost.
    pub quick_restored: bool,
    /// Section 8 "Hover": the cursor state and the hover state (cursor.md), the mouse tile over the map (None = off
    /// the map), a mouse button held, Shift / Alt held, and gv_t3 of JA.EXE (None: no hover step).
    pub cursor: crate::cursor::CursorState,
    pub hover: crate::cursor::HoverState,
    pub mouse: Option<i32>,
    pub mouse_held: bool,
    /// Hover steps per tick (0 = one per pass, the script's rule). The original runs one hover step per loop pass,
    /// and its loop is not paced: DOSBox at cycles=max (the GOG setting) makes ~20 passes per 10 ms tick (measured
    /// 2026-10-29: the AP delay 800 -> 0 in 0.04 s; #29 H). The window sets 20, so the path marks, the AP number
    /// and the other hover delays come as quickly as there.
    pub hover_per_tick: u32,
    /// The right button is held (the window's state; a posture confirm hands it to the hold).
    pub right_held: bool,
    /// A right-button look ended in this pass: Set mode (0) after it (panel.md "View modes").
    pub look_view: bool,
    /// cursor.md "Tile popups": the tile whose popup the front-end shows after the pass, and true for the key Z
    /// (the end wait is a key or a click) or false for the right click (the right button up).
    pub popup: Option<(i32, bool)>,
    /// The script's last tile popup (`popup ...` line), kept for JA1_POPUP_PNG.
    pub last_popup: Option<crate::tilepopup::Popup>,
    /// The messages of the last debug key (cursor.md "Debug keys (level 4)"), for the script report.
    pub debug_msgs: Vec<String>,
    /// The soldier of the script's last debug pages (JA1_PAGES_PNG).
    pub last_pages: Option<usize>,
    /// panel.md "Say it again (key L)": the box shows after the pass; `last_again` = a script's key L (JA1_AGAIN_PNG).
    pub again: bool,
    pub last_again: bool,
    /// panel.md "The overview mouse step": its values (overview colour and "over") and the tile of a right click
    /// in mode 0 (the overview click after the pass).
    pub ov: crate::viewmode::OvMouse,
    pub ov_jump: Option<i32>,
    /// panel.md "Pause (key P) and the clock".
    pub pause: crate::pause::Pause,
    /// The mouse point on the screen (the window sets it; a script keeps (160, 100)) and the point Set mode moved
    /// it to (the window moves the real mouse there).
    pub mouse_px: (i32, i32),
    /// In a hold with the mouse off the map the frames do not show a held item (HideMouse? 0x67368).
    pub held_off: bool,
    /// The mouse left the window (the window sets it; a DOS mouse never leaves the screen, so a mouse outside the
    /// window does not scroll the view at its last edge point, unless `edge_out` gives an edge point).
    pub mouse_out: bool,
    /// Port option (2026-11-25): the screen point at the window edge nearest to a released pointer outside the
    /// focused window (the window sets it before each pass); with it the edge scroll goes on while `mouse_out`. Play
    /// scripts never set it (the DOS rule above).
    pub edge_out: Option<(i32, i32)>,
    /// The mouse point off the map, None = on the map (the window and the script line `face` set it).
    pub slot_pt: Option<(i32, i32)>,
    /// The mouse on the merc slots: its four bytes (0x92DC8, 0x93448, 0x92E58, 0x284F16).
    pub slots: crate::slotmouse::SlotMouse,
    /// SlotMouse 0x92DC8: the slot whose face shows "cont" (0x40 none).
    /// A left press on a merc slot ran its action: the window stops the game until the release (WaitLeftUp of SlotMouse's left arm).
    pub face_press: bool,
    pub cont: u8,
    pub mouse_warp: Option<(i32, i32)>,
    /// cursor.md "Drag to an edge": the left press on the map in walk mode that waits for its release.
    pub drag: Option<EdgeDrag>,
    /// The cursor value the drag shows (the edge arrow 0x0A..0x0D or 0xA1); None = the value of the press.
    pub drag_value: Option<u8>,
    /// AllMoveTraverse? 0x1CFEC wrote the cursor tile 0xC41 at the release: the walk or traverse of its input runs with
    /// that, and the next pass's hover finds the mouse's tile as a new one.
    pub drag_end: bool,
    /// The pass after the end of the loop of AllMoveTraverse? 0x1CFEC is the rest of the pass of the press: its key flag
    /// step 0x1409B ran before the press, so the Alt, Ctrl and Shift latches keep their state of the press until the
    /// next pass (a key held in the loop reads the Alt latch of the press, not the Alt state at the release).
    pub drag_tail_done: bool,
    /// An open aim or confirm of cursor.md "Clicks" (section 8 "Click": a wait).
    pub wait: Option<crate::cursor::ClickWait>,
    pub shift_alt: (bool, bool),
    /// The keyboard flags 0x22A46C of this pass, written by the window before `pass`: 3 Shift, 0x104 Ctrl, 0x208 Alt.
    pub key_flags: u16,
    /// The latches of `cursor::key_flags_pass`.
    pub key_latches: crate::cursor::KeyLatches,
    pub gv_t3: Option<Box<[u8; 256]>>,
    /// Key V's text (TacticalKeyHandler, from JA.EXE: data::VERSION_TACTICAL; "" before set_exe).
    pub version: &'static str,
    /// panel.md "Arrow-key scroll": the steps and timers of the view scroll.
    pub scroll: crate::viewmode::Scroll,
    /// The `amap` lines' island map screen (the window's day map: assignment screen, item panel); made at the
    /// first `amap` line of a day, dropped at `start`.
    pub amap: Option<Box<crate::mapui::MapUi>>,
    /// The `gmap` lines' guard screen (tactical key M, strategic.md "The guard screen"); made at the first `gmap`
    /// line, dropped when the screen closes.
    pub gmap: Option<Box<crate::mapui::MapUi>>,
    /// The screen after the last `amap` line and its palette (JA1_AMAP_PNG).
    pub amap_last: Option<(Vec<u8>, [u8; 768])>,
}

impl Play {
    /// The values of the overview cursor (panel.md "The overview picture" step 4) for a draw in view mode 0: the
    /// mouse point (the window's mouse; a script's mouse tile gives the middle of its overview cell), the window
    /// rectangle and the overview mouse step's colour, cost and "over". combat / tactical_on / overlay_up come from
    /// the cursor state at the draw.
    pub fn ov_cursor(&self, w: &World) -> crate::overview::OvCursor {
        let (cols, rows, left, top) = crate::viewmode::world_window(w);
        let (mut mx, mut my) = self.mouse_px;
        if let Some(t) = self.mouse {
            if crate::viewmode::overview_tile(mx, my) != t || mx < 48 || my < 14 {
                (mx, my) = (48 + 4 * (t % 56) + 2, 14 + 3 * (t / 56) + 1);
            }
        }
        // AllMoveTraverse? 0x1CFEC: the drag's last redraw gives the overview its value and point.
        let (p, px, py) = match self.drag.and_then(|d| d.loop_.shown) {
            Some((v, x, y)) => (v as i32, x, y),
            None => (self.ov.colour, 0, 0),
        };
        crate::overview::OvCursor { p, px, py, mouse_x: mx, mouse_y: my, left, top,
            right: left + cols * 16 - 1, bottom: top + rows * 14 - 1, cost: self.ov.cost, over: self.ov.over,
            combat: 0, tactical_on: 0, overlay_up: 0, clock: self.pause.clock_shows() as i32,
            clock_hour: w.clock.hour as i32, clock_minute: w.clock.minute as i32 }
    }

    /// The result of a posture confirm and of the hold answer after it: the confirm's text (a merc text or a status
    /// line text of the posture change) stays; the hold answer gives the wait and the rest.
    fn posture_out(confirm_status: Option<String>, o2: crate::cursor::ClickOut) -> crate::cursor::ClickOut {
        crate::cursor::ClickOut { status: confirm_status.or(o2.status.clone()), ..o2 }
    }

    /// After a posture confirm (cursor.md "Clicks", right step 3.4): the hold ends or goes on by the right
    /// button's state now.
    #[allow(clippy::too_many_arguments)]
    fn posture_end(s: &mut Session, cursor: &mut crate::cursor::CursorState, hover: &mut crate::cursor::HoverState,
                   lines: &mut Vec<String>, right_held: bool, g: &[u8; 256], o: crate::cursor::ClickOut) -> crate::cursor::ClickOut {
        match o.wait {
            Some(h @ crate::cursor::ClickWait::Hold(crate::cursor::Hold { check: true, .. })) => {
                let p = crate::cursor::Answer::Poll { left: false, right: right_held, y: 0 };
                let o2 = s.answer(cursor, hover, g, h, p);
                Self::hold_line(lines, cursor, &o2, h);
                Self::posture_out(o.status, o2)
            }
            _ => o,
        }
    }

    /// panel.md "The small panel in the full view": Show (after an op 196 of this pass), End (`in_view` = the mouse
    /// step ran inside the view window), then the Countdown over the pass's timer ticks.
    pub fn small_panel(&mut self, w: &mut World, ticks: u32, in_view: bool) {
        if self.host.select.panel.take().is_some() && w.opt_onscreen == 3 {
            w.small_panel = self.host.select.panel_timer;
            w.strip_flag = 1;
            w.view_h = 14 * 13;
            if self.host.select.mouse_y != self.mouse_px.1 {
                self.mouse_px.1 = self.host.select.mouse_y;
                self.mouse_warp = Some(self.mouse_px);
            }
        }
        if in_view && w.panel_ends as i8 == 1 {
            w.panel_ends = 0;
            let mut v = crate::viewmode::from_world(w, self.mouse_px);
            v.full_view();
            crate::viewmode::to_world(&v, w);
            self.day.lines.push(format!("full view x {} y {}", w.view_x, w.view_y));
        }
        for _ in 0..ticks {
            panel_countdown(w, self.hover.lit);
        }
    }

    /// panel.md "View modes": key 0 = Set mode (0) (the end of a look), 1 PgUp, 2 PgDn, 3 Insert, 4 Tab. Writes the
    /// script line `view mode M x X y Y filter F`.
    pub fn view_key(&mut self, w: &mut World, key: u8) {
        let mut v = crate::viewmode::from_world(w, self.mouse_px);
        match key {
            0 => v.set(0),
            1 => v.page_up(),
            2 => v.page_down(),
            3 => v.insert(),
            _ => v.tab(),
        }
        crate::viewmode::to_world(&v, w);
        if v.marks_off > 0 {
            Session::new(w, &mut self.host).hide_path_marks(0);
        }
        if v.mouse_moves > 0 {
            self.mouse_px = (v.mouse_x, v.mouse_y);
            self.mouse_warp = Some(self.mouse_px);
            self.day.lines.push(format!("view mouse {} {}", v.mouse_x, v.mouse_y));
        }
        self.day.lines.push(format!("view mode {} x {} y {} filter {}", w.opt_onscreen, w.view_x, w.view_y, w.overview_filter));
    }

    /// panel.md "Arrow-key scroll" + "Mouse-edge scroll": the arrow keys of this pass, the mouse at a screen edge,
    /// then the scroll step (TacticalKeyHandler's end). Writes the script line `scroll x X y Y` when the view moved.
    pub fn scroll_pass(&mut self, w: &mut World, inputs: &[Input]) {
        let mut key = false;
        for i in inputs {
            if let Input::Arrow(k) = *i {
                key |= self.scroll.arrow_key(k);
            }
        }
        let mut v = crate::viewmode::from_world(w, self.mouse_px);
        let edge = if self.mouse_out { self.edge_out } else { Some(self.mouse_px) };
        if let Some((x, y)) = edge {
            self.scroll.mouse_edge(&v, x, y);
        }
        let (fast, alt_scroll) = (crate::ctlpanel::get(w, 2), crate::ctlpanel::get(w, 3));
        if self.scroll.pass(&mut v, &mut self.cursor.tile, key, fast, alt_scroll, self.shift_alt.1 as u8) {
            crate::viewmode::to_world(&v, w);
            self.day.lines.push(format!("scroll x {} y {}", w.view_x, w.view_y));
        }
    }

    /// panel.md "The overview click" at the overview point of tile `t`: the view goes there and the old mode comes
    /// back. Writes the script line `overview T view mode M x X y Y`.
    pub fn overview_jump(&mut self, w: &mut World, t: i32) {
        let mut v = crate::viewmode::from_world(w, self.mouse_px);
        let (x, y) = crate::viewmode::overview_point(t);
        let tile = v.overview_click(x, y);
        crate::viewmode::to_world(&v, w);
        if v.marks_off > 0 {
            Session::new(w, &mut self.host).hide_path_marks(0);
        }
        self.cursor.tile = tile;
        self.mouse_px = (v.mouse_x, v.mouse_y);
        self.mouse_warp = Some(self.mouse_px);
        self.mouse = Some(tile);
        self.day.lines.push(format!("overview {} view mode {} x {} y {}", tile, w.opt_onscreen, w.view_x, w.view_y));
    }

    /// The script line of a right button hold: `hold T posture confirm` when the posture's confirm opens,
    /// `hold T turn A look B posture C value V mode M` when the hold ends.
    fn hold_line(lines: &mut Vec<String>, cur: &crate::cursor::CursorState, out: &crate::cursor::ClickOut, h: crate::cursor::ClickWait) {
        let crate::cursor::ClickWait::Hold(h) = h else { return };
        if let Some(crate::cursor::ClickWait::Posture(_)) = out.wait {
            lines.push(format!("hold {} posture confirm", h.tile));
        } else if let Some(g) = out.ended {
            lines.push(format!("hold {} turn {} look {} posture {} value {:#x} mode {}", g.tile, g.turn as u8,
                               g.look as u8, g.posture as u8, cur.value, cur.mode));
        }
    }

    /// One `amap` line on the window's island map screen (the random numbers are play's, as in the window).
    /// Returns the script line `amap X Y B assign A selected K cursor T`.
    fn amap_line(&mut self, w: &mut World, x: i32, y: i32, b: i32) -> Result<String, String> {
        let fail = |what: &str, c: i32| format!("amap: {} failed ({})", what, c);
        if self.amap.is_none() {
            let (iso, exe) = self.files.clone().ok_or("amap: no CD image / JA.EXE")?;
            let mut m = Box::new(crate::mapui::MapUi::new(&iso, &exe, self.host.rng.seed));
            m.day = true;
            m.show(w, (x.max(0), y)).map_err(|c| fail("show", c))?;
            self.amap = Some(m);
        }
        let m = self.amap.as_mut().expect("amap");
        if x == -3 {
            m.keys.1 = y != 0;
            return Ok(format!("amap ctrl {}", y));
        }
        *m.rng() = crate::host::Rng { seed: self.host.rng.seed, draws: self.host.rng.draws };
        if m.box_shown() {
            // panel.md "Message box": as in the window, a key or a press (up again) only ends the box.
            use crate::mapui::BoxInput;
            if x < 0 {
                m.box_input(BoxInput::Key).map_err(|c| fail("box", c))?;
            } else if b != 0 {
                m.box_input(BoxInput::Down(1)).map_err(|c| fail("box", c))?;
                m.box_input(BoxInput::Up(1)).map_err(|c| fail("box", c))?;
            } else {
                m.track(w, (x, y)).map_err(|c| fail("mouse", c))?;
            }
        } else if x < 0 {
            m.key(w, y as u8).map_err(|c| fail("key", c))?;
            if y == b'T' as i32 && m.box_shown() {
                self.day.lines.push(format!("map msg {}", m.box_text().unwrap_or("")));
            }
            if y == b'L' as i32 && m.box_shown() {
                // panel.md "Say it again (key L)": the shown quote as `again S cat C row R quote Q member M`.
                let q = &w.last_quote;
                self.day.lines.push(format!("again {} cat {} row {} quote {} member {}", q[0], q[1],
                                            u16::from_le_bytes([q[2], q[3]]), u16::from_le_bytes([q[4], q[5]]), w.quote_member));
            }
        } else {
            m.track(w, (x, y)).map_err(|c| fail("mouse", c))?;
            // Button value + 16: only the press. + 32: only the release.
            let (down, up, bits) = (b & 32 == 0, b & 16 == 0, b & 3);
            if bits != 0 {
                if down {
                    m.click(w, bits).map_err(|c| fail("click", c))?;
                    m.warp = None;
                }
                if up {
                    m.release(w, bits).map_err(|c| fail("release", c))?;
                    // Keys kept in the queue during the release wait go to the map now (as `amap key`).
                    for c in m.take_kept().into_iter().filter(|&c| c != crate::mapui::KEPT_ALT_X) {
                        self.day.lines.push(format!("map kept key {}", c));
                        m.key(w, c).map_err(|c| fail("key", c))?;
                        if c == b'T' && m.box_shown() {
                            self.day.lines.push(format!("map msg {}", m.box_text().unwrap_or("")));
                        }
                    }
                }
            }
        }
        m.drain();
        (self.host.rng.seed, self.host.rng.draws) = (m.rng().seed, m.rng().draws);
        let sel = m.selected_merc().map(|k| k as i32).unwrap_or(-1);
        self.amap_last = Some((m.img.clone(), m.pal));
        Ok(format!("amap {} {} {} assign {} selected {} cursor {}", x, y, b, m.assigning() as u8, sel, w.held[0]))
    }

    /// The steps of `guardopen::ENTER` and `LEAVE` that change the world; a window does the other ones.
    pub fn guard_step(&mut self, w: &mut World, step: crate::guardopen::Step) {
        use crate::guardopen::Step;
        match step {
            Step::CursorBack => {
                if Session::new(w, &mut self.host).cursor_back() {
                    self.day.lines.push("gmap cursor back".into());
                }
            }
            Step::HideMarks => Session::new(w, &mut self.host).hide_path_marks(0),
            Step::MouseOff => crate::guardopen::mouse_off(&mut self.cursor),
            Step::HideMouse | Step::Fade(..) | Step::Palette | Step::Strips | Step::Redraw => {}
        }
    }

    /// One `gmap` line on the guard screen (tactical key M; as the window: main.rs runs guard_show, then track /
    /// click / release / key, guard_pass every round and Close through `guard_close`). Returns the script line
    /// `gmap X Y B pick P here H sent N box M` (M = 1: a message box shows); a line that closes the screen adds the line
    /// `guard screen closed: N guards sent, M here`. The line opens the screen first when it is not open
    /// (`gmap open` only opens it; `gmap open` when the screen does not open: `gmap open no`).
    fn gmap_line(&mut self, w: &mut World, x: i32, y: i32, b: i32) -> Result<(), String> {
        let fail = |what: &str, c: i32| format!("gmap: {} failed ({})", what, c);
        if self.gmap.is_none() {
            if !crate::guardopen::key_m_opens(w.combat, w.update.screen_drawn) {
                self.day.lines.push("gmap open no".into());
                return Ok(());
            }
            for k in crate::guardopen::ENTER {
                self.guard_step(w, k);
            }
            let (iso, exe) = self.files.clone().ok_or("gmap: no CD image / JA.EXE")?;
            let mut m = Box::new(crate::mapui::MapUi::new(&iso, &exe, self.host.rng.seed));
            *m.rng() = crate::host::Rng { seed: self.host.rng.seed, draws: self.host.rng.draws };
            let shown = m.guard_show(w).map_err(|c| fail("open", c))?;
            (self.host.rng.seed, self.host.rng.draws) = (m.rng().seed, m.rng().draws);
            if !shown {
                self.day.lines.push("gmap open no".into());
                return Ok(());
            }
            m.warp = None;
            self.gmap = Some(m);
            if x == -2 {
                let m = self.gmap.as_ref().expect("gmap");
                self.amap_last = Some((m.img.clone(), m.pal));
                self.day.lines.push("gmap open".into());
                return Ok(());
            }
        }
        if x == -2 {
            return Ok(());
        }
        let m = self.gmap.as_mut().expect("gmap");
        *m.rng() = crate::host::Rng { seed: self.host.rng.seed, draws: self.host.rng.draws };
        if m.box_shown() {
            use crate::mapui::BoxInput;
            if x < 0 {
                m.box_input(BoxInput::Key).map_err(|c| fail("box", c))?;
            } else if b != 0 {
                m.box_input(BoxInput::Down(1)).map_err(|c| fail("box", c))?;
                m.box_input(BoxInput::Up(1)).map_err(|c| fail("box", c))?;
            } else {
                m.track(w, (x, y)).map_err(|c| fail("mouse", c))?;
            }
        } else if x < 0 {
            m.key(w, y as u8).map_err(|c| fail("key", c))?;
            if y == b'T' as i32 && m.box_shown() {
                self.day.lines.push(format!("map msg {}", m.box_text().unwrap_or("")));
            }
        } else {
            m.track(w, (x, y)).map_err(|c| fail("mouse", c))?;
            if b != 0 {
                m.click(w, b).map_err(|c| fail("click", c))?;
                m.warp = None;
                m.release(w, b).map_err(|c| fail("release", c))?;
            }
        }
        m.guard_pass(w).map_err(|c| fail("pass", c))?;
        m.drain();
        (self.host.rng.seed, self.host.rng.draws) = (m.rng().seed, m.rng().draws);
        self.amap_last = Some((m.img.clone(), m.pal));
        let line = match m.guard_state() {
            Some(g) => format!("gmap {} {} {} pick {} here {} sent {} box {}", x, y, b, g.pick_n, g.here,
                               g.moves_n.iter().map(|&n| n as i32).sum::<i32>(), m.box_shown() as u8),
            None => format!("gmap {} {} {}", x, y, b),
        };
        self.day.lines.push(line);
        if let Some(mut g) = m.guard_take_closing() {
            let sent: i32 = g.moves_n.iter().map(|&n| n as i32).sum();
            self.gmap = None;
            self.guard_close(w, &mut g);
            self.day.lines.push(format!("guard screen closed: {} guards sent, {} here", sent, g.here));
        }
        Ok(())
    }

    /// cursor.md "Drag to an edge": a left press on map tile `tile` at screen point `at` in walk mode.
    /// A left press on the map: does it start the edge drag? UseItemAt 0x1D9B8 and OverviewMouse 0x24630 run the loop
    /// only for a tile with no merc; every other press acts at once.
    pub fn press_walks(&mut self, w: &mut World, tile: i32, shift: bool, alt: bool) -> bool {
        let overview = w.opt_onscreen == 0;
        let Some(g) = self.gv_t3.as_deref() else { return false };
        let mut s = Session::new(w, &mut self.host);
        if overview {
            return s.overview_press_walks(tile);
        }
        // Section 8 "Click": the mouse comes to the tile one frame before the press.
        if self.mouse != Some(tile) {
            self.mouse = Some(tile);
            s.hover(&mut self.cursor, &mut self.hover, g, tile, shift, alt);
        }
        s.press_walks(&mut self.cursor, &mut self.hover, g, tile, shift, alt)
    }

    pub fn drag_press(&mut self, tile: i32, at: (i32, i32), shift: bool, alt: bool, value: u8, cursor_tile: i32) {
        self.drag = Some(EdgeDrag { loop_: crate::drag::Drag::new(tile, at, value, cursor_tile), shift, alt, at });
        self.drag_value = None;
    }

    /// One mouse poll of the loop of AllMoveTraverse? 0x1CFEC (drag.rs). `buttons`: 1 left, 2 right. At the end of the
    /// loop: the input that goes on (the walk or the traverse), if one does.
    pub fn drag_step(&mut self, w: &mut World, x: i32, y: i32, buttons: u8) -> Option<Input> {
        use crate::drag::End;
        let mut d = self.drag?;
        d.at = (x, y);
        let (combat, locked) = (w.combat != 0, self.pause.paused);
        let all0 = d.loop_.all;
        let (end, refused) = {
            let mut s = Session::new(w, &mut self.host);
            let mut h = DragHost { s: &mut s, refused: false };
            let end = d.loop_.poll(x, y, buttons, combat, locked, &mut h);
            (end, h.refused)
        };
        if refused {
            self.host.status_msg("ALL-MOVE not available during Turn-based".into());
            self.day.lines.push("allmove refused".into());
        }
        if d.loop_.all && !all0 {
            self.day.lines.push("allmove on".into());
        }
        self.drag_value = d.loop_.shown.map(|v| v.0);
        let Some(end) = end else {
            self.drag = Some(d);
            return None;
        };
        self.drag = None;
        self.drag_value = None;
        self.drag_tail_done = true;
        let (tile, shift, alt) = (d.loop_.tile, d.shift, d.alt);
        match end {
            End::Refused | End::NoEdge => {
                // The value and the tile of the loop's end; the next pass finds the mouse's tile again.
                self.cursor.value = d.loop_.value;
                self.cursor.tile = d.loop_.cursor_tile;
                self.mouse = None;
                // 0x1CFFC: the press's field resets; a walk or a traverse runs them in `Walk`.
                Session::new(w, &mut self.host).drag_resets();
                if end == End::NoEdge {
                    self.host.status_msg("Can't traverse here!".into());
                    self.day.lines.push("drag noedge".into());
                }
                None
            }
            End::Traverse(dir) => {
                self.drag_end = true;
                Some(Input::Drag { tile, dir: dir as i32, shift, alt })
            }
            End::Walk { all: true } => {
                self.drag_end = true;
                Some(Input::AllMove { tile, shift, alt })
            }
            End::Walk { all: false } => {
                self.drag_end = true;
                Some(Input::Click { tile, shift, alt })
            }
        }
    }

    /// The timer runs while the left button holds the drag loop; the game's passes do not.
    pub fn drag_ticks(&mut self, w: &mut World, n: u64) {
        let mut s = Session::new(w, &mut self.host);
        timer_ticks(&mut s, &mut self.pause, &mut self.cursor, &mut self.scroll, &mut self.day.lines, n);
    }

    /// Play goes on from the seed at the end of the sector entry.
    pub fn new(seed: u32) -> Play {
        Play { host: PlayHost::new(seed), quick_restored: false, left: false, leave_held: false, passes: 0, inv_blink: 0, inv_last: None, last_box: None, last_battle: None, last_battle_dark: false, stat_shown: None, ctl: None, ctl_log: None, last_balloon: None, last_santino: false, last_door_film: None, last_anim_popup: None, last_guide: None, day: Default::default(), files: None,
               keep_frame: false, frame_world: None, aborted: None, quick: None, traversed: None, moved: false, cursor: Default::default(), hover: Default::default(),
               key_flags: 0, key_latches: Default::default(), mouse: None, mouse_held: false, hover_per_tick: 0, right_held: false, look_view: false, popup: None, last_popup: None, debug_msgs: Vec::new(), last_pages: None, again: false, last_again: false, ov: Default::default(), ov_jump: None, pause: Default::default(), mouse_px: (160, 100), held_off: false, mouse_out: false, edge_out: None, slot_pt: None, slots: Default::default(), cont: 0x40, face_press: false, mouse_warp: None, drag: None, drag_value: None, drag_end: false, drag_tail_done: false, wait: None, shift_alt: (false, false), gv_t3: None, version: "", scroll: Default::default(), amap: None, gmap: None, amap_last: None }
    }

    /// Takes gv_t3 from JA.EXE (needed by the hover step).
    pub fn set_exe(&mut self, exe: &str) -> Result<(), i32> {
        crate::music::set_exe(exe);
        self.gv_t3 = Some(Box::new(crate::exe_tables(exe)?.gv_t3));
        self.version = crate::data::exe_text(exe, crate::data::VERSION_TACTICAL);
        Ok(())
    }

    /// Section 8 "Hover": the mouse over map tile `t` (None = off the map). Off the map over_map stays as the last
    /// hover step left it: the pass's off-map step (TacticalKeyHandler 0x13490) clears it and the rest.
    pub fn set_mouse(&mut self, t: Option<i32>) {
        self.mouse = t;
    }

    /// Key I (section 8 "Inventory screen"): when the rule allows it, the screen runs with `input` (faces and
    /// table 0 of the tactical screen); returns None when it did not open, else the library's result.
    /// The guard screen closes (strategic.md "The guard screen", Close).
    pub fn guard_close(&mut self, w: &mut World, g: &mut crate::mapui::guard::GuardScreen) {
        crate::strips::clear_lost_shade(); // GuardAssignScreen 0x132F2 reloads the strip.
        Session::new(w, &mut self.host).guard_close(g);
    }

    #[allow(clippy::too_many_arguments)]
    pub fn inventory(&mut self, w: &mut World, iso: &str, exe: &str, tileset: u8, faces: &[u8], table0: &mut [u8; 256],
                     input: &mut dyn InvInput) -> Option<i32> {
        // Op 344 cursor_to_soldier (a pick-up put an item on the cursor): the screen opens for that merc unless its
        // +0x188 = 1, without key I's other tests; the screen shows the merc that picked up, also when another merc
        // is selected (CursorItemToSoldier 0x54EC8 passes its soldier on).
        let picked = pick_shows_info(std::mem::take(&mut self.host.pick_info), self.host.pick_inv);
        let from_pick = self.host.pick_inv.is_some();
        let who = if let Some(k) = self.host.pick_inv.take() {
            let dead = w.soldiers.get(k as usize * crate::soldier::REC + 0x188).copied() == Some(1);
            if self.left || dead {
                eprintln!("pick up screen skipped: slot {} selected {}", k, w.selected);
                if dead && !self.left {
                    self.cursor_after(w, false, picked);
                }
                return None;
            }
            if k != w.selected as i32 {
                eprintln!("pick up screen for slot {} (selected {})", k, w.selected);
            }
            k as usize
        } else if self.left || !can_open(&w.soldiers, w.selected, w.combat, w.update.screen_drawn) {
            return None;
        } else {
            w.selected as usize
        };
        self.sound_files();
        let mut blink = self.inv_blink;
        let mut screen = InvScreen::new(iso, exe, tileset, faces, table0, &mut blink, input, who);
        screen.opt_info = crate::ctlpanel::get(w, 6) != 0;
        screen.opt_sub = crate::ctlpanel::subtitles(w);
        // Fan fix F66 (#70 row 90; 1.15m CursorItemToSoldier? 0x54EE9 calls cave 0x2B6A1): the pick-up's screen runs
        // with screen_drawn 0x92DCC = 1 and puts the old value back after, so a merc that an enemy interrupted in the
        // pick-up sees the pointer (changelog 1.15 jan 08, found by DarzaR). 1.13b: the byte stays (0 in the
        // enemies' turn: no pointer, invscreen::pointer_parts).
        let drawn = w.update.screen_drawn;
        w.update.screen_drawn = pick_screen_drawn(drawn, from_pick, ja1logic::fanfix::on(66));
        screen.ready = w.update.screen_drawn != 0;
        let r = Session::new(w, &mut self.host).inventory(&mut screen, who, picked);
        w.update.screen_drawn = drawn;
        // InvScreenRun? 0x54DD3 reloads the top strip when 0x92DDF < 2.
        crate::strips::inv_end_strip(w.opt_onscreen);
        self.inv_last = Some(screen.frame.clone());
        drop(screen);
        self.inv_blink = blink;
        crate::invscreen::wrapper_end(w.opt_onscreen, &mut w.panel_ends);
        self.cursor_after(w, true, picked);
        Some(r)
    }

    /// The leave path that waited for the pick-up's inventory screen (`leave_held`); the game stops until the screen ran.
    pub fn leave_late(&mut self, w: &mut World) {
        if self.leave_held && self.host.pick_inv.is_none() {
            self.leave_held = false;
            self.host.squad_play = self as *mut Play;
            self.host.squad_world = w as *mut World;
            Session::new(w, &mut self.host).leave_path();
            self.host.squad_play = std::ptr::null_mut();
            self.host.squad_world = std::ptr::null_mut();
            self.left = true;
        }
    }

    /// 0x54D50 and 0x54EC8 after the inventory screen: see invscreen::cursor_after.
    fn cursor_after(&mut self, w: &mut World, screen_ran: bool, picked: bool) {
        let held = w.held.first().copied().unwrap_or(0);
        let (flag, to) = crate::invscreen::cursor_after(held, w.inv.cursor_drop, self.mouse_px.0, screen_ran, picked);
        w.inv.cursor_drop = flag;
        if let Some(p) = to {
            self.host.mouse_to = Some(p);
            self.mouse_px = p;
        }
    }

    /// sound.md: the game's files give the sound (once).
    fn sound_files(&mut self) {
        if !self.host.sound.has_files() {
            if let Some((iso, exe)) = self.files.as_ref() {
                let live = self.host.sound.live;
                self.host.sound = crate::sound::Sound::with_files(iso, exe);
                self.host.sound.live = live;
                self.host.sound.wav = std::env::var_os("JA1_WAV").map(|_| Vec::new());
            }
        }
    }

    /// One pass of the play loop, the same for the window and `--script`: `ja1_loop_head`, the inputs (in order),
    /// `ticks` x `ja1_tick` (each followed by the front-end's animation timers), `ja1_frame`, `ja1_update_end`,
    /// `ja1_fuses`, `ja1_leave_tests`. Every step runs; the 8-bit sum of the results is tested at the end; a sum
    /// != 0 runs `ja1_leave_path` once and ends play.
    /// The caller draws after it. Nothing happens once the sector was left.
    /// panel.md "Other users of Lock": a box, balloon or button hold that Locks the game while `n` timer ticks
    /// pass (the soldiers and the turn stand still; the show timer, the blink timer and the tile marker's life
    /// run). `clock` = the clock switch of its Lock; `box_end` = the message box's end (cursor tile 0xC41).
    /// panel.md "The switch view", a left press on switch i of slot k: the switch flips, its pressed face shows
    /// and the press sound plays.
    pub fn switch_press(&mut self, w: &mut World, k: u8, i: usize) {
        let (at, sel) = (k as usize * REC, w.selected);
        crate::slotmouse::switch_press(&mut w.soldiers[at..at + REC], k, sel, i);
        self.sound_files();
        self.host.sound.sfx(0, 0x2B11, 0x32, 1);
    }

    /// panel.md "The switch view", the left button comes up after a press on switch i: the release sound, then
    /// Reserve AP for switches 0 and 1.
    pub fn switch_release(&mut self, w: &mut World, k: u8, i: usize) {
        self.host.sound.sfx(1, 0x2B11, 0x32, 1);
        if crate::slotmouse::switch_reserves(i) {
            Session::new(w, &mut self.host).reserve_ap(k as i32);
        }
    }

    /// The window: the mouse moved, so the merc slots take one pass at once (the hover frame and the press read them).
    pub fn slots_moved(&mut self, w: &mut World) {
        let mut s = Session::new(w, &mut self.host);
        s.slot_mouse(&mut self.slots, self.slot_pt);
        self.cont = self.slots.cont;
    }

    pub fn locked_ticks(&mut self, w: &mut World, clock: bool, n: u64, box_end: bool) {
        let mut s = Session::new(w, &mut self.host);
        s.lock(&mut self.pause, clock);
        timer_ticks(&mut s, &mut self.pause, &mut self.cursor, &mut self.scroll, &mut self.day.lines, n);
        s.unlock(&mut self.pause);
        if box_end {
            self.cursor.tile = 0xC41;
        }
    }

    /// The time that the inventory screen stayed open (section 8 "Inventory screen", window): `locked` ticks, then
    /// `open` ticks after the first message box in it.
    /// 1.13b: UseInventoryItem? 0x53B06 Lock?(0) takes the round lock; a message box (ShowMessage? Lock? + Unlock?
    /// 0x27CCA) frees it again because the screen's lock_owned 0x92DE2 is still set, so the rest of the screen's time
    /// is plain time (fuses, turn timers); 0x54D0E Unlock? then does nothing.
    /// Fan fix F65 (#70 row 73; 1.15m calls cave 0x42780 at both places instead, `xor byte [0x92DE1], 1`): the
    /// screen sets round_lock2 for its time, so TacticalTimer? 0x5EEA4 does nothing (no fuse, turn or blink timer),
    /// and the pause blink byte stays as it was (changelog 1.15l, found by DarzaR).
    pub fn inv_ticks(&mut self, w: &mut World, locked: u64, open: u64) {
        self.inv_ticks_with(w, locked, open, ja1logic::fanfix::on(65))
    }

    pub fn inv_ticks_with(&mut self, w: &mut World, locked: u64, open: u64, fan65: bool) {
        if fan65 {
            w.update.round_lock2 ^= 1;
            let mut s = Session::new(w, &mut self.host);
            timer_ticks(&mut s, &mut self.pause, &mut self.cursor, &mut self.scroll, &mut self.day.lines, locked + open);
            drop(s);
            w.update.round_lock2 ^= 1;
            return;
        }
        let mut s = Session::new(w, &mut self.host);
        s.lock(&mut self.pause, false);
        timer_ticks(&mut s, &mut self.pause, &mut self.cursor, &mut self.scroll, &mut self.day.lines, locked);
        if open > 0 {
            // the box's Lock? (round lock already taken: no change) and Unlock? (lock_owned set: frees it)
            s.lock(&mut self.pause, false);
            s.unlock(&mut self.pause);
            timer_ticks(&mut s, &mut self.pause, &mut self.cursor, &mut self.scroll, &mut self.day.lines, open);
        }
        s.unlock(&mut self.pause);
    }

    /// After a pass: the front-end's timers of `n` locked balloon ticks whose library Timer tick ran inside the
    /// pass (`PlayHost::timer_in_pass`).
    pub fn locked_ticks_ui(&mut self, w: &mut World, n: u64) {
        let mut s = Session::new(w, &mut self.host);
        s.lock(&mut self.pause, false);
        timer_ticks_in(&mut s, &mut self.pause, &mut self.cursor, &mut self.scroll, &mut self.day.lines, n, Some(false));
        s.unlock(&mut self.pause);
    }

    /// After a pass: the front-end's timers of `n` release-wait ticks whose library Timer tick ran inside the pass.
    /// The tile and effect animation timers of those ticks ran in the wait too (`PlayHost::timer_in_pass`).
    pub fn drag_ticks_ui(&mut self, w: &mut World, n: u64) {
        let mut s = Session::new(w, &mut self.host);
        timer_ticks_in(&mut s, &mut self.pause, &mut self.cursor, &mut self.scroll, &mut self.day.lines, n, None);
        drop(s);
        w.tile_frames = self.host.anims.sprite_frames();
    }

    /// world_load.md section 8 "The slide in time": the game time of the view slides of the last pass. The timer
    /// runs while the view slides (no frames): `ticks` plain ticks, then ripple_timer = 50 (the last step set it).
    /// Script mode logs `slide X0 Y0 X1 Y1 STEPS TICKS`.
    pub fn slide_ticks(&mut self, w: &mut World, slides: &[crate::focus::Slide]) {
        for sl in slides {
            self.day.lines.push(format!("slide {} {} {} {} {} {}", sl.from.0, sl.from.1, sl.target.0, sl.target.1, sl.steps, sl.ticks()));
            let mut s = Session::new(w, &mut self.host);
            timer_ticks(&mut s, &mut self.pause, &mut self.cursor, &mut self.scroll, &mut self.day.lines, sl.ticks() as u64);
            drop(s);
            w.update.ripple_timer = 50;
        }
    }

    pub fn pass(&mut self, w: &mut World, inputs: &[Input], ticks: u32) {
        // soldier_update.md "Squad lost": host op 188 in this pass moves to sector 59 at once.
        self.host.squad_play = self as *mut Play;
        self.host.squad_world = w as *mut World;
        // cursor.md: overlay_up is the busy display 0x92DCE (op 72 stop_walk): no cursor and no hover during a walk.
        self.cursor.overlay_up = w.update.view_hold;
        self.host.mouse_now = self.mouse_px;
        // TacticalKeyHandler 0x134A0: the poll runs only with 0x92DCC set.
        if w.update.screen_drawn != 0 {
            crate::pointer::mouse_poll(self.mouse_px);
        }
        self.pass_inner(w, inputs, ticks);
        if let Some(p) = self.host.turn_warp.take() {
            self.mouse_px = p;
            self.mouse_warp = Some(p);
        }
        self.cursor.overlay_up = w.update.view_hold;
        self.host.squad_play = std::ptr::null_mut();
        self.host.squad_world = std::ptr::null_mut();
    }

    /// TacticalKeyHandler 0x1409B..0x141A8: the Alt, Ctrl and Shift keys. Ctrl holds the hand cursor.
    fn key_flags_step(s: &mut Session, cursor: &mut crate::cursor::CursorState, latches: &mut crate::cursor::KeyLatches, flags: u16) {
        let slot = s.w.selected as usize;
        match crate::cursor::key_flags_pass(cursor, latches, flags) {
            crate::cursor::CtrlCall::None => {}
            // 0x1411B calls MoveCursorState? 0x1C464 for any selected value (also 0x40 = none): in mode 3 it reads no
            // record, clears the cursor tile and the AP and sets the hand 0x94 (move_cursor_state reads 0 past 48).
            crate::cursor::CtrlCall::Hand => s.cursor_state_move(cursor, slot),
            crate::cursor::CtrlCall::Back(m) => s.cursor_mode_set(cursor, slot as i32, m as i32),
        }
        // TacticalClick reads this latch 0x284F1F: with Ctrl held the click sets no cursor mode (live, #31).
        s.cw.key_ctrl = latches.ctrl as u8;
    }

    fn pass_inner(&mut self, w: &mut World, inputs: &[Input], ticks: u32) {
        if self.leave_held {
            self.leave_late(w);
        }
        if self.left || self.leave_held {
            return;
        }
        if self.keep_frame {
            self.frame_world = Some((Box::new(w.clone()), self.day.tileset));
        }
        self.sound_files();
        if self.host.sound.bank_stale {
            self.host.sound.load_tactical(&w.soldiers);
            // music.md "Who plays what": leaving the island map stops the music; a sector entry loads DAY and
            // plays theme 1.
            crate::music::with(|m| m.stop_theme());
            crate::music::screen("DAY", 1);
        }
        // music.md "The tactical theme rule" (enemies alive in the sector, sector enemies in sight).
        let enemies = w.sector_state.get(w.cur_sector as usize * 9 + 1).copied().unwrap_or(0);
        let seen = (0x28..0x30usize)
            .filter(|k| {
                let s = &w.soldiers[k * 512..(k + 1) * 512];
                s[0x182] != 0 && s[0x138] != 0 && s[0x188] == 0 && (s[0x9C] as i8) > 0
            })
            .count();
        crate::music::with(|m| m.tactical(enemies, seen as u32, w.santino_spoke != 0));
        self.host.sound.pass(ticks);
        // panel.md "View modes": in mode 0 the overview mouse step takes the place of the hover and the clicks.
        let overview = w.opt_onscreen == 0;
        // cursor.md "Hover": redraw_off is the view mode byte.
        self.hover.redraw_off = w.opt_onscreen;
        // select.md "Panel view (op 196)" reads the mouse pointer's y.
        self.host.select.panel = None;
        self.host.select.mouse_y = self.mouse_px.1;
        let mut s = Session::new(w, &mut self.host);
        // world_load.md section 8 "Loop head": before the inputs (also restarts the fast timer).
        let r0 = s.loop_head();
        // A cursor_select of the previous pass's ticks (an interrupt) lands before this pass's inputs.
        s.cursor_select_apply(&mut self.cursor);
        let mut clicked = false;
        let mut status = None;
        // Section 8 "Click": the mouse off the tile of an open confirm answers it at once.
        if let Some(crate::cursor::ClickWait::Confirm { tile } | crate::cursor::ClickWait::Walk { tile, .. }) = self.wait {
            if let Some(a) = crate::cursor::confirm_poll(tile, self.mouse, false, false, false) {
                if let (Some(wt), Some(g)) = (self.wait.take(), self.gv_t3.as_deref()) {
                    self.wait = s.answer(&mut self.cursor, &mut self.hover, g, wt, a).wait;
                }
            }
        }
        // TacticalKeyHandler: the mouse on the merc slots, at each pass (the window sets slot_pt).
        s.slot_mouse(&mut self.slots, self.slot_pt);
        self.cont = self.slots.cont;
        // A pass that starts paused: its view keys only end the pause (they run after the other inputs).
        let was_paused = self.pause.paused;
        let mut flags_done = std::mem::take(&mut self.drag_tail_done);
        for i in inputs {
            // TacticalKeyHandler runs the key flags at 0x1409B, after the key and before the mouse (0x141EC on).
            if !flags_done && self.wait.is_none() && i.is_mouse() {
                Self::key_flags_step(&mut s, &mut self.cursor, &mut self.key_latches, self.key_flags);
                flags_done = true;
            }
            // cursor.md "Clicks": the window's right press starts a hold only with no wait open; an open wait (the
            // aim: one more AP per right click; a confirm) takes it as a right click (the maintainer play test #14 C).
            let rclick;
            let i = match (i, self.wait) {
                (Input::RHold { tile, shift, alt, .. }, Some(w)) if !matches!(w, crate::cursor::ClickWait::Hold(_)) => {
                    rclick = Input::RClick { tile: *tile, shift: *shift, alt: *alt };
                    &rclick
                }
                _ => i,
            };
            // Section 8 "Click": while a wait is open, keys other than Escape do nothing.
            if self.wait.is_some() && !matches!(*i, Input::Click { .. } | Input::RClick { .. } | Input::Drag { .. } | Input::AllMove { .. } | Input::Escape | Input::Poll { .. }) {
                continue;
            }
            // panel.md "Pause (key P) and the clock": while paused, any key or click only ends the pause.
            if self.pause.paused && !matches!(*i, Input::Poll { .. }) {
                s.pause(&mut self.pause, false);
                continue;
            }
            // turns.md "Time compression": while it is on, any other key or click only turns it off.
            if *i != Input::Compress && s.compressing() {
                s.time_compress(0);
                continue;
            }
            match *i {
                Input::Compress => s.compress_key(),
                Input::Pause => s.pause(&mut self.pause, true),
                // cursor.md "Tile popups": key Z at the mouse tile, in every view mode but the overview.
                Input::Popup => {
                    if !overview {
                        if let Some(t) = self.mouse {
                            self.popup = Some((t, true));
                        }
                    }
                }
                Input::SayAgain => self.again = true,
                Input::Other => {}
                Input::ForceTurn => s.force_turn(),
                Input::Debug(code) => self.debug_msgs = s.debug_key(code, self.mouse),
                Input::Click { tile, shift, alt } | Input::RClick { tile, shift, alt } | Input::Drag { tile, shift, alt, .. }
                | Input::AllMove { tile, shift, alt } => {
                    // 0x141EC: the enemies' turn takes no press on the map.
                    if !crate::slotmouse::pass_runs(s.w.update.screen_drawn) {
                        continue;
                    }
                    let right = matches!(*i, Input::RClick { .. });
                    s.squad = matches!(*i, Input::AllMove { .. }) as i32;
                    let drag = if let Input::Drag { dir, .. } = *i { dir } else { 0 };
                    clicked = true;
                    let Some(g) = self.gv_t3.as_deref() else { continue };
                    // The mouse came to T before the press: off the tile of an open confirm, that answers it first.
                    if let Some(crate::cursor::ClickWait::Confirm { tile: t0 } | crate::cursor::ClickWait::Walk { tile: t0, .. }
                                | crate::cursor::ClickWait::Posture(crate::cursor::Hold { tile: t0, .. })) = self.wait {
                        if let Some(a @ crate::cursor::Answer::Off) = crate::cursor::confirm_poll(t0, Some(tile), false, false, false) {
                            let wt = self.wait.take().expect("wait");
                            let o = s.answer(&mut self.cursor, &mut self.hover, g, wt, a);
                            self.wait = Self::posture_end(&mut s, &mut self.cursor, &mut self.hover, &mut self.day.lines, self.right_held, g, o).wait;
                        }
                    }
                    let mut out = if let Some(wt) = self.wait.take() {
                        // The press in an open confirm is one poll of WaitConfirmClick? 0x1C514 with the mouse on `tile`.
                        let a = match wt {
                            crate::cursor::ClickWait::Confirm { tile: t0 } | crate::cursor::ClickWait::Walk { tile: t0, .. }
                            | crate::cursor::ClickWait::Posture(crate::cursor::Hold { tile: t0, .. }) => {
                                crate::cursor::confirm_poll(t0, Some(tile), !right, right, false).expect("a press answers")
                            }
                            _ if right => crate::cursor::Answer::Right,
                            _ => crate::cursor::Answer::Left(tile),
                        };
                        let o = s.answer(&mut self.cursor, &mut self.hover, g, wt, a);
                        if right && matches!(wt, crate::cursor::ClickWait::Aim { .. }) {
                            eprintln!("aim ap {}", self.cursor.ap);
                            // sound.md "Click sound": each right click of the aim (AimLoop 0x1D7F4)
                            s.click_sound();
                        }
                        Self::posture_end(&mut s, &mut self.cursor, &mut self.hover, &mut self.day.lines, self.right_held, g, o)
                    } else if overview {
                        // `tile` is the overview tile under the mouse.
                        self.mouse = Some(tile);
                        s.set_keys(shift, alt);
                        let (o, jump) = s.overview_mouse(&mut self.cursor, &mut self.hover, g, &mut self.ov, tile,
                                                         if right { 2 } else { 1 }, shift, alt, drag);
                        if jump {
                            self.ov_jump = Some(tile);
                        }
                        o
                    } else {
                        // Section 8 "Click": the mouse comes to the tile one frame before the press.
                        if self.mouse != Some(tile) {
                            self.mouse = Some(tile);
                            s.hover(&mut self.cursor, &mut self.hover, g, tile, shift, alt);
                        }
                        s.set_keys(shift, alt);
                        let o = s.press(&mut self.cursor, &mut self.hover, g, tile, shift, alt, right, 100, drag);
                        if matches!(o.wait, Some(crate::cursor::ClickWait::Aim { .. })) {
                            // The aim starts with the click sound (UseItemAt? 0x1DFFA, before AimLoop).
                            s.click_sound();
                        }
                        o
                    };
                    s.squad = 0;
                    // 0x1D0C8: the loop's release left tile 0xC41. A walk confirm sets the tile again in its own step.
                    if std::mem::take(&mut self.drag_end) && out.wait.is_none() {
                        self.cursor.tile = crate::drag::TILE_OFF;
                    }
                    // A right click is a hold of one poll with both buttons up.
                    if let Some(h @ crate::cursor::ClickWait::Hold(_)) = out.wait.filter(|_| right) {
                        let p = crate::cursor::Answer::Poll { left: false, right: false, y: 100 };
                        let o2 = s.answer(&mut self.cursor, &mut self.hover, g, h, p);
                        Self::hold_line(&mut self.day.lines, &self.cursor, &o2, h);
                        out = crate::cursor::ClickOut { status: out.status.or(o2.status), ..o2 };
                    }
                    self.wait = out.wait;
                    status = out.status.or(status);
                    if let Some((k, t)) = s.take_marker() {
                        put_marker(&mut self.cursor, &mut self.day.lines, k, t);
                    }
                    // ClearMarksBeforeQuote 0x11418: HidePathMarks(0) clears the AP number 0x229DEC and the cursor
                    // tile 0x229D2C; over_map 0x284F13 and the mouse wobble 0x22A47E / 0x22A47C go to 0.
                    if s.take_cursor_reset() {
                        crate::cursor::clear_marks_before_quote(&mut self.cursor);
                    }
                    // RedrawMercPortrait 0x336B4: the selected soldier's stop clears the cursor tile 0x229D2C.
                    if s.take_marker_clear() && self.cursor.tile != 0xC41 {
                        self.cursor.tile = 0xC41;
                        self.day.lines.push("cursor tile off".into());
                    }
                    // CursorModeSet? 0x1C438 (host op 185 click_panel of TacticalClick, SoldierDies, StoryFind).
                    if let Some((slot, mode)) = s.take_click_panel() {
                        s.apply_click_panel(&mut self.cursor, slot, mode);
                    }
                    if let Some(t) = out.all_marker {
                        put_marker(&mut self.cursor, &mut self.day.lines, 0x0F, t);
                    }
                    if let Some(t) = out.popup {
                        self.popup = Some((t, false));
                    }
                    if let Some(o) = out.debug_pages {
                        s.debug_open(o);
                    }
                    // cursor.md "Drag to an edge": the release ends the walk with the traverse.
                    if let Some(dir) = out.traverse {
                        let (v, to) = s.traverse(dir);
                        self.day.lines.push(format!("drag {} result {}", ["N", "E", "S", "W"][(dir - 10) as usize & 3], v));
                        if v == 1 {
                            drop(s);
                            self.traversed = Some(to);
                            self.passes += 1;
                            return;
                        }
                    }
                }
                Input::RHold { tile, shift, alt, y } => {
                    // 0x141EC: UseItemAt 0x1D9B8 has its two calls behind this gate, so the enemies' turn takes no right press.
                    if !crate::slotmouse::pass_runs(s.w.update.screen_drawn) {
                        continue;
                    }
                    eprintln!("right hold {}", tile);
                    clicked = true;
                    let Some(g) = self.gv_t3.as_deref() else { continue };
                    if self.wait.is_some() {
                        continue;
                    }
                    if overview {
                        // The overview has no hold: the right press is the overview click.
                        self.ov_jump = Some(tile);
                        continue;
                    }
                    if self.mouse != Some(tile) {
                        self.mouse = Some(tile);
                        s.hover(&mut self.cursor, &mut self.hover, g, tile, shift, alt);
                    }
                    s.set_keys(shift, alt);
                    self.right_held = true;
                    let out = s.press(&mut self.cursor, &mut self.hover, g, tile, shift, alt, true, y, 0);
                    self.wait = out.wait;
                    if let Some(t) = out.popup {
                        self.popup = Some((t, false));
                    }
                    status = out.status.or(status);
                }
                Input::Poll { left, right, y } => {
                    self.right_held = right;
                    let (Some(g), Some(h @ crate::cursor::ClickWait::Hold(_))) = (self.gv_t3.as_deref(), self.wait) else { continue };
                    let out = s.answer(&mut self.cursor, &mut self.hover, g, h, crate::cursor::Answer::Poll { left, right, y });
                    Self::hold_line(&mut self.day.lines, &self.cursor, &out, h);
                    self.look_view |= out.look;
                    self.wait = out.wait;
                    status = out.status.or(status);
                }
                Input::Escape => {
                    if let (Some(wt), Some(g)) = (self.wait.take(), self.gv_t3.as_deref()) {
                        let out = s.answer(&mut self.cursor, &mut self.hover, g, wt, crate::cursor::Answer::Escape);
                        let out = Self::posture_end(&mut s, &mut self.cursor, &mut self.hover, &mut self.day.lines, self.right_held, g, out);
                        self.wait = out.wait;
                    } else {
                        s.escape_stop();
                    }
                }
                Input::EndTurn => {
                    s.end_turn();
                }
                Input::NextMerc => s.next_merc(),
                Input::NextEnemy => s.next_enemy(),
                Input::Version => status = Some(self.version.to_string()),
                Input::Volume(k) => {
                    if crate::sound::press_volume_key(k).is_some() {
                        let v = crate::sound::volumes();
                        self.day.lines.push(format!("volume music {} sound {}", v.music, v.sound));
                    }
                }
                Input::Swap => {
                    if let Some(t) = self.mouse {
                        s.swap_places(t as i32);
                    }
                }
                Input::Merc(k, alt) => {
                    // #70 F60 (row 87): 1.15m TacticalKeyHandler 0x136B3..0x1377B call tail 0x8AEED: no merc
                    // switch while an item is on the cursor (0x222B50 != 0), so it cannot move to another merc.
                    if ja1logic::fanfix::on(60) && s.w.held[0] != 0 {
                        self.day.lines.push(format!("merc key {} held", k));
                        continue;
                    }
                    choose_merc_slots(&mut s, &mut self.slots, k as usize, alt);
                    self.cont = self.slots.cont;
                }
                // SlotMouse 0x11714 left button on a face: "cont" shown = go on with the order, else choose the merc
                // (SelectMerc (k, Alt held, quote 1), 0x11B18).
                Input::Face(k) => {
                    self.slot_pt = (k < 8).then(|| crate::slotmouse::face_point(k, s.w.soldiers[k as usize * crate::soldier::REC + 0x180]));
                    s.slot_mouse(&mut self.slots, self.slot_pt);
                    self.cont = self.slots.cont;
                    self.day.lines.push(format!("face {} cont {}", k, (self.cont < 8) as u8));
                }
                Input::Portrait(k, alt) => {
                    // A press is one pass on the face of slot k, then the left button (SlotMouse 0x1196C).
                    use crate::slotmouse::{face_point, slot_takes_mouse, Act};
                    if !crate::slotmouse::pass_runs(s.w.update.screen_drawn) {
                        continue;
                    }
                    let rec = k as usize * crate::soldier::REC;
                    s.slot_mouse(&mut self.slots, Some(face_point(k, s.w.soldiers[rec + 0x180])));
                    let r = &s.w.soldiers[rec..rec + crate::soldier::REC];
                    let act = if slot_takes_mouse(r) { self.slots.left(k, r, s.w.selected) } else { None };
                    match (act, self.gv_t3.as_deref()) {
                        (Some(Act::Cont), Some(g)) => {
                            self.slots.cont = crate::slotmouse::NONE;
                            let out = s.cont(&mut self.cursor, &mut self.hover, g);
                            self.day.lines.push(format!("cont slot {} status {:?}", k, out.status));
                            status = out.status.or(status);
                        }
                        (Some(Act::Select), _) => {
                            // #70 F61 (row 87): 1.15m SlotMouse 0x11B24 calls cave 0x8AF04: CursorItemBack? 0x53660
                            // puts a held item back into the old merc's box before SelectMerc? 0x1148C.
                            if crate::slotmouse::cursor_back_first() {
                                s.cursor_back();
                            }
                            choose_merc_slots(&mut s, &mut self.slots, k as usize, alt);
                        }
                        _ => {}
                    }
                    self.cont = self.slots.cont;
                    // 0x119AE: every road of the left arm ends in WaitLeftUp 0x67C54: the game stops until the release (live, issue #31).
                    if act.is_some() {
                        self.face_press = true;
                    }
                }
                Input::SeesLine => s.sees_key(),
                Input::View(_) | Input::Arrow(_) => {}
                Input::Strip { x, right, ctrl } => {
                    // 0x141EC, and 0x1462A again before StripClick 0x54F3C: the enemies' turn takes no press on the strip.
                    if !crate::slotmouse::pass_runs(s.w.update.screen_drawn) {
                        continue;
                    }
                    if let Some(g) = self.gv_t3.as_deref() {
                        s.strip_click(&mut self.cursor, &mut self.hover, g, x, right, ctrl);
                    }
                }
                Input::Abort { restart } => {
                    // Play stops at once: the rest of the pass (ticks, frame, update end, fuses, leave tests)
                    // does not run, and neither does the leave path.
                    drop(s);
                    self.aborted = Some(restart);
                    self.passes += 1;
                    return;
                }
                Input::Traverse(dir) => {
                    if !s.can_traverse() {
                        continue;
                    }
                    let (v, out) = s.traverse(dir);
                    if v == 1 {
                        // The squad leaves: the rest of the pass does not run (the move loads the new sector).
                        drop(s);
                        self.traversed = Some(out);
                        self.passes += 1;
                        return;
                    }
                }
            }
            // select.md Select soldier step 3: op 200 cursor_select puts the cursor in walk mode (1, 0x8E), also
            // when the merc was chosen by key or the hand emptied in weapon mode.
            s.cursor_select_apply(&mut self.cursor);
        }
        // Section 8 "Click": while an aim or confirm is open the pass runs only its inputs.
        if self.wait.is_some() {
            drop(s);
            for t in status.iter().flat_map(|t| t.split('\n')) {
                self.host.status_msg(t.to_string());
            }
            self.cursor.combat = w.combat;
            self.passes += 1;
            return;
        }
        // A pass with no mouse input runs the key step here.
        if !flags_done {
            Self::key_flags_step(&mut s, &mut self.cursor, &mut self.key_latches, self.key_flags);
        }
        // TacticalKeyHandler 0x13490, the mouse off the map (asm 0x14308): the frame the mouse leaves the map,
        // unlight the lit portrait, forget the name, hide the path marks, clear the mouse wobble and the cursor
        // tile, and set over_map = 0. The name over the merc and the lit face frame go in this same frame.
        // 0x141EC: in the enemies' turn neither this step nor the hover step runs.
        let mouse_steps = crate::slotmouse::pass_runs(s.w.update.screen_drawn);
        if self.mouse.is_none() && mouse_steps {
            s.off_map(&mut self.cursor, &mut self.hover);
        }
        // Section 8 "Hover": after the inputs, before the ticks (a click took the hover step's place).
        if let (Some(t), false, false, true, Some(g)) = (self.mouse, self.mouse_held, clicked, mouse_steps, self.gv_t3.as_deref()) {
            if overview {
                s.overview_mouse(&mut self.cursor, &mut self.hover, g, &mut self.ov, t, 0, self.shift_alt.0, self.shift_alt.1, 0);
            } else {
                let n = hover_steps(self.hover_per_tick, ticks);
                for _ in 0..n {
                    s.hover(&mut self.cursor, &mut self.hover, g, t, self.shift_alt.0, self.shift_alt.1);
                }
            }
        }
        if let Some((k, t)) = s.take_marker() {
            put_marker(&mut self.cursor, &mut self.day.lines, k, t);
        }
        // ClearMarksBeforeQuote 0x11418: HidePathMarks(0) clears the AP number 0x229DEC and the cursor tile
        // 0x229D2C; over_map 0x284F13 and the mouse wobble 0x22A47E / 0x22A47C go to 0.
        if s.take_cursor_reset() {
            crate::cursor::clear_marks_before_quote(&mut self.cursor);
        }
        // RedrawMercPortrait 0x336B4: the selected soldier's stop clears the cursor tile 0x229D2C.
        if s.take_marker_clear() && self.cursor.tile != 0xC41 {
            self.cursor.tile = 0xC41;
            self.day.lines.push("cursor tile off".into());
        }
        // CursorModeSet? 0x1C438 (host op 185 click_panel of TacticalClick, SoldierDies, StoryFind).
        if let Some((slot, mode)) = s.take_click_panel() {
            s.apply_click_panel(&mut self.cursor, slot, mode);
        }
        timer_ticks(&mut s, &mut self.pause, &mut self.cursor, &mut self.scroll, &mut self.day.lines, ticks as u64);
        // panel.md "Pause (key P) and the clock": the loop's blink step.
        self.pause.pass();
        s.fuse_beep_step();
        if let Some(d7) = s.story_step() {
            if self.hover.lit == d7 {
                self.hover.lit = 0x40;
            }
        }
        s.santino_step();
        let r1 = s.frame();
        s.effect_hits();
        let r2 = s.update_end();
        s.fuses();
        // soldier_update.md "Squad lost": op 188 outside a pass's library calls (squad_play not set) moves here,
        // before the leave tests (they test the new sector, as after the death).
        if s.squad_lost() {
            drop(s);
            self.host.squad_lost = false;
            if let Err(m) = self.squad_lost_move(w) {
                eprintln!("squad lost: {}", m);
            }
            s = Session::new(w, &mut self.host);
        }
        if let Some((k, t)) = s.take_marker() {
            put_marker(&mut self.cursor, &mut self.day.lines, k, t);
        }
        // ClearMarksBeforeQuote 0x11418: HidePathMarks(0) clears the AP number 0x229DEC and the cursor tile
        // 0x229D2C; over_map 0x284F13 and the mouse wobble 0x22A47E / 0x22A47C go to 0.
        if s.take_cursor_reset() {
            crate::cursor::clear_marks_before_quote(&mut self.cursor);
        }
        // RedrawMercPortrait 0x336B4: the selected soldier's stop clears the cursor tile 0x229D2C.
        if s.take_marker_clear() && self.cursor.tile != 0xC41 {
            self.cursor.tile = 0xC41;
            self.day.lines.push("cursor tile off".into());
        }
        // CursorModeSet? 0x1C438 (host op 185 click_panel of TacticalClick, SoldierDies, StoryFind).
        if let Some((slot, mode)) = s.take_click_panel() {
            s.apply_click_panel(&mut self.cursor, slot, mode);
        }
        // MoveMercTo 0x1C7A8: the walk end's TacticalClick (ActiveFrame 0x1BFD9) with the mine detector in hand left
        // its move order: the confirm wait opens now (the original waits inside the pass), or the merc moves.
        if let Some((slot, t)) = s.take_detector_wait() {
            if detector_wait_opens(slot, s.selected(), self.wait.is_some()) {
                if let Some(g) = self.gv_t3.as_deref() {
                    let out = s.detector_pass(&mut self.cursor, &mut self.hover, g, t);
                    self.day.lines.push(format!("detector pass {} wait {}", t, out.wait.is_some() as u8));
                    self.wait = out.wait;
                    status = out.status.or(status);
                }
            }
        }
        let r3 = s.leave_tests();
        let sum = (r0 as u8).wrapping_add(r1 as u8).wrapping_add(r2 as u8).wrapping_add(r3 as u8);
        if sum != 0 {
            if s.pick_screen_due() {
                // Op 344 runs the screen inside the pick-up in the original (CursorItemToSoldier? 0x54EC8, before
                // CursorItemBack? 0x11370 of the leave); here it runs after the pass, so the leave waits for it.
                self.leave_held = true;
            } else {
                // world_load.md section 8: the sector ends; Leave path runs once, then play stops.
                s.leave_path();
                self.left = true;
            }
        }
        drop(s);
        for t in status.iter().flat_map(|t| t.split('\n')) {
            self.host.status_msg(t.to_string());
        }
        self.small_panel(w, ticks, self.mouse.is_some() && !overview);
        // panel.md "View modes": the keys of this pass, then the end of a look.
        for i in inputs.iter().filter(|_| !was_paused) {
            if let Input::View(k) = *i {
                self.view_key(w, k);
            }
        }
        if std::mem::take(&mut self.look_view) {
            self.view_key(w, 0);
        }
        if !was_paused {
            self.scroll_pass(w, inputs);
        }
        if let Some(t) = self.ov_jump.take() {
            self.overview_jump(w, t);
        }
        // Section 8 "Hover": the cursor's combat byte is the world's.
        self.cursor.combat = w.combat;
        // Op 219 save_tmp came in this pass: the front-end keeps the file with the sector files.
        if let Some((sec, f)) = self.host.saved_tmp.take() {
            if let Some(slot) = w.tmp_files.get_mut(sec as usize) {
                *slot = Some(f);
            }
        }
        self.passes += 1;
    }
}

/// The x of an office poll that is the test line `latch N`: it sets `invextra::latch` to N (y) when the polls run.
pub const LATCH_POLL: i32 = i32::MIN;

/// One command of a play script (section 8 "Play script").
#[derive(Clone, Debug, PartialEq)]
pub enum Cmd {
    /// N passes with no input.
    Wait(u32),
    /// A message box stays open MS ms (panel.md "Other users of Lock"): MS / 10 locked timer ticks.
    BoxWait(u32),
    /// Key S (panel.md "Key S: all stat popups"): the stat popups show MS ms, then a key ends them (MS / 10 locked
    /// timer ticks with clock switch 1).
    StatKey(u32),
    /// `switch K I.. [open]`: panel.md "The switch view" of slot K, presses on switches I.., then closed (or left
    /// open with `open`).
    Switch(u8, Vec<usize>, bool),
    /// One pass with this input.
    Do(Input),
    /// Key I: the inventory screen with these polls (the `inv` lines after it), then one pass.
    Inventory(Vec<InvPoll>),
    /// `pay X Y B A`: one poll of the morning's "Not enough funds" dialog (world_load.md section 9 step 1): mouse
    /// (X, Y), buttons B, A = the answer (0 / 1) to the question that this poll asks; the polls run at the next `day`.
    Pay(i32, i32, i32, bool, i32),
    /// (x = LATCH_POLL: the test line `latch N`.)
    Office(i32, i32, i32, Vec<u8>),
    /// A test line: the old debt of profile P (+0x3E, as a fire on the hire day leaves it, dismiss.rs `fire_yes`).
    AimDebt(usize, u16),
    /// `menu X Y B [K,K,...]`: a poll of the main menu screens (world_load.md section 12).
    Menu(i32, i32, i32, Vec<u8>),
    /// `eod N`: the end-of-day screen answers 1 at its poll number N (no pass of its own).
    Eod(u32),
    /// world_load.md section 9 step 3: the sunrise screen skips at poll N.
    Sunrise(u32),
    /// `day`: the day loop's steps 1..3 (section 9).
    Day,
    /// `map S MODE B`: a click on the day loop's island map.
    Map(i32, i32, i32),
    /// `amap X Y B`: the window's island map screen at game pixel (X, Y): B = 0 the mouse moves there, 1 a left
    /// press and release, 2 a right press and release; `amap key C` (C = a byte: 13 Enter, 27 Escape, 69 E ...).
    Amap(i32, i32, i32),
    /// `gmap X Y B` / `gmap key C` / `gmap open`: the guard screen (tactical key M) as `amap` (B 0 move, 1 left,
    /// 2 right press and release); Ctrl is the `ctrl` line's state. Only while the sector is not left.
    Gmap(i32, i32, i32),
    /// `start A`: Enter on the day loop's map; op 303 answers A.
    /// `start A [B]`: the answer A of op 303; B = 1: the start button (strategic.md "Start the day"), else Enter.
    Start(i32, i32),
    /// `clock`: the line `clock H:MM hours D minutes M` (no pass of its own).
    Clock,
    /// `practice`: per merc slot k 0..7 the line `practice k` + its nine practice counters (no pass of its own).
    Practice,
    /// sound.md "Output": a line `sound N1 N2 ...` with the samples started since the last one.
    Sound,
    /// music.md "Output and script line".
    Music,
    /// `blast T K`: check aid (no game input): the library's explosion (op 90) of kind K at map tile T, thrower 0,
    /// hit 1, as a set explosive that goes off there (no pass of its own).
    Blast(i32, i32),
    /// `mine TILE COND` (test setup): a set mine on TILE.
    Mine(i32, i32),
    /// Test line `gas TILE TURNS RADIUS`: gas cloud 0 (g_gasClouds 0x22A44E: u16 tile, i16 turns, i16 radius).
    Gas(i32, i32, i32),
    /// Test line `hp SLOT VALUE`: sets the health byte of a soldier.
    Hp(i32, i32),
    /// Test line `stop SLOT`: the writes of StopAnim (S, 0) 0x196F8, the stop of a new sighting in a walk (task kept).
    Stop(i32),
    /// Test only: `tile SLOT TILE` puts a soldier on a tile (enemies are not in a day save). The tile map moves with it.
    Tile(i32, i32),
    /// Test line `hit K HEIGHT DAMAGE`: HitLocationDamage on soldier K.
    Hit(i32, i32, i32),
    /// `seed N`: the random seed becomes N (validate/replay_week.py writes the same seed into the original).
    Seed(u32),
    ClockSeed(u32),
    FrameFree,
    SetupSeed(u32),
    CompressDay,
    /// `opt X Y [L]` / `opt esc`: one pass of the Control Panel (panel.md), the mouse at game pixel (X, Y), L = a
    /// left click; the first line opens it (no pass of the game meanwhile).
    Opt(i32, i32, bool, bool),
    Quick(bool),
    /// `item TILE TYPE COND [B6]` (test setup): a found item (count 1, byte 6 = B6, default 1 as the hand search leaves
    /// it; 0 = not seen yet) on TILE.
    Item(i32, i32, i32, i32),
    /// `corpse Y|N`: the answer to the next op 292 corpse_ask question; no pass of its own.
    Corpse(bool),
    /// `disarm Y|N`: the answer to the next op 345 disarm_ask question; no pass of its own.
    Disarm(bool),
    /// `raise up|down|ok|no ...`: the presses in the next op 297 salary box (0 NO, 1 OK, 2 down, 3 up); no pass of
    /// its own.
    Raise(Vec<usize>),
    /// `abort R`: key A answered Y, then R (true = restart) for "RESTART this day?", in one pass.
    Abort(bool),
    /// `trav N|E|S|W`: key T and the popup's pick (soldier_update.md "Traverse to a neighbour sector"), in one
    /// pass; with result 1 the move runs. Writes `trav D sector S tile T` or `trav D refused`.
    Trav(i32),
    /// `hover T`: the mouse over map tile T (-1 = off the map); no pass of its own.
    Hover(i32),
    /// `keyflags N`: the keyboard flags 0x22A46C from now on (a window key hold; 4 = left Ctrl).
    KeyFlags(u16),
    /// `pointer X Y`: the mouse at screen point (X, Y) (panel.md "Mouse-edge scroll"; X = -1: the mouse leaves the
    /// window); no pass of its own.
    Pointer(i32, i32),
    /// `dragto T DX DY`: a left press on T at screen point (160, 100), then the mouse at (160 + DX, 100 + DY) with
    /// the button still held (cursor.md "Drag to an edge"); writes `dragto dir D cursor V` (V = the drag cursor,
    /// 0 = the press value); no pass of its own. `dragto up` releases it and runs one pass with the release.
    DragTo(i32, i32, i32),
    DragUp,
    /// `dragto right`: the right button pressed and released while the left one is held (cursor.md "ALL-move");
    /// writes `allmove on` or `allmove refused`; no pass of its own.
    DragRight,
    /// `view T`: the window's arrows put the view's top left tile at T (view_x / view_y); no pass of its own.
    View(i32),
}

/// Parses a play script: one command per line (`wait N`, `click T`, `key K` with K = D or 1..8); a line that
/// starts with `#` (after blanks) is a comment; empty lines are skipped.
pub fn parse_script(text: &str) -> Result<Vec<Cmd>, String> {
    let mut out = Vec::new();
    for (n, line) in text.lines().enumerate() {
        let l = line.trim();
        if l.is_empty() || l.starts_with('#') {
            continue;
        }
        let bad = || format!("script line {}: {:?}", n + 1, line);
        let mut it = l.split_whitespace();
        let cmd = it.next().ok_or_else(bad)?;
        if cmd == "framefree" {
            out.push(Cmd::FrameFree);
            continue;
        }
        if cmd == "compressday" || cmd == "day" || cmd == "clock" || cmd == "practice" || cmd == "sound" || cmd == "music" {
            if it.next().is_some() {
                return Err(bad());
            }
            out.push(match cmd { "compressday" => Cmd::CompressDay, "day" => Cmd::Day, "clock" => Cmd::Clock, "sound" => Cmd::Sound, "music" => Cmd::Music, _ => Cmd::Practice });
            continue;
        }
        let arg = it.next().ok_or_else(bad)?;
        if cmd == "item" {
            let num = |v: Option<&str>| -> Result<i32, String> { v.ok_or_else(bad)?.parse().map_err(|_| bad()) };
            let (t, ty, c) = (num(Some(arg))?, num(it.next())?, num(it.next())?);
            let b6 = match it.next() {
                None => 1,
                v => num(v)?,
            };
            if it.next().is_some() {
                return Err(bad());
            }
            out.push(Cmd::Item(t, ty, c, b6));
            continue;
        }
        if cmd == "seed" {
            out.push(Cmd::Seed(arg.parse().map_err(|_| bad())?));
            continue;
        }
        if cmd == "clockseed" {
            out.push(Cmd::ClockSeed(arg.parse().map_err(|_| bad())?));
            continue;
        }
        if cmd == "stop" {
            out.push(Cmd::Stop(arg.parse().map_err(|_| bad())?));
            continue;
        }
        if cmd == "setupseed" {
            out.push(Cmd::SetupSeed(arg.parse().map_err(|_| bad())?));
            continue;
        }
        if cmd == "hit" || cmd == "gas" {
            let num = |v: Option<&str>| -> Result<i32, String> { v.ok_or_else(bad)?.parse().map_err(|_| bad()) };
            let (k, h, d) = (num(Some(arg))?, num(it.next())?, num(it.next())?);
            if it.next().is_some() {
                return Err(bad());
            }
            out.push(if cmd == "hit" { Cmd::Hit(k, h, d) } else { Cmd::Gas(k, h, d) });
            continue;
        }
        if cmd == "blast" || cmd == "mine" || cmd == "hp" || cmd == "tile" {
            let num = |v: Option<&str>| -> Result<i32, String> { v.ok_or_else(bad)?.parse().map_err(|_| bad()) };
            let (t, k) = (num(Some(arg))?, num(it.next())?);
            if it.next().is_some() {
                return Err(bad());
            }
            out.push(match cmd {
                "mine" => Cmd::Mine(t, k),
                "hp" => Cmd::Hp(t, k),
                "tile" => Cmd::Tile(t, k),
                _ => Cmd::Blast(t, k),
            });
            continue;
        }
        if cmd == "start" {
            let a = arg.parse().map_err(|_| bad())?;
            let b = match it.next() {
                None => 0,
                Some(v) => v.parse().map_err(|_| bad())?,
            };
            if it.next().is_some() {
                return Err(bad());
            }
            out.push(Cmd::Start(a, b));
            continue;
        }
        if cmd == "map" {
            let num = |v: Option<&str>| -> Result<i32, String> { v.ok_or_else(bad)?.parse().map_err(|_| bad()) };
            let (sec, mode, b) = (num(Some(arg))?, num(it.next())?, num(it.next())?);
            if it.next().is_some() {
                return Err(bad());
            }
            out.push(Cmd::Map(sec, mode, b));
            continue;
        }
        if cmd == "amap" {
            let num = |v: Option<&str>| -> Result<i32, String> { v.ok_or_else(bad)?.parse().map_err(|_| bad()) };
            // `amap latch N`: the test line `latch N` in order with the map polls (x = -2).
            // `amap ctrl N`: the Ctrl key is held (1) or not (0) for the next map presses, as `map.keys` in the window.
            let c = if arg == "key" {
                (-1, num(it.next())?, 0)
            } else if arg == "ctrl" {
                (-3, num(it.next())?, 0)
            } else if arg == "latch" {
                (-2, num(it.next())?.clamp(0, 3), 0)
            } else {
                (num(Some(arg))?, num(it.next())?, num(it.next())?)
            };
            if it.next().is_some() || !(0..=2).contains(&(c.2 & !48)) {
                return Err(bad());
            }
            out.push(Cmd::Amap(c.0, c.1, c.2));
            continue;
        }
        if cmd == "gmap" {
            let num = |v: Option<&str>| -> Result<i32, String> { v.ok_or_else(bad)?.parse().map_err(|_| bad()) };
            let c = match arg {
                "open" => (-2, 0, 0),
                "key" => (-1, num(it.next())?, 0),
                _ => (num(Some(arg))?, num(it.next())?, num(it.next())?),
            };
            if it.next().is_some() || !(0..=2).contains(&c.2) {
                return Err(bad());
            }
            out.push(Cmd::Gmap(c.0, c.1, c.2));
            continue;
        }
        if cmd == "pay" {
            let num = |v: Option<&str>| -> Result<i32, String> { v.ok_or_else(bad)?.parse().map_err(|_| bad()) };
            let (x, y, b, a) = (num(Some(arg))?, num(it.next())?, num(it.next())?, num(it.next())?);
            let ms = match it.next() {
                Some(v) => num(Some(v))?,
                None => 0,
            };
            if it.next().is_some() || !(0..=1).contains(&a) {
                return Err(bad());
            }
            out.push(Cmd::Pay(x, y, b, a == 1, ms));
            continue;
        }
        if cmd == "office" || cmd == "slots" || cmd == "menu" || cmd == "aim" {
            let num = |v: Option<&str>| -> Result<i32, String> { v.ok_or_else(bad)?.parse().map_err(|_| bad()) };
            let (x, y, b) = (num(Some(arg))?, num(it.next())?, num(it.next())?);
            let keys = match it.next() {
                None => vec![],
                Some(k) => k.split(',').map(|v| v.parse::<u8>().map_err(|_| bad())).collect::<Result<Vec<u8>, String>>()?,
            };
            if it.next().is_some() {
                return Err(bad());
            }
            out.push(if cmd == "menu" { Cmd::Menu(x, y, b, keys) } else { Cmd::Office(x, y, b, keys) });
            continue;
        }
        if cmd == "latch" {
            // A test line: the button start flags 0x284F1A / 0x284F1C (bit 0 left, bit 1 right) as a box that a key
            // ended under a held button leaves them; it acts in order with the office / A.I.M. polls.
            let v = arg.parse::<u8>().map_err(|_| bad())?;
            if v > 3 || it.next().is_some() {
                return Err(bad());
            }
            out.push(Cmd::Office(LATCH_POLL, v as i32, 0, vec![]));
            continue;
        }
        if cmd == "aimdebt" {
            let p = arg.parse::<usize>().map_err(|_| bad())?;
            let n = it.next().ok_or_else(bad)?.parse::<u16>().map_err(|_| bad())?;
            out.push(Cmd::AimDebt(p, n));
            continue;
        }
        if cmd == "inv" {
            let num = |v: Option<&str>| -> Result<i32, String> { v.ok_or_else(bad)?.parse().map_err(|_| bad()) };
            let (x, y, b) = (num(Some(arg))?, num(it.next())?, num(it.next())?);
            let keys = match it.next() {
                None => vec![],
                Some("Esc") => vec![0x1B],
                Some(k) if k.len() == 1 => vec![k.as_bytes()[0].to_ascii_uppercase()],
                Some(_) => return Err(bad()),
            };
            if it.next().is_some() {
                return Err(bad());
            }
            // inv lines with no screen open are ignored.
            if let Some(Cmd::Inventory(polls)) = out.last_mut() {
                polls.push(InvPoll { x, y, buttons: b as u8, flags: 0, keys });
            }
            continue;
        }
        if cmd == "opt" {
            if arg == "esc" {
                if it.next().is_some() {
                    return Err(bad());
                }
                out.push(Cmd::Opt(0, 0, false, true));
                continue;
            }
            let x = arg.parse().map_err(|_| bad())?;
            let y = it.next().ok_or_else(bad)?.parse().map_err(|_| bad())?;
            let left = match (it.next(), it.next()) {
                (None, None) => false,
                (Some("L"), None) => true,
                _ => return Err(bad()),
            };
            out.push(Cmd::Opt(x, y, left, false));
            continue;
        }
        // `drag T N|E|S|W`: a left press on T dragged toward that sector edge, then released (cursor.md "Drag to an
        // edge"); writes `drag D result R` and, when the squad leaves, `trav D sector S tile T` as `trav`.
        if cmd == "drag" {
            let tile = arg.parse().map_err(|_| bad())?;
            let dir = match it.next() {
                Some("N") => 10,
                Some("E") => 11,
                Some("S") => 12,
                Some("W") => 13,
                _ => return Err(bad()),
            };
            out.push(Cmd::Do(Input::Drag { tile, dir, shift: false, alt: false }));
            continue;
        }
        // `rturn T` / `rlook T`: the right button held on T with a left press / a drag down by 11 pixels
        // (cursor.md "Clicks", right step 3), then released.
        if cmd == "rturn" || cmd == "rlook" {
            let tile = arg.parse().map_err(|_| bad())?;
            let (left, y) = if cmd == "rturn" { (true, 100) } else { (false, 111) };
            out.push(Cmd::Do(Input::RHold { tile, shift: false, alt: false, y: 100 }));
            out.push(Cmd::Do(Input::Poll { left, right: true, y }));
            out.push(Cmd::Do(Input::Poll { left: false, right: false, y }));
            continue;
        }
        if cmd == "dragto" {
            if arg == "up" {
                out.push(Cmd::DragUp);
            } else if arg == "right" {
                out.push(Cmd::DragRight);
            } else {
                let mut n = || it.next().and_then(|v| v.parse().ok()).ok_or_else(bad);
                let (dx, dy) = (n()?, n()?);
                out.push(Cmd::DragTo(arg.parse().map_err(|_| bad())?, dx, dy));
            }
            continue;
        }
        // `raise up|down|ok|no ...`: the presses of the next op 297 salary box.
        if cmd == "raise" {
            let mut v = Vec::new();
            for a in std::iter::once(arg).chain(it.by_ref()) {
                v.push(match a {
                    "no" => 0,
                    "ok" => 1,
                    "down" => 2,
                    "up" => 3,
                    _ => return Err(bad()),
                });
            }
            out.push(Cmd::Raise(v));
            continue;
        }
        // `switch K I.. [open]`: the switch view of slot K (panel.md), presses on switches I (0..2).
        if cmd == "switch" {
            let k = arg.parse().map_err(|_| bad())?;
            let (mut is, mut open) = (Vec::new(), false);
            for t in it.by_ref() {
                match t {
                    "open" => open = true,
                    _ => is.push(t.parse::<usize>().ok().filter(|&i| i < 3).ok_or_else(bad)?),
                }
            }
            out.push(Cmd::Switch(k, is, open));
            continue;
        }
        // `strip X L|R|C`: a left / right / Ctrl-left press on the bottom strip at x X (panel.md).
        if cmd == "strip" {
            let x = arg.parse().map_err(|_| bad())?;
            let (right, ctrl) = match (it.next(), it.next()) {
                (Some("L"), None) => (false, false),
                (Some("R"), None) => (true, false),
                (Some("C"), None) => (false, true),
                _ => return Err(bad()),
            };
            out.push(Cmd::Do(Input::Strip { x, right, ctrl }));
            continue;
        }
        // `pointer X Y` / `pointer -1` (panel.md "Mouse-edge scroll").
        if cmd == "pointer" {
            let x: i32 = arg.parse().map_err(|_| bad())?;
            let y = if x < 0 { 0 } else { it.next().and_then(|v| v.parse().ok()).ok_or_else(bad)? };
            if it.next().is_some() {
                return Err(bad());
            }
            out.push(Cmd::Pointer(x, y));
            continue;
        }
        if it.next().is_some() {
            return Err(bad());
        }
        out.push(match cmd {
            "eod" => Cmd::Eod(arg.parse().map_err(|_| bad())?),
            "sunrise" => Cmd::Sunrise(arg.parse().map_err(|_| bad())?),
            "hover" => Cmd::Hover(arg.parse().map_err(|_| bad())?),
            "keyflags" => Cmd::KeyFlags(arg.parse().map_err(|_| bad())?),
            "view" => Cmd::View(arg.parse().map_err(|_| bad())?),
            "corpse" => match arg {
                "Y" | "y" => Cmd::Corpse(true),
                "N" | "n" => Cmd::Corpse(false),
                _ => return Err(bad()),
            },
            "disarm" => match arg {
                "Y" | "y" => Cmd::Disarm(true),
                "N" | "n" => Cmd::Disarm(false),
                _ => return Err(bad()),
            },
            "quick" => match arg {
                "Y" | "y" => Cmd::Quick(true),
                "N" | "n" => Cmd::Quick(false),
                _ => return Err(bad()),
            },
            "abort" => match arg {
                "0" => Cmd::Abort(false),
                "1" => Cmd::Abort(true),
                _ => return Err(bad()),
            },
            "trav" => Cmd::Trav(match arg {
                "N" | "n" => 10,
                "E" | "e" => 11,
                "S" | "s" => 12,
                "W" | "w" => 13,
                _ => return Err(bad()),
            }),
            "wait" => Cmd::Wait(arg.parse().map_err(|_| bad())?),
            "boxwait" => Cmd::BoxWait(arg.parse().map_err(|_| bad())?),
            "statkey" => Cmd::StatKey(arg.parse().map_err(|_| bad())?),
            "click" => Cmd::Do(Input::Click { tile: arg.parse().map_err(|_| bad())?, shift: false, alt: false }),
            // cursor.md "Clicks": a left click with Alt held.
            "altclick" => Cmd::Do(Input::Click { tile: arg.parse().map_err(|_| bad())?, shift: false, alt: true }),
            // cursor.md "Debug keys (level 4)": a left click with Shift held.
            "shiftclick" => Cmd::Do(Input::Click { tile: arg.parse().map_err(|_| bad())?, shift: true, alt: false }),
            // A left click on the face of slot K (0..7): SlotMouse ("cont" or choose the merc).
            // The mouse over the face of slot K (0..7; any other number: off the faces).
            "face" => Cmd::Do(Input::Face(arg.parse().map_err(|_| bad())?)),
            "portrait" => Cmd::Do(Input::Portrait(arg.parse().ok().filter(|&k: &u8| k < 8).ok_or_else(bad)?, false)),
            "rclick" => Cmd::Do(Input::RClick { tile: arg.parse().map_err(|_| bad())?, shift: false, alt: false }),
            "key" => match arg {
                "D" | "d" => Cmd::Do(Input::EndTurn),
                "I" | "i" => Cmd::Inventory(Vec::new()),
                "C" | "c" => Cmd::Do(Input::Compress),
                "F9" => Cmd::Do(Input::SeesLine),
                "Escape" => Cmd::Do(Input::Escape),
                "PgUp" => Cmd::Do(Input::View(1)),
                "PgDn" => Cmd::Do(Input::View(2)),
                "Insert" => Cmd::Do(Input::View(3)),
                "Tab" => Cmd::Do(Input::View(4)),
                "P" | "p" => Cmd::Do(Input::Pause),
                "Z" | "z" => Cmd::Do(Input::Popup),
                "L" | "l" => Cmd::Do(Input::SayAgain),
                "Space" => Cmd::Do(Input::NextMerc),
                "Enter" => Cmd::Do(Input::NextEnemy),
                "X" | "x" => Cmd::Do(Input::Swap),
                "V" | "v" => Cmd::Do(Input::Version),
                "Up" => Cmd::Do(Input::Arrow(0x48)),
                "Left" => Cmd::Do(Input::Arrow(0x4B)),
                "Right" => Cmd::Do(Input::Arrow(0x4D)),
                "Down" => Cmd::Do(Input::Arrow(0x50)),
                "+" | "-" | "." | "," => Cmd::Do(Input::Volume(arg.as_bytes()[0])),
                "Ctrl+F" => Cmd::Do(Input::ForceTurn),
                k if crate::debugkeys::parse(k).is_some() => Cmd::Do(Input::Debug(crate::debugkeys::parse(k).unwrap())),
                k if k.len() == 1 && ("1"..="8").contains(&k) => Cmd::Do(Input::Merc(k.as_bytes()[0] - b'1', false)),
                // Alt + 1..8: SelectMerc with force = 1 (selects a merc off the view at once).
                k if k.len() == 5 && k.starts_with("Alt+") && ("1"..="8").contains(&&k[4..]) => {
                    Cmd::Do(Input::Merc(k.as_bytes()[4] - b'1', true))
                }
                _ => return Err(bad()),
            },
            _ => return Err(bad()),
        });
    }
    Ok(out)
}

/// Runs a play script, one tick per pass. After the sector was left only the day lines run (section 9; the
/// others are ignored until the sector is entered again). `inv` runs the inventory screen of a `key I` line with
/// its polls. The day lines' stdout lines go to `play.day.lines`; a failed sector entry ends the script (Err).
pub fn run_script(w: &mut World, play: &mut Play, cmds: &[Cmd], inv: &mut dyn FnMut(&mut World, &mut Play, &[InvPoll]))
                  -> Result<(), String> {
    run_script_part(w, play, cmds, inv).map(|_| ())
}

/// As `run_script`, but it stops after the line whose office ended with Leave (world_load.md section 12) and
/// returns the number of lines it ran (all of them when it did not stop). `menu` lines do nothing here.
pub fn run_script_part(w: &mut World, play: &mut Play, cmds: &[Cmd], inv: &mut dyn FnMut(&mut World, &mut Play, &[InvPoll]))
                       -> Result<usize, String> {
    for (i, c) in cmds.iter().enumerate() {
        if play.day.to_menu {
            return Ok(i);
        }
        // A pick-up opened the inventory screen (op 344): the next `key I` line with its `inv` lines is that screen;
        // any other line closes it at once (no polls).
        if play.host.pick_inv.is_some() && !matches!(c, Cmd::Inventory(_)) {
            inv(w, play, &[]);
        }
        play.leave_late(w);
        if let Cmd::Menu(..) = c {
            continue;
        }
        // SPEC GAP: `clock` runs before and after the sector was left (it only prints).
        if *c == Cmd::Clock {
            let k = &w.clock;
            play.day.lines.push(format!("clock {} hours {} minutes {}", clock_text(w), k.hours, k.minutes));
            continue;
        }
        if *c == Cmd::Music {
            play.day.lines.push(crate::music::with(|m| m.line()));
            continue;
        }
        if *c == Cmd::Sound {
            let l = play.host.sound.line();
            play.day.lines.push(l);
            continue;
        }
        if *c == Cmd::Practice {
            for k in 0..8 {
                let row = &w.stat_progress[0x12 * k..0x12 * k + 0x12];
                let v: Vec<String> = row.chunks(2).map(|b| i16::from_le_bytes([b[0], b[1]]).to_string()).collect();
                play.day.lines.push(format!("practice {} {}", k, v.join(" ")));
            }
            continue;
        }
        if let Cmd::Pay(x, y, b, a, ms) = *c {
            play.host.pay_polls.push_back((x, y, b, a, ms));
            continue;
        }
        // The answers to op 345 disarm_ask and op 292 corpse_ask, before and after the sector was left.
        if let Cmd::Disarm(y) = *c {
            play.host.disarm_answers.push_back(y);
            continue;
        }
        if let Cmd::Corpse(y) = *c {
            play.host.corpse_answers.push_back(y);
            continue;
        }
        if let Cmd::Raise(v) = c {
            play.host.raise_presses.push_back(v.clone());
            continue;
        }
        if let Cmd::Office(x, y, b, keys) = c {
            play.host.office_polls.push_back((*x, *y, *b, keys.clone()));
            continue;
        }
        if let Cmd::AimDebt(p, n) = *c {
            play.host.aim_debt = Some((p, n));
            continue;
        }
        // A written seed also counts behind the sector end: the day-end screens draw before the first clock step.
        if let Cmd::Seed(n) = c {
            play.host.rng.seed = *n;
            if play.host.rng_log {
                eprintln!("seedset {:08x}", *n);
            }
            continue;
        }
        let day_line = matches!(c, Cmd::Day | Cmd::Map(..) | Cmd::Amap(..) | Cmd::Start(..));
        // SPEC GAP: a day line while the sector was not left is ignored, and so are `map` / `start` before `day`.
        // After a game end (world_load.md section 9 step 0) no line does anything.
        if play.day.game_over.is_some() {
            continue;
        }
        if play.left != day_line || (matches!(c, Cmd::Map(..) | Cmd::Amap(..) | Cmd::Start(..)) && !play.day.map_open) {
            continue;
        }
        // `dragto up`: the release's input runs as a `Do` line.
        let up;
        let c = if let Cmd::DragUp = c {
            let (x, y) = play.drag.map_or((160, 100), |d| d.at);
            up = play.drag_step(w, x, y, 0).map(Cmd::Do);
            match &up {
                Some(c) => c,
                None => continue,
            }
        } else {
            c
        };
        // The popups of key S go away with the next line.
        play.stat_shown = None;
        match c {
            Cmd::Day => {
                let (m, d) = play.day_steps(w);
                play.script_guide(crate::ctlpanel::sub_byte(w.update.opt_step_text));
                if play.day.to_menu {
                    // Section 12: the office ended with Leave; no sunrise, no map.
                    play.day.lines.extend(play.host.office_lines.iter().cloned());
                    continue;
                }
                if let Some(r) = play.day.verdict {
                    let word = play.day.rating.map(|r| crate::dayloop::rating_word(r)).unwrap_or("-");
                    play.day.lines.push(format!("verdict {} rating {}", r, word));
                }
                if let Some(r) = play.day.game_over {
                    play.day.lines.push(format!("game over {}", r));
                    let (iso, exe) = play.files.clone().ok_or("game end: no CD image / JA.EXE")?;
                    play.game_end(w, &iso, &exe)?;
                    continue;
                }
                // SPEC GAP: after an abort the morning does not run; its result shows as `-`.
                play.day.lines.push(format!("day morning {} day {}", m.map(|m| m.to_string()).unwrap_or("-".into()), d));
                for p in play.host.gone.clone() {
                    play.day.lines.push(format!("gone {}", p));
                }
                for &n in &play.host.pay_msgs {
                    play.day.lines.push(format!("pay message {}", n));
                }
                for &(k, f, a) in &play.host.pay_asks {
                    play.day.lines.push(format!("pay ask {} {} {}", k, f, a as u8));
                }
                for &(k, x, p) in &play.host.pay_said {
                    play.day.lines.push(format!("pay balloon {} {} {}", k, x, p));
                }
                for s in &play.host.merc_said {
                    play.day.lines.push(format!("merc say {} {} {} {} {} {}", s.k, s.x0, s.p, s.m, s.q, s.first as u8));
                }
                if play.host.pay_view.end {
                    let v = &play.host.pay_view;
                    play.day.lines.push(format!("pay end frames {:?}", v.frames));
                }
                play.day.lines.extend(play.host.office_lines.iter().cloned());
            }
            Cmd::Map(sec, mode, b) => {
                let n = play.day_map_click(w, *sec, *mode, *b);
                play.day.lines.push(format!("map {} msg {}", sec, n));
            }
            Cmd::Amap(-2, v, _) => crate::invextra::set_latch(*v as u8),
            Cmd::Amap(x, y, b) => {
                let line = play.amap_line(w, *x, *y, *b)?;
                play.day.lines.push(line);
            }
            Cmd::Gmap(x, y, b) => {
                play.gmap_line(w, *x, *y, *b)?;
            }
            Cmd::Start(a, b) => {
                play.amap = None;
                let a = *a as u32;
                let (m, r) = play.day_map_start(w, *b, Box::new(move |_| a));
                if let Some(v) = play.host.day_ask_args {
                    play.day.lines.push(format!("ask kind {} wait {} go {} number {}", v[0], v[1], v[2], v[3]));
                }
                play.day.lines.push(format!("start {} {}", m, r.map(|r| r.to_string()).unwrap_or("-".into())));
                if r == Some(1) {
                    let (iso, exe) = play.files.clone().ok_or("start: no CD image / JA.EXE for the sector entry")?;
                    // StrategicScreen 0x43E9E: the window also shows the 24 step fade and stops the music here.
                    crate::music::with(|m| m.set_volume(0, 1200));
                    play.day_enter(w, &iso, &exe)?;
                }
            }
            Cmd::BoxWait(ms) => {
                play.locked_ticks(w, false, (*ms / 10) as u64, true);
                play.day.lines.push(format!("boxwait {}", ms));
            }
            Cmd::Switch(k, is, open) => {
                if !crate::screen::switch_can_open(w, *k) {
                    play.day.lines.push(format!("switch {} refused", k));
                    continue;
                }
                // panel.md "The switch view": SlotMouse 0x11714 adds 1 to the view byte, then SetSlotView 0x332E4
                // keeps 0 or 1 (a 2 is the close, 0).
                let vb = *k as usize * REC + 0x17F;
                let _ = crate::slotmouse::view_step(&mut w.soldiers[vb - 0x17F..vb - 0x17F + REC]);
                for &i in is {
                    play.switch_press(w, *k, i);
                    play.switch_release(w, *k, i);
                }
                if !*open {
                    let _ = crate::slotmouse::view_step(&mut w.soldiers[vb - 0x17F..vb - 0x17F + REC]);
                }
                let r = &w.soldiers[*k as usize * REC..];
                play.day.lines.push(format!("switch {} flags {} {} {} reserved {} view {}", k, r[0x16A], r[0x16B], r[0x16C],
                                            r[0xEF], r[0x17F]));
            }
            Cmd::StatKey(ms) => {
                let ks = crate::screen::key_s(&w.soldiers, &mut play.cursor.over_map);
                play.day.lines.push(format!("stat popups {:?}", ks));
                play.stat_shown = Some(ks);
                play.locked_ticks(w, true, (*ms / 10) as u64, false);
            }
            Cmd::Wait(n) => {
                for _ in 0..*n {
                    play.pass(w, &[], 1);
                    play.slide_ticks(w, &crate::focus::take_slides());
                    play.script_box(crate::ctlpanel::sub_byte(w.update.opt_step_text));
                }
            }
            Cmd::Do(i) => {
                play.pass(w, &[*i], 1);
                play.slide_ticks(w, &crate::focus::take_slides());
                play.script_box(crate::ctlpanel::sub_byte(w.update.opt_step_text));
                // cursor.md "Tile popups": `popup item T pic P count C`, `popup sign T sector N` or `popup none T`;
                // the popup passes no game time.
                if let Some((t, _)) = play.popup.take() {
                    let p = popup_of(w, t);
                    play.day.lines.push(match p {
                        Some(crate::tilepopup::Popup::Item { pic, count, .. }) => format!("popup item {} pic {} count {}", t, pic, count.unwrap_or(0)),
                        Some(crate::tilepopup::Popup::Sign { sector, .. }) => format!("popup sign {} sector {}", t, sector),
                        None => format!("popup none {}", t),
                    });
                    play.last_popup = p;
                }
                if let (Input::Drag { dir, .. }, Some(out)) = (*i, play.traversed.take()) {
                    play.traverse_move(w, out)?;
                    play.traverse_theme(w);
                    let k = (w.selected as usize).min(7);
                    let t = i16::from_le_bytes([w.soldiers[k * 512 + 0x114], w.soldiers[k * 512 + 0x115]]);
                    play.day.lines.push(format!("trav {} sector {} tile {}", ["N", "E", "S", "W"][(dir - 10) as usize], w.cur_sector, t));
                }
                // `key K` with a debug key writes `debug K level L msg M|M` (cursor.md "Debug keys (level 4)").
                if let Input::Debug(code) = *i {
                    play.day.lines.push(format!("debug {} level {} msg {}", code, crate::debugkeys::level(),
                                                std::mem::take(&mut play.debug_msgs).join("|")));
                }
                // panel.md "Say it again (key L)": `again K cat C row R quote Q text T` or `again none K`.
                if std::mem::take(&mut play.again) {
                    let q = &w.last_quote;
                    let (row, quote) = (i16::from_le_bytes([q[2], q[3]]), i16::from_le_bytes([q[4], q[5]]));
                    play.day.lines.push(if q[0] <= 0x42 && (q[1] as usize) < crate::sound::CATS.len() {
                        format!("again {} cat {} row {} quote {}", q[0], q[1], row, quote)
                    } else {
                        format!("again none {}", q[0])
                    });
                    play.last_again = true;
                }
                // The debug pages (Ctrl+P, Shift+left click): `pages S p1 V,V,..  p2 .. p4 ..` (the 17 row values
                // of each page); a script waits for no key between the pages.
                if let Some(k) = play.host.debug_pages.take() {
                    let mut t = format!("pages {}", k);
                    for (n, p) in crate::debugpage::pages_of(w, k).iter().enumerate() {
                        t += &format!(" p{} {}", n + 1, p.rows.iter().map(|r| r.1.to_string()).collect::<Vec<_>>().join(","));
                    }
                    play.day.lines.push(t);
                    play.last_pages = Some(k);
                }
                // `strip X B` writes `strip X B hand T box T cursor T drop F` (types of entry 0, the box's entry,
                // the cursor item; F = cursor_drop) for the selected merc.
                if let Input::Strip { x, right, ctrl } = *i {
                    let k = (w.selected as usize).min(47) * 512;
                    let e = [0usize, 7, 8, 9, 10, 11, 1][(0..7).rev().find(|&b| x >= [0, 49, 94, 139, 184, 229, 272][b] - 1).unwrap_or(0)];
                    let ty = |e: usize| w.soldiers[k + 0x0C * e];
                    let b = if right { "R" } else if ctrl { "C" } else { "L" };
                    play.day.lines.push(format!("strip {} {} hand {} box {} cursor {} drop {}", x, b, ty(0), ty(e), w.held[0], w.inv.cursor_drop));
                }
            }
            &Cmd::Opt(x, y, left, esc) => {
                // panel.md "The Control Panel": the line `opt X Y hover H switch K value V close R` (K = -1: no
                // switch changed; R = -2: still open; `opt closed` when the panel cannot open).
                if play.ctl.is_none() {
                    if play.left || w.update.screen_drawn == 0 {
                        play.day.lines.push("opt closed".into());
                        continue;
                    }
                    // When it opens: key F10 puts the cursor item back first (the line `opt back`).
                    if Session::new(w, &mut play.host).cursor_back() {
                        play.day.lines.push("opt back".into());
                    }
                    let restore = play.day.save_dir.as_ref().is_some_and(|d| d.join("QUICK.SAV").exists());
                    let quick = w.base.get(21285).is_some_and(|&b| b != 0);
                    play.ctl = Some(crate::ctlpanel::Panel::open(w, quick, restore, w.sound_setup.sound_driver, w.sound_setup.music_driver));
                }
                let cp = play.ctl.as_mut().unwrap();
                let o = cp.pass(w, x, y, left, esc);
                for _ in 0..o.clicks {
                    play.host.sound.sfx(0, 0x2B11, 20, 1);
                }
                play.host.sound.fx_off = crate::ctlpanel::fx_off(w);
                let fx_off = play.host.sound.fx_off;
                crate::sound::screen(|s| s.fx_off = fx_off);
                if let Some(on) = o.music {
                    crate::music::with(|m| m.switch(on));
                }
                let (hover, k) = (cp.hover, o.toggled.map(|k| k as i32).unwrap_or(-1));
                play.ctl_log = Some(cp.log.clone());
                if o.hide_marks {
                    let n = show_paths_off(w, &mut play.host);
                    play.day.lines.push(format!("opt marks {} -> {}", n, w.update.marks_n));
                }
                let v = if k >= 0 { crate::ctlpanel::get(w, k as usize) as i32 } else { -1 };
                play.day.lines.push(format!("opt {} {} hover {} switch {} value {} close {}", x, y, hover, k, v, o.close.unwrap_or(-2)));
                if let Some(r) = o.close {
                    play.ctl = None;
                    // The panel was a Lock user (clock switch 0); script passes take no time.
                    play.locked_ticks(w, false, 0, false);
                    if let Some(save) = crate::ctlpanel::after_close(r) {
                        play.quick = Some(if save { crate::ctlpanel::QUICK_SAVE } else { crate::ctlpanel::QUICK_RESTORE });
                    }
                }
            }
            // `quick Y|N` answers the question box after Quick Save / Quick Restore: `quick save B` (B bytes written),
            // `quick restore day D sector S selected K theme T tileset N`, `quick no` (No) or `quick none` (no question open).
            &Cmd::Quick(yes) => {
                let line = match (play.quick.take(), yes) {
                    (None, _) => "quick none".to_string(),
                    (Some(_), false) => "quick no".to_string(),
                    (Some(r), true) if r == crate::ctlpanel::QUICK_SAVE => format!("quick save {}", play.quick_save(w)?),
                    (Some(_), true) => {
                        let t = play.quick_restore(w)?;
                        format!("quick restore day {} sector {} selected {} theme {} tileset {}", w.day, w.cur_sector, w.selected, t, play.day.tileset)
                    }
                };
                play.day.lines.push(line);
            }
            Cmd::Abort(r) => {
                // `events B -> A`: the day replay's event count before and after (strategic.md "The day replay").
                let before = play.host.replay_events.len();
                play.pass(w, &[Input::Abort { restart: *r }], 1);
                if let Some(restart) = play.aborted.take() {
                    play.abort_day(w, restart)?;
                }
                play.day.lines.push(format!("abort {} day {} sector {} events {} -> {}", *r as u8, w.day, w.cur_sector,
                    before, play.host.replay_events.len()));
            }
            Cmd::Trav(d) => {
                let name = ["N", "E", "S", "W"][(*d - 10) as usize];
                play.host.status = None;
                play.pass(w, &[Input::Traverse(*d)], 1);
                if let Some(out) = play.traversed.take() {
                    play.traverse_move(w, out)?;
                    play.traverse_theme(w);
                    let k = (w.selected as usize).min(7);
                    let t = i16::from_le_bytes([w.soldiers[k * 512 + 0x114], w.soldiers[k * 512 + 0x115]]);
                    play.day.lines.push(format!("trav {} sector {} tile {}", name, w.cur_sector, t));
                } else {
                    play.day.lines.push(format!("trav {} refused {}", name, play.host.status.clone().unwrap_or_default()));
                }
            }
            Cmd::Clock | Cmd::Practice | Cmd::Sound | Cmd::Music | Cmd::Pay(..) | Cmd::Office(..) | Cmd::AimDebt(..) | Cmd::Menu(..) => {}
            Cmd::Blast(t, k) => Session::new(w, &mut play.host).explode(0, *t, *k, 1),
            Cmd::Mine(t, c) => Session::new(w, &mut play.host).put_mine(*t, *c),
            Cmd::Gas(t, n, r) => {
                for (o, v) in [(0, *t), (2, *n), (4, *r)] {
                    w.gas_clouds[o..o + 2].copy_from_slice(&(v as i16).to_le_bytes());
                }
            }
            Cmd::Hp(k, v) => w.soldiers[*k as usize * 512 + 0x183] = *v as u8,
            Cmd::Stop(k) => {
                let b = *k as usize * 512;
                let so = &mut w.soldiers;
                so[b + 0x12A..b + 0x12C].copy_from_slice(&0xC41u16.to_le_bytes());
                so[b + 0x1F6] = 0;
                so.copy_within(b + 0x114..b + 0x116, b + 0x124);
                if so[b + 0x1A6] != 0 {
                    so[b + 0x95] = so[b + 0x1A6];
                    so.copy_within(b + 0x134..b + 0x136, b + 0x96);
                }
            }
            Cmd::Hit(k, h, d) => {
                Session::new(w, &mut play.host).hit_location(*k, *h, *d);
            }
            Cmd::Tile(k, t) => {
                let o = *k as usize * 512 + 0x114;
                let old = u16::from_le_bytes([w.soldiers[o], w.soldiers[o + 1]]) as usize;
                if old < 3136 {
                    w.tiles[old * 8 + 5] = (w.tiles[old * 8 + 5] & 0x80) | 0x40;
                }
                w.soldiers[o..o + 2].copy_from_slice(&(*t as u16).to_le_bytes());
                // TileToX / TileToY: the sight line starts and ends at these places.
                let (x, y) = ((*t % 56 * 16 + 5) as u16, (*t / 56 * 14 + 5) as u16);
                for (f, v) in [(0x116, x), (0x118, y), (0x126, x), (0x128, y)] {
                    let p = *k as usize * 512 + f;
                    w.soldiers[p..p + 2].copy_from_slice(&v.to_le_bytes());
                }
                w.tiles[*t as usize * 8 + 5] = (w.tiles[*t as usize * 8 + 5] & 0x80) | *k as u8;
            }
            Cmd::Seed(_) => {}
            Cmd::ClockSeed(n) => play.host.clock_seed = Some((*n, unsafe { ja1_clock_steps() })),
            Cmd::SetupSeed(n) => play.host.setup_seed = Some(*n),
            Cmd::FrameFree => play.host.frame_free = Some(1),
            Cmd::CompressDay => {
                // Test only (validate/replay_week.py): key C again each time an event ends the compressed time, until
                // the squad leaves the sector. The other side is press_until_stopped in the original.
                for _ in 0..20000 {
                    if play.left {
                        break;
                    }
                    let on = Session::new(w, &mut play.host).compressing();
                    let keys = [Input::Compress];
                    play.pass(w, if on { &[] } else { &keys }, 1);
                    play.slide_ticks(w, &crate::focus::take_slides());
                    play.script_box(crate::ctlpanel::sub_byte(w.update.opt_step_text));
                }
            }
            Cmd::Item(t, ty, c, b6) => Session::new(w, &mut play.host).put_ground(*t, *ty, *c, *b6),
            Cmd::Disarm(_) | Cmd::Corpse(_) | Cmd::Raise(_) => {}
            Cmd::Hover(t) => play.set_mouse(if *t < 0 { None } else { Some(*t) }),
            Cmd::KeyFlags(f) => play.key_flags = *f,
            Cmd::Pointer(x, y) => {
                play.mouse_out = *x < 0;
                if *x >= 0 {
                    play.mouse_px = (*x, *y);
                }
            }
            Cmd::DragTo(t, dx, dy) => {
                if play.drag.is_none_or(|d| d.loop_.tile != *t) {
                    // As the window: only a press that reaches the loop starts it; another one acts at the press.
                    if !play.press_walks(w, *t, false, false) {
                        play.day.lines.push(format!("dragto press {}", t));
                        play.pass(w, &[Input::Click { tile: *t, shift: false, alt: false }], 1);
                        continue;
                    }
                    let (value, ct) = (play.cursor.value, play.cursor.tile);
                    play.drag_press(*t, (160, 100), false, false, value, ct);
                }
                // The original reads the buttons as levels: the script's left stays down.
                let _ = play.drag_step(w, 160 + dx, 100 + dy, 1);
                let dir = play.drag.map_or(0, |d| if d.loop_.active { d.loop_.value as i32 } else { 0 });
                play.day.lines.push(format!("dragto dir {} cursor {}", dir, play.drag_value.unwrap_or(0)));
            }
            Cmd::DragUp => {}
            Cmd::DragRight => {
                if let Some((x, y)) = play.drag.map(|d| d.at) {
                    let _ = play.drag_step(w, x, y, 3);
                    let _ = play.drag_step(w, x, y, 1);
                }
            }
            Cmd::View(t) => {
                // As the window's arrows: the view's top left tile, kept on the map (14 x 12 tiles).
                w.view_x = (t.rem_euclid(56)).min(56 - 14) * 16;
                w.view_y = (t.div_euclid(56)).clamp(0, 56 - 12) * 14;
            }
            Cmd::Eod(n) => {
                if let Some(e) = play.host.eod.as_mut() {
                    e.poll_at = Some(*n);
                }
            }
            Cmd::Sunrise(n) => {
                if let Some(e) = play.host.sunrise.as_mut() {
                    e.poll_at = Some(*n);
                }
            }
            // While time compression is on, key I only turns it off (turns.md "Time compression").
            Cmd::Inventory(_) if w.update.time_compress != 0 && play.host.pick_inv.is_none() => {
                play.pass(w, &[Input::Other], 1)
            }
            Cmd::Inventory(polls) => {
                inv(w, play, polls);
                play.pass(w, &[], 1);
            }
        }
    }
    // A pick-up in the last line: its screen closes at once, and a leave that waited for it runs.
    if play.leave_held {
        if play.host.pick_inv.is_some() {
            inv(w, play, &[]);
        }
        play.leave_late(w);
    }
    Ok(cmds.len())
}

impl Play {
    /// A play script closes each message box at once (no game time passes while a box shows): the line
    /// `msgbox TEXT`, and the text is kept for JA1_MSG_PNG. Before it, the `sighting` lines.
    fn script_guide(&mut self, sub: bool) {
        // panel.md "The guide's and Brenda's balloons": Subtitles off = `balloon guide X Y` / `balloon brenda X Y`
        // (the face point); Subtitles on = `balloon guide box Q` / `balloon brenda box Q` per part (the text-only
        // merc-face box); the last one is kept for JA1_BALLOON_PNG (drawn over the island map).
        for (op, a) in std::mem::take(&mut self.host.guide_said) {
            if sub {
                for q in crate::sound::npc_parts(op, &a).map(|p| p.2).unwrap_or_default() {
                    self.day.lines.push(format!("balloon {} box {}", crate::balloon::npc_name(op), q));
                }
            } else {
                let (x, y) = crate::balloon::SANTINO;
                self.day.lines.push(format!("balloon {} {} {}", crate::balloon::npc_name(op), x, y));
            }
            self.last_guide = Some((op, a, sub));
            (self.last_balloon, self.last_santino) = (None, false);
        }
    }

    fn script_box(&mut self, sub: bool) {
        for k in std::mem::take(&mut self.host.wipes) {
            self.day.lines.push(format!("wipe {}", if k == 0 { "close" } else { "open" }));
        }
        // combat.md "Doors": `door film MEMBER X Y FRAMES`; the last one is kept for JA1_DOOR_PNG.
        for f in std::mem::take(&mut self.host.door_films) {
            let n = crate::flc::Flc::new(&f.3).map(|c| c.frames).unwrap_or(0);
            self.day.lines.push(format!("door film {} {} {} {}", f.0, f.1, f.2, n));
            self.last_door_film = Some(f);
        }
        // LoadPopups? 0x288DC (op 71): `anim popup SLOT FLAG FRAMES` (flag 1 = drown, 0 = injection); the last one
        // is kept for JA1_ANIMPOP_PNG.
        for (slot, flag, selected) in std::mem::take(&mut self.host.popups) {
            let n = match &self.files {
                Some((_, exe)) => crate::popup::frames(exe, slot, selected, flag).map(|f| f.len()).unwrap_or(0),
                None => 0,
            };
            self.day.lines.push(format!("anim popup {} {} {}", slot, flag, n));
            self.last_anim_popup = Some((slot, flag, selected));
        }
        // soldier_update.md "Merc sighting": one line `sighting SLOT HOW FOCUS TEXT` per reaction.
        for r in std::mem::take(&mut self.host.reactions) {
            if r.nat {
                self.day.lines.push(format!("sector line {} {} {}", r.slot, r.how, r.focus.unwrap_or(-1)));
            } else {
                self.day.lines.push(format!("sighting {} {} {} {}", r.slot, r.how, r.focus.unwrap_or(-1), r.text));
            }
            // panel.md "Speech balloon": `balloon SLOT QUOTE TEXT` (the decoded quote); kept for JA1_BALLOON_PNG.
            if let Some(rec) = r.balloon {
                let t = match (&self.files, r.nat) {
                    (Some((iso, exe)), false) => crate::balloon::quote_text(iso, exe, &rec, r.how as i32),
                    (Some((iso, exe)), true) => crate::balloon::nat_text(iso, exe, r.how as i32),
                    (None, _) => Vec::new(),
                };
                // turns.md "Sector line": `balloon SLOT nat SECTOR TEXT`.
                let q = if r.nat { format!("nat {}", r.how) } else { r.how.to_string() };
                self.day.lines.push(format!("balloon {} {} {}", r.slot, q, String::from_utf8_lossy(&t)));
                self.last_balloon = Some((rec, r.how as i32, r.text.clone(), r.nat));
                self.last_santino = false;
                self.last_guide = None;
            }
        }
        for (cat, q) in std::mem::take(&mut self.host.npc_said) {
            let t = match (&self.files, cat) {
                (Some((iso, exe)), 9) => crate::balloon::shared_text(&crate::aim::edt(iso, exe, "SANQUOTE.EDT"), 200 * q as usize),
                _ => Vec::new(),
            };
            self.day.lines.push(format!("speech {} {} {}", cat, q, String::from_utf8_lossy(&t)));
            // panel.md "Santino's balloon" (Subtitles off): `balloon santino X Y` = the face point; JA1_BALLOON_PNG.
            if cat == 9 {
                let (x, y) = crate::balloon::SANTINO;
                self.day.lines.push(format!("balloon santino {} {}", x, y));
                self.last_santino = true;
                self.last_guide = None;
            }
        }
        self.script_guide(sub);
        // strategic.md "The battle show": a script starts the sounds at once and keeps the last box (JA1_BATTLE_PNG).
        // After the Leave path (the screen faded out) the show is dark: the line `battle dark`.
        // Fan fix F28: a fade held back for the show (fade_held) means the show was lit.
        let dark = self.left && self.host.fade_held.take().is_none();
        if dark && !self.host.battle_show.is_empty() {
            self.day.lines.push("battle dark".to_string());
        }
        for ev in std::mem::take(&mut self.host.battle_show) {
            match ev {
                BattleEv::Sound(n) => {
                    self.host.sound.battle(&n);
                }
                BattleEv::Box(s, e, g, t, k) => {
                    self.last_battle = Some((s, (e, g, t), k));
                    self.last_battle_dark = dark;
                }
                _ => {}
            }
        }
        // Loud deaths: one "is dead!" box each, in death order (MercDies? 0x3A4A4).
        for (_, text) in std::mem::take(&mut self.host.morph) {
            if let Some(t) = text {
                self.day.lines.push(format!("msgbox {}", t));
                self.last_box = Some(t);
            }
        }
        if let Some((t, _)) = self.host.msg_box.take() {
            self.day.lines.push(format!("msgbox {}", t));
            self.last_box = Some(t);
        }
    }
}

/// CursorModeSet? 0x1C438: the host op 185 click_panel (slot, mode) of the library call that just returned, if any.
/// The click handler applies it inside the call: the original's UseItemAt 0x1D9B8 reads the cursor mode right after
/// TacticalClick returns at 0x1ECFD.
pub fn take_click_panel_now(h: *const crate::host::Ja1Host) -> Option<(i32, i32)> {
    // SAFETY: h.ctx is a PlayHost while a library call of a Session runs; null in the unit tests.
    unsafe {
        if h.is_null() || (*h).ctx.is_null() {
            return None;
        }
        let host = &mut *((*h).ctx as *mut PlayHost);
        host.click_panel.take()
    }
}

/// cursor.md "Tile popups": the popup of tile `t` in the world's view (None when nothing shows).
pub fn popup_of(w: &World, t: i32) -> Option<crate::tilepopup::Popup> {
    let (_, _, left, top) = crate::viewmode::world_window(w);
    crate::tilepopup::which(&w.tiles, &w.ground_items, &w.item_types, t, (w.view_x, w.view_y, left, top, w.view_w, w.view_h), w.cur_sector as i32)
}

/// The script's stdout lines: `slot K tile T hp H ap A` per soldier with +0x182 != 0, `dayclock M day D` (day_minutes since
/// 7:00, the day), then `passes P left L`.
pub fn script_report(w: &World, play: &Play) -> String {
    use crate::soldier::REC;
    let mut s = String::new();
    for k in 0..48 {
        let r = &w.soldiers[k * REC..(k + 1) * REC];
        if r[0x182] == 0 {
            continue;
        }
        s += &format!("slot {} tile {} hp {} ap {}\n", k, soldier_tile(w, k), r[0x183] as i8, r[0x1A5] as i8);
    }
    for c in w.gas_clouds.chunks_exact(6) {
        let v = |o: usize| i16::from_le_bytes([c[o], c[o + 1]]);
        if v(0) as u16 != 0xC41 {
            s += &format!("cloud tile {} turns {} radius {}\n", v(0), v(2), v(4));
        }
    }
    if play.mouse.is_some() {
        let c = &play.cursor;
        let label = String::from_utf8_lossy(&c.label[..c.label.iter().position(|&b| b == 0).unwrap_or(16)]).into_owned();
        s += &format!("cursor tile {} value {} mode {} ap {} over {} label {:?}\n", c.tile, c.value, c.mode, c.ap, c.over_map, label);
        s += &format!("marks shown {} spotted {} view {} phase {}\n", w.update.shown_slot, w.update.spotted,
                      w.update.view_tile, c.mark_phase);
    }
    let wait = match &play.wait {
        None => "none",
        Some(crate::cursor::ClickWait::Aim { .. }) => "aim",
        Some(crate::cursor::ClickWait::Confirm { .. }) => "confirm",
        Some(crate::cursor::ClickWait::Walk { .. }) => "walk",
        Some(crate::cursor::ClickWait::Hold(_)) => "hold",
        Some(crate::cursor::ClickWait::Posture(_)) => "posture",
    };
    s += &format!("dayclock {} day {}\n", w.clock.minutes, w.day);
    s += &format!("wait {}\n", wait);
    s += &format!("passes {} left {}\n", play.passes, play.left as u8);
    s
}

/// s16 +0x114 of soldier `slot` (its tile).
pub fn soldier_tile(w: &World, slot: usize) -> i16 {
    let o = slot * crate::soldier::REC + 0x114;
    i16::from_le_bytes([w.soldiers[o], w.soldiers[o + 1]])
}

/// The game's 10 ms timer, `n` ticks: the library's Timer tick, the blink timer and the tile marker.
/// TacticalTimer? 0x5EF30..0x5EF6F: the small panel's countdown while no merc is lit (0x284F0E = 0x40).
pub fn panel_countdown(w: &mut World, lit: u8) {
    // 0x5EEA4: round_lock2 stops the whole callback.
    if w.update.round_lock2 == 0 && w.small_panel > 0 && lit == 0x40 {
        w.small_panel -= 10;
        if w.small_panel <= 0 {
            w.small_panel = 0;
            w.panel_ends = 1;
        }
    }
}

pub fn timer_ticks(s: &mut Session, pause: &mut crate::pause::Pause, cursor: &mut crate::cursor::CursorState,
               scroll: &mut crate::viewmode::Scroll, lines: &mut Vec<String>, n: u64) {
    timer_ticks_in(s, pause, cursor, scroll, lines, n, Some(true))
}

/// `timer_ticks`; `lib` = false leaves out the library's Timer tick (it ran inside the pass) but keeps the
/// front-end's animation timers.
#[allow(clippy::too_many_arguments)]
/// screen_drawn 0x92DCC while the inventory screen runs: 1 for a pick-up's screen under fan fix F66 (cave 0x2B6A1),
/// else as it was.
pub fn pick_screen_drawn(drawn: u8, from_pick: bool, fan66: bool) -> u8 {
    if from_pick && fan66 {
        1
    } else {
        drawn
    }
}

fn timer_ticks_in(s: &mut Session, pause: &mut crate::pause::Pause, cursor: &mut crate::cursor::CursorState,
               scroll: &mut crate::viewmode::Scroll, lines: &mut Vec<String>, n: u64, lib: Option<bool>) {
    // lib: Some(true) = the library's Timer tick (with the animation timers), Some(false) = the animation timers
    // only, None = neither (both ran inside the pass).
    for _ in 0..n {
        let (lock2_open, open) = s.round_gates();
        match lib {
            Some(true) => s.tick(),
            Some(false) => s.anims_tick(),
            None => {}
        }
        // TacticalTimer? 0x5EEA4: round_lock2 stops the whole callback, the pause blink timer 0x229D48 too.
        if lock2_open {
            pause.tick();
        }
        let had = cursor.marker_tile != 0xC41;
        crate::cursor::marker_tick(cursor, lock2_open, open);
        crate::cursor::clocks_tick(cursor, lock2_open);
        // TacticalTimer? 0x5EE98: the scroll's hover timer 0x229D28 and delay 0x229D44 go with the blink timer.
        if lock2_open {
            s.loss_tick();
            scroll.hover = scroll.hover.wrapping_sub(10);
            scroll.delay = scroll.delay.wrapping_sub(10);
        }
        if had && cursor.marker_tile == 0xC41 {
            lines.push("marker off".into());
        }
    }
}

/// Section 8 "Test command" after the entry: move order (SLOT, TILE, 0, 0, 0, 0, 0), then TICKS x (tick, frame).
/// Returns the move order's result.
pub fn walk(w: &mut World, host: &mut PlayHost, slot: i32, tile: i32, ticks: u32) -> i32 {
    let mut s = Session::new(w, host);
    let r = s.move_order(slot, tile, 0, 0, 0, 0, 0);
    for _ in 0..ticks {
        s.tick();
        s.frame();
    }
    r
}

#[cfg(test)]
mod tests {
    /// Ledger row c2aea50:ja1front:18098: Space is refused with an item on the cursor in any state, and in a fight
    /// while the screen is not drawn or the view is held.
    /// Ledger row c2aea50:ja1front:17912: the tile and effect animations of a tick run only with both round locks 0.
    #[test]
    fn the_animation_timers_need_both_locks_off() {
        assert!(super::timer_gates_open(0, 0));
        assert!(!super::timer_gates_open(1, 0));
        assert!(!super::timer_gates_open(0, 1));
    }

    #[test]
    fn space_needs_an_empty_cursor() {
        assert!(super::space_blocked(false, true, false, true));
        assert!(!super::space_blocked(false, true, false, false));
        assert!(super::space_blocked(true, false, false, false));
        assert!(super::space_blocked(true, true, true, false));
        assert!(!super::space_blocked(true, true, false, false));
    }

    /// Fan fix F66 (#70 row 90): a pick-up's inventory screen in the enemies' turn (screen_drawn 0) shows the
    /// pointer only with the fix; key I's screen and the screen of a pick-up in the player's turn stay as they were.
    #[test]
    fn fan66_pick_screen_shows_the_pointer() {
        use crate::invscreen::pointer_parts;
        assert_eq!(super::pick_screen_drawn(0, true, false), 0);
        assert_eq!(super::pick_screen_drawn(0, true, true), 1);
        assert_eq!(super::pick_screen_drawn(0, false, true), 0);
        assert_eq!(super::pick_screen_drawn(1, true, false), 1);
        // ShowMouse? does nothing with screen_drawn 0: no arrow, no held item
        assert_eq!(pointer_parts(false, false), (false, false));
        assert_eq!(pointer_parts(false, true), (false, false));
        assert_eq!(pointer_parts(true, false), (true, false));
        assert_eq!(pointer_parts(true, true), (false, true));
    }

    /// Fan fix F65 (#70 row 73): the inventory time runs with round_lock2 set (1.15m cave 0x42780), so the
    /// timer does nothing: no Lock blink, the blink timer stays, round_lock2 is back at 0 after. 1.13b: Lock?(0)
    /// sets the blink byte and the timer counts down from 800; after a box the round lock is free (open ticks).
    #[test]
    fn fan65_inventory_time_stops_the_timer() {
        let (exe, jag, save) = crate::world::tests::files(&[]);
        for (fan, open) in [(false, 0), (false, 10), (true, 0), (true, 10)] {
            let mut w = World::from_files(&exe, &jag, &save).unwrap();
            let mut p = Play::new(1);
            (w.update.round_lock, w.update.round_lock2) = (0, 0);
            (p.pause.blink, p.pause.blink_timer) = (false, 5);
            w.update.busy_timer = 1000;
            p.inv_ticks_with(&mut w, 30, open, fan);
            // the busy timer runs only with the round lock free: the open ticks of 1.13b
            assert_eq!(w.update.busy_timer, if fan { 1000 } else { 1000 - 10 * open as i32 });
            assert_eq!((w.update.round_lock, w.update.round_lock2), (0, 0));
            assert!(!p.pause.lock_owned);
            if fan {
                assert_eq!((p.pause.blink, p.pause.blink_timer), (false, 5));
            } else {
                // the box's Lock? sets the blink timer to 800 again
                assert_eq!((p.pause.blink, p.pause.blink_timer), (true, 800 - 10 * if open > 0 { open } else { 30 } as i32));
            }
        }
    }

    /// Ledger row c2aea50:ja1front:17378: Show Paths off hides the shown path marks also while a path preview
    /// is on (force 1; Hide path marks steps 1..3).
    #[test]
    fn show_paths_off_hides_the_preview_marks() {
        let (exe, jag, save) = crate::world::tests::files(&[]);
        let mut w = World::from_files(&exe, &jag, &save).unwrap();
        let mut p = Play::new(1);
        (w.update.marks_shown, w.update.marks_n, w.path_preview) = (1, 0, 1);
        super::show_paths_off(&mut w, &mut p.host);
        assert_eq!((w.update.marks_shown, w.path_preview), (0, 0));
    }

    /// Ledger rows 0b4d529:ja1front:18428/18430 (TacticalKeyHandler case 0x20, play.rs next_merc): Space gives the
    /// next merc outside a fight with no item on the cursor, and does nothing while an item is on the cursor.
    #[test]
    fn space_picks_the_next_merc_only_with_an_empty_cursor() {
        let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
        let (iso, exe, save) = (root.join("rom/JA1.gog"), root.join("build/game113b/JA.EXE"), root.join("build/DAY_day3.SAV"));
        if !iso.exists() || !exe.exists() || !save.exists() {
            return;
        }
        let e = crate::enter::enter(iso.to_str().unwrap(), exe.to_str().unwrap(), save.to_str().unwrap(), 1).unwrap();
        let base = e.world;
        // (selected, view_x, view_y, marks_shown) after Space; the view starts far from the mercs, marks shown
        let run = |combat: u8, drawn: u8, item: u8| {
            let mut w = base.clone();
            let mut p = Play::new(1);
            (w.combat, w.update.screen_drawn, w.update.view_hold, w.held[0]) = (combat, drawn, 0, item);
            (w.view_x, w.view_y, w.update.marks_shown) = (0, 0, 1);
            let before = (w.selected, w.view_x, w.view_y, w.update.marks_shown);
            Session::new(&mut w, &mut p.host).next_merc();
            (before, (w.selected, w.view_x, w.view_y, w.update.marks_shown))
        };
        let (before, after) = run(0, 1, 0);
        assert_ne!(before.0, after.0, "no item: Space picks another merc");
        let (before, after) = run(0, 0, 0);
        assert_ne!(before, after, "out of a fight Space runs (the marks go, the view goes to the merc)");
        let (before, after) = run(1, 0, 0);
        assert_eq!(before, after, "in a fight before the screen is drawn Space does nothing");
        let (before, after) = run(0, 1, 7);
        assert_eq!(before, after, "an item on the cursor blocks Space");
    }

    /// Ledger rows c2aea50:ja1front:18190/18193/5416/5447, 0b4d529:ja1front:18502/18504/18517/18518 (cursor.md
    /// "Debug keys (level 4)"; TacticalKeyHandler 0x13490 cases 0xF, 0x10, 0x14, 0x46): a player enters level 4 with
    /// Ctrl+B, Ctrl+A, Ctrl+L, Ctrl+D; then F, Ctrl+P (+ page 4 "# enemies"), Ctrl+O and Ctrl+T act as the original.
    #[test]
    fn the_debug_keys_after_the_four_level_keys() {
        use crate::debugkeys as d;
        let _g = d::TEST_LOCK.lock().unwrap_or_else(|e| e.into_inner());
        let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
        let (iso, exe, save) = (root.join("rom/JA1.gog"), root.join("build/game113b/JA.EXE"), root.join("build/DAY_day3.SAV"));
        if !iso.exists() || !exe.exists() || !save.exists() {
            return;
        }
        let e = crate::enter::enter(iso.to_str().unwrap(), exe.to_str().unwrap(), save.to_str().unwrap(), 1).unwrap();
        let base = e.world;
        // A free tile with an object byte.
        let t = (0..3136).find(|&t| base.tiles[t * 8 + 1] != 0 && base.tiles[t * 8 + 5] & 0x7F == 0x40).unwrap() as i32;
        let level4 = |s: &mut Session| {
            d::set_level(0);
            assert!(s.debug_key(0x47, Some(t)).is_empty(), "G does nothing at level 0");
            assert!(s.debug_key(d::CTRL_A, None).is_empty());
            assert_eq!(d::level(), 0, "Ctrl+A out of order: level 0");
            for (k, l) in [(d::CTRL_B, 1), (d::CTRL_A, 2), (d::CTRL_L, 3), (d::CTRL_D, 4)] {
                assert!(s.debug_key(k, None).is_empty(), "nothing shows when the level changes");
                assert_eq!(d::level(), l);
            }
        };
        // F: "struc is N", N = T's object byte; Ctrl+P: "CHARACTER SELECTED = n", then the pages of n.
        let mut w = base.clone();
        let cur = w.cur_sector as usize;
        (w.sector_state[cur * 9 + 1], w.sector_state[cur + 10]) = (3, 9);
        let mut p = Play::new(1);
        let sel = w.selected as usize;
        {
            let mut s = Session::new(&mut w, &mut p.host);
            level4(&mut s);
            assert_eq!(s.debug_key(0x46, Some(t)), vec![format!("struc is {}", base.tiles[t as usize * 8 + 1])]);
            assert_eq!(s.debug_key(d::CTRL_P, None), vec![format!("CHARACTER SELECTED = {sel}")]);
        }
        assert_eq!(p.host.debug_pages, Some(sel), "Ctrl+P opens the pages of the selected soldier");
        let pg = crate::debugpage::pages_of(&w, sel);
        assert_eq!(pg[3].rows[5], ("# enemies", 3), "page 4 row 5 = the current sector's enemies byte");
        // Ctrl+O on soldiers of slot 15 (8 SEEN entries) and 16 (SEEN: alone, cursor.md "Opponent lists").
        for (slot, seen) in [(15usize, 8usize), (16, 0)] {
            let mut w = base.clone();
            w.soldiers[20 * 512 + 0x17A] = slot as u8;
            w.tiles[t as usize * 8 + 5] = 20;
            let mut p = Play::new(1);
            let mut s = Session::new(&mut w, &mut p.host);
            level4(&mut s);
            let m = s.debug_key(d::CTRL_O, Some(t));
            assert_eq!(m.len(), 4, "{m:?}");
            assert_eq!(m[3].split(' ').filter(|x| !x.is_empty()).count(), 1 + seen, "slot {slot}: {m:?}");
        }
        // Ctrl+T with the turn flag on: the "first merc" byte takes `selected` only below 8; a selected value past
        // the soldier table does nothing (the original reads a soldier pointer past its 48 entries: no defined result).
        for (sel, kept) in [(7u8, 7u8), (8, 0x55), (48, 0x55)] {
            let mut w = base.clone();
            let mut p = Play::new(1);
            w.selected = sel;
            (w.update.turn_moves, w.update.kept_merc) = (1, 0x55);
            let before = (w.soldiers.clone(), w.knowledge.clone());
            let mut s = Session::new(&mut w, &mut p.host);
            level4(&mut s);
            s.debug_key(d::CTRL_T, Some(t));
            if sel < 48 {
                assert_eq!(s.cw.kept_merc, kept, "selected {sel}");
            } else {
                assert_eq!((s.cw.kept_merc, s.cw.turn_moves), (0x55, 1), "selected 48: no next turn");
                drop(s);
                assert!(before == (w.soldiers.clone(), w.knowledge.clone()), "selected 48: no soldier changes");
            }
        }
        d::set_level(0);
    }

    /// Ledger row 0b4d529:ja1front:18776 (panel.md "The switch view"): a press on switch i of slot k flips byte
    /// 0x16A + i of merc k's record, not of record 0.
    #[test]
    fn a_switch_press_flips_the_switch_of_its_own_merc() {
        let (exe, jag, save) = crate::world::tests::files(&[]);
        let mut w = World::from_files(&exe, &jag, &save).unwrap();
        let mut p = Play::new(1);
        let (k, i) = (3usize, 1usize);
        let (mine, first) = (w.soldiers[k * REC + 0x16A + i], w.soldiers[0x16A + i]);
        p.switch_press(&mut w, k as u8, i);
        assert_eq!(w.soldiers[k * REC + 0x16A + i], mine ^ 1);
        assert_eq!(w.soldiers[0x16A + i], first);
    }

    /// Ledger row c2aea50:ja1front:18306: a view mode change (key 0 from mode 3) hides the path marks.
    #[test]
    fn a_view_mode_change_hides_the_marks() {
        let (exe, jag, save) = crate::world::tests::files(&[]);
        let mut w = World::from_files(&exe, &jag, &save).unwrap();
        let mut p = Play::new(1);
        w.opt_onscreen = 3;
        (w.update.marks_shown, w.update.marks_n, w.path_preview) = (1, 0, 0);
        p.view_key(&mut w, 0);
        assert_eq!(w.opt_onscreen, 0);
        assert_eq!(w.update.marks_shown, 0);
    }

    /// Ledger rows d057aca:ja1front:17970/18164/18171/18174/18192/18278: the small key and press rules of the
    /// tactical screen (Fast Fire switch, Ctrl+F, the strip press shape, Enter, the Ctrl+T turn).
    #[test]
    fn the_tactical_key_rules() {
        let (exe, jag, save) = crate::world::tests::files(&[]);
        let mut w = World::from_files(&exe, &jag, &save).unwrap();
        crate::ctlpanel::set(&mut w, 13, 1);
        assert!(super::fast_fire_on(&w));
        crate::ctlpanel::set(&mut w, 13, 0);
        assert!(!super::fast_fire_on(&w));
        assert!(!super::ctrl_f_runs(0) && !super::ctrl_f_runs(7) && super::ctrl_f_runs(8) && super::ctrl_f_runs(0xFF));
        assert!(super::enter_runs(2, 1, 1));
        assert!(!super::enter_runs(8, 1, 1) && !super::enter_runs(2, 0, 1) && !super::enter_runs(2, 1, 0));
        assert!(super::strip_press_shapes(false, 1, 15));
        assert!(!super::strip_press_shapes(true, 1, 15) && !super::strip_press_shapes(false, 0, 15));
        assert!(!super::strip_press_shapes(false, 1, 14) && !super::strip_press_shapes(false, 1, -20));
        assert!(super::ctrl_t_next_turn(1) && !super::ctrl_t_next_turn(0));
    }

    /// Ledger rows d057aca:ja1front:17987 (overview press of a selected merc reads its own record, no panic) and
    /// 18513 (the switch release runs Reserve AP for switches 0 and 1 only).
    #[test]
    fn the_overview_press_and_the_switch_release() {
        let (exe, jag, save) = crate::world::tests::files(&[]);
        let mut w = World::from_files(&exe, &jag, &save).unwrap();
        let r = crate::soldier::REC;
        w.tiles[8 * 100 + 5] = 0x40;
        w.tiles[8 * 101 + 5] = 3;
        for k in [0u8, 1, 7, 48, 0xFF] {
            w.selected = k;
            let mut p = Play::new(1);
            let mut s = Session::new(&mut w, &mut p.host);
            assert!(s.overview_press_walks(100));
            assert!(!s.overview_press_walks(101));
        }
        let mut p = Play::new(1);
        (w.soldiers[3 * r], w.soldiers[3 * r + 0xEF]) = (0, 9);
        p.switch_release(&mut w, 3, 2);
        assert_eq!(w.soldiers[3 * r + 0xEF], 9);
        p.switch_release(&mut w, 3, 1);
        assert_eq!(w.soldiers[3 * r + 0xEF], 0);
    }

    /// Ledger row d057aca:ja1front:18419 (panel.md "The overview click"): the path marks go only when the click
    /// changes the view mode.
    #[test]
    fn an_overview_click_hides_the_marks_only_on_a_mode_change() {
        let (exe, jag, save) = crate::world::tests::files(&[]);
        let mut w = World::from_files(&exe, &jag, &save).unwrap();
        let mut p = Play::new(1);
        (w.opt_onscreen, w.view_old_mode, w.update.marks_shown, w.path_preview) = (1, 1, 1, 0);
        p.overview_jump(&mut w, 500);
        assert_eq!(w.update.marks_shown, 1, "marks hidden with no mode change");
        (w.opt_onscreen, w.view_old_mode) = (3, 1);
        p.overview_jump(&mut w, 500);
        assert_eq!((w.opt_onscreen, w.update.marks_shown), (1, 0));
    }

    /// Ledger row d057aca:ja1front:18589 (SlotMouse 0x1196C): a press on face k reads merc k's record: a merc that is
    /// not on the map takes no press, also when merc 0 would.
    #[test]
    fn a_face_press_reads_the_record_of_its_slot() {
        let (exe, jag, save) = crate::world::tests::files(&[]);
        let mut w = World::from_files(&exe, &jag, &save).unwrap();
        let r = crate::soldier::REC;
        w.update.screen_drawn = 1;
        for k in 0..8 {
            let x = &mut w.soldiers[k * r..(k + 1) * r];
            (x[0x182], x[0x194], x[0x138], x[0x17E], x[0x183]) = (1, 0, (k != 1) as u8, 0, 50);
        }
        let mut p = Play::new(1);
        p.pass(&mut w, &[super::Input::Portrait(1, false)], 0);
        assert!(!p.face_press, "a press on the face of a merc off the map");
    }

    /// Ledger row c2aea50:ja1front:18090: Reserve AP (ReserveAp, sector_start.md) of a merc with nothing in hand
    /// sets its reserve byte +0xEF to 0.
    #[test]
    fn reserve_ap_writes_the_reserve() {
        let (exe, jag, save) = crate::world::tests::files(&[]);
        let mut w = World::from_files(&exe, &jag, &save).unwrap();
        let mut p = Play::new(1);
        let r = crate::soldier::REC;
        (w.soldiers[3 * r], w.soldiers[3 * r + 0xEF]) = (0, 9);
        Session::new(&mut w, &mut p.host).reserve_ap(3);
        assert_eq!(w.soldiers[3 * r + 0xEF], 0);
    }
    /// TacticalKeyHandler 0x13DE4: the key flags (Alt, Ctrl, Shift at 0x1409B) run after the key part and before
    /// the mouse part (0x141EC on). The inputs of the mouse part are the tile clicks and holds, the button poll,
    /// the portraits and faces and the strip press; a key input is not one of them, so the flags of its pass wait.
    #[test]
    fn the_key_flags_run_before_a_mouse_input_only() {
        use super::Input::*;
        let (tile, shift, alt) = (5, false, false);
        for i in [Click { tile, shift, alt }, RClick { tile, shift, alt }, Drag { tile, dir: 1, shift, alt },
                  AllMove { tile, shift, alt }, RHold { tile, shift, alt, y: 9 }, Poll { left: true, right: false, y: 9 },
                  Portrait(1, false), Face(1), Strip { x: 40, right: false, ctrl: false }] {
            assert!(i.is_mouse(), "{i:?}");
        }
        for i in [Escape, NextMerc, Swap, Arrow(1), EndTurn, Merc(1, false), View(1), Pause, Other] {
            assert!(!i.is_mouse(), "{i:?}");
        }
    }

    /// TacticalKeyHandler 0x140F8..0x1411B: a Ctrl press keeps the mode in 0x284F22, sets mode 3 and runs
    /// MoveCursorState? 0x1C464 of the selected merc. That calls HidePathMarks? 0x1D8B8 (0x1D8BC..0x1D8CF: AP
    /// number 0x229DEC = 0, cursor tile 0x229D2C = 0xC41) before mode 3 gives the hand 0x94.
    #[test]
    fn a_ctrl_press_runs_the_move_cursor_state_of_the_selected_merc() {
        let (exe, jag, save) = crate::world::tests::files(&[]);
        let mut w = World::from_files(&exe, &jag, &save).unwrap();
        w.selected = 0;
        let mut host = PlayHost::new(1);
        let mut s = Session::new(&mut w, &mut host);
        let mut c = crate::cursor::CursorState { mode: 1, value: 0xA1, ap: 5, tile: 0x100, ..Default::default() };
        let mut l = crate::cursor::KeyLatches::default();
        super::Play::key_flags_step(&mut s, &mut c, &mut l, 0x104);
        assert_eq!((c.mode, c.value, c.ap, c.tile, l.ctrl, l.saved_mode), (3, 0x94, 0, 0xC41, true, 1));
    }

    /// Ledger rows c2aea50:ja1front:18063/18064 (Insert #12730): CursorItemBack? 0x53660 (cursorback_diff checks the
    /// raw routine against the original) through the session the window's keys and the guard screen use: the item on
    /// the cursor goes into strip box entry 0 of the selected merc, the cursor empties, the answer is yes; with an
    /// empty cursor nothing moves and the answer is no.
    #[test]
    fn the_session_puts_the_cursor_item_back() {
        let (exe, jag, save) = crate::world::tests::files(&[]);
        let mut w = World::from_files(&exe, &jag, &save).unwrap();
        w.selected = 0;
        w.update.screen_drawn = 1;
        w.inv.cursor_drop = 1;
        w.inv.strip_box = 0;
        w.soldiers[..12].fill(0);
        let item = [0x21, 0, 3, 0, 0, 0, 0, 0, 0, 0, 0, 0];
        w.held[..12].copy_from_slice(&item);
        let mut host = PlayHost::new(1);
        assert!(Session::new(&mut w, &mut host).cursor_back());
        assert_eq!((&w.soldiers[..12], &w.held[..12], w.inv.cursor_drop), (&item[..], &[0u8; 12][..], 0));
        // An empty cursor: no.
        w.inv.cursor_drop = 1;
        assert!(!Session::new(&mut w, &mut host).cursor_back());
        assert_eq!(&w.soldiers[..12], &item[..]);
    }

    /// Ledger row d057aca:ja1front:7146: SlotMouse 0x118EA asks OnScreen 0x1FC34 of the selected merc before "cont"
    /// shows; with the "on screen" option a merc out of the view answers no, and no merc (slot 8 and up) answers no.
    #[test]
    fn a_merc_out_of_the_view_is_not_on_screen() {
        let (exe, jag, save) = crate::world::tests::files(&[]);
        let mut w = World::from_files(&exe, &jag, &save).unwrap();
        w.selected = 0;
        w.opt_onscreen = 1;
        w.view_x = 30000;
        w.view_y = 30000;
        let mut host = PlayHost::new(1);
        {
            let mut s = Session::new(&mut w, &mut host);
            assert!(!s.slot_on_screen(0));
            assert!(!s.slot_on_screen(8));
        }
        // With the option off every merc counts as on screen (the library reads the world when the session opens).
        w.opt_onscreen = 0;
        let mut s = Session::new(&mut w, &mut host);
        assert!(s.slot_on_screen(0));
        assert!(!s.slot_on_screen(0xFF));
    }

    /// cursor.md "Clicks", right step 3.4: the posture change wrote its text before the hold is asked again, so
    /// that text stays when the hold answer has none; the wait and the other parts are the hold answer's.
    #[test]
    fn a_posture_confirm_keeps_its_text_over_the_hold_answer() {
        use crate::cursor::ClickOut;
        let o2 = ClickOut { look: true, popup: Some(7), ..Default::default() };
        let o = super::Play::posture_out(Some("first".into()), o2.clone());
        assert_eq!(o, ClickOut { status: Some("first".into()), ..o2.clone() });
        assert_eq!(super::Play::posture_out(None, o2.clone()), o2);
        let o3 = ClickOut { status: Some("second".into()), ..Default::default() };
        assert_eq!(super::Play::posture_out(None, o3.clone()).status.as_deref(), Some("second"));
        assert_eq!(super::Play::posture_out(Some("first".into()), o3).status.as_deref(), Some("first"));
    }

    /// strategic.md "The replay pictures": a picture with a picture right behind it is not shown; a sound takes no
    /// time; the last picture is shown.
    #[test]
    fn replay_shows_the_picture_before_a_wait() {
        use ReplayEv::*;
        let f = |b: u8| Frame(vec![b]);
        let ev = vec![f(0), Wait(90), f(1), Sound("EXPLODE", 104), f(2), Wait(400), f(3), f(4), Fx(1, 50), f(5)];
        assert_eq!(replay_shown(&ev), [true, false, false, false, true, false, false, false, false, true]);
    }

    /// EndTurn? 0x121D8: MouseSetPos? 0x6820B also writes the game's place, so a slide in the enemies' turn saves (80, 8) (live, fight59).
    #[test]
    fn turn_mouse_sets_the_game_place() {
        let mut host = PlayHost::new(1);
        host.mouse_now = (200, 100);
        crate::pointer::mouse_poll((200, 100));
        let ctx = &mut host as *mut PlayHost as *mut c_void;
        let args = [80, 8];
        unsafe {
            cb_call(ctx, 146, args.as_ptr(), 2, std::ptr::null_mut(), 0);
            cb_call(ctx, 147, std::ptr::null(), 0, std::ptr::null_mut(), 0);
        }
        assert_eq!(crate::pointer::game_place(), (80, 8));
        assert_eq!(crate::pointer::mouse_saved(), (80, 8));
    }

    use super::*;

    /// SelectMerc 0x1148C (the end of its code at 0x116E2): the "cont" of the strip goes only when the choice took
    /// the open order (+0x16D) of a merc: it was set before and is 0 after.
    #[test]
    fn the_cont_goes_only_with_a_taken_order() {
        let mut soldiers = vec![0u8; 48 * crate::soldier::REC];
        soldiers[3 * crate::soldier::REC + 0x16D] = 5;
        soldiers[7 * crate::soldier::REC + 0x16D] = 1;
        soldiers[3 + 0x16D] = 9; // not an order byte
        assert_eq!(open_orders(&soldiers), [0, 0, 0, 5, 0, 0, 0, 1]);
        assert!(order_taken(&[0, 0, 0, 5, 0, 0, 0, 1], &[0, 0, 0, 0, 0, 0, 0, 1]));
        assert!(!order_taken(&[0, 0, 0, 5, 0, 0, 0, 1], &[0, 0, 0, 5, 0, 0, 0, 1]), "no order went");
        assert!(!order_taken(&[0; 8], &[0; 8]), "no order was open");
        assert!(!order_taken(&[0, 2, 0, 0, 0, 0, 0, 0], &[3, 2, 0, 0, 0, 0, 0, 0]), "a new order is no taken order");
    }

    /// The story find of the tactical loop (finder slot 0x92DE3): no scene for the "no finder" slot 0x40, nor while
    /// the finder's own health is below 15.
    #[test]
    fn the_story_scene_waits_for_a_finder_with_15_health() {
        let mut soldiers = vec![0u8; 48 * 512];
        soldiers[0x183] = 50; // slot 0 is fit; it is not the finder
        soldiers[512 + 0x183 + 3] = 50; // (slot 1's neighbour bytes)
        for (hp, fit) in [(14u8, false), (15, true), (100, true), (0, false), (0xF0, false)] {
            soldiers[3 * 512 + 0x183] = hp;
            assert_eq!(finder_fit(&soldiers, 3), fit, "health {}", hp as i8);
        }
        assert!(!finder_fit(&soldiers, 1) && finder_fit(&soldiers, 0));
        assert!(!finder_fit(&soldiers, 0x40));
    }

    /// panel.md "The small panel in the full view": Show (after an op 196, in mode 3): window 20 x 13, small panel =
    /// the op's time, strip flag = 1. Countdown: 10 per timer tick while no portrait is lit; at 0 panel ends = 1.
    #[test]
    fn the_small_panel_shows_and_counts_down() {
        let (exe, jag, save) = crate::world::tests::files(&[]);
        let mut w = World::from_files(&exe, &jag, &save).unwrap();
        let mut p = Play::new(1);
        (w.opt_onscreen, w.small_panel, w.strip_flag, w.view_h, w.panel_ends) = (3, 0, 0, 14 * 14, 0);
        w.update.round_lock2 = 0;
        p.host.select.panel = Some((0, 0));
        p.host.select.panel_timer = 3000;
        p.host.select.mouse_y = p.mouse_px.1;
        p.hover.lit = 0x40;
        p.small_panel(&mut w, 0, false);
        assert_eq!((w.small_panel, w.strip_flag, w.view_h), (3000, 1, 14 * 13));
        assert!(p.host.select.panel.is_none());
        p.small_panel(&mut w, 299, false);
        assert_eq!((w.small_panel, w.panel_ends), (10, 0));
        p.small_panel(&mut w, 1, false);
        assert_eq!((w.small_panel, w.panel_ends), (0, 1));
    }

    /// Quick save block 78 (QuickRestore 0x2C898 reads the 10 tile animations of 0x229E24 back): an entry keeps the
    /// pointer to its frame table as the file holds it, 0x95659 (the ripple) or 0x9562E (the fire) + DOS/4GW's load
    /// base 0x1F1000. A ripple that was saved goes on after the restore and is saved again as it was.
    #[test]
    fn a_saved_ripple_goes_on_after_a_quick_restore() {
        let mut b = [0u8; 110];
        for (k, ptr) in [(1usize, 0x286659u32), (2, 0x28662E)] {
            let e = &mut b[11 * k..11 * k + 11];
            e[0] = 7;
            e[1..5].copy_from_slice(&40i32.to_le_bytes());
            e[5..7].copy_from_slice(&(900i16 + k as i16).to_le_bytes());
            e[7..11].copy_from_slice(&ptr.to_le_bytes());
        }
        let mut a = AnimTimers::default();
        a.from_block(&b);
        assert_eq!((a.frames[1], a.frames[2], a.frames[3]), (RIPPLE_FRAMES, FIRE_FRAMES, 0));
        assert_eq!((a.tiles[1].count, a.tiles[1].timer, a.tiles[1].tile), (7, 40, 901));
        assert_eq!(a.to_block(), b);
    }


    /// TacticalKeyHandler 0x134A0: MousePoll? runs only with screen_drawn 0x92DCC set, so the game's mouse place
    /// 0x229D00 keeps its old value in a pass with that byte 0 (live, #31 comment 11879).
    #[test]
    fn pass_polls_the_mouse_only_with_the_screen_drawn() {
        let (exe, jag, save) = crate::world::tests::files(&[]);
        let mut w = World::from_files(&exe, &jag, &save).unwrap();
        let mut p = Play::new(1);
        p.left = true; // the pass body returns at once: only the poll gate runs
        crate::pointer::mouse_poll((10, 20));
        p.mouse_px = (200, 100);
        w.update.screen_drawn = 0;
        p.pass(&mut w, &[], 1);
        assert_eq!(crate::pointer::game_place(), (10, 20));
        w.update.screen_drawn = 1;
        p.pass(&mut w, &[], 1);
        assert_eq!(crate::pointer::game_place(), (200, 100));
    }

    /// ShowMercMessage? 0x299B8 inside a pass: the library's timer runs in the wait, so the rest of the pass reads
    /// the timer's values. Under the balloon's Lock only the show timer (and the ambient timers) run.
    #[test]
    fn balloon_wait_runs_the_timer_inside_the_pass() {
        let (exe, jag, save) = crate::world::tests::files(&[]);
        let mut w = World::from_files(&exe, &jag, &save).unwrap();
        w.update.show_timer = 300;
        w.update.shown_slot = 2;
        w.update.busy_timer = 300;
        w.update.tick_step = 10;
        (w.update.round_lock, w.update.round_lock2) = (0, 0);
        w.busy = 1;
        let mut host = PlayHost::new(1);
        let mut s = Session::new(&mut w, &mut host);
        s.bind();
        let (cw, hp) = (&mut s.cw as *mut Ja1World, s.h.ctx as *mut PlayHost);
        unsafe {
            // Locked: the show timer ends, the busy timer stands.
            (*hp).timer_in_pass(29, true);
            assert_eq!(((*cw).show_timer, (*cw).shown_slot, (*cw).busy_timer, (*cw).round_lock), (10, 2, 300, 0));
            (*hp).timer_in_pass(1, true);
            assert_eq!(((*cw).show_timer, (*cw).shown_slot, (*cw).busy_timer), (0, 0x40, 300));
            // The release wait before the Lock: the whole timer runs.
            (*hp).timer_in_pass(30, false);
            assert_eq!(((*cw).busy_timer, (*cw).busy), (0, 0));
        }
    }

    #[test]
    fn balloon_wait_runs_the_animation_timers_inside_the_pass() {
        // TacticalTimer? 0x5EEA4 steps 1-2: the tile and effect animation timers run in a balloon's release wait
        // inside the pass (ShowMercMessage? 0x299B8), under the gates; the Lock stops them.
        let (exe, jag, save) = crate::world::tests::files(&[]);
        let mut w = World::from_files(&exe, &jag, &save).unwrap();
        (w.update.round_lock, w.update.round_lock2) = (0, 0);
        let mut host = PlayHost::new(1);
        let tile = 1000;
        assert!(host.anims.start(tile, 0, &mut w.tiles).is_some());
        w.fx_anims[0] = 5;
        host.anims.fx_timers[0] = 3;
        let mut s = Session::new(&mut w, &mut host);
        s.bind();
        let (cw, hp) = (&mut s.cw as *mut Ja1World, s.h.ctx as *mut PlayHost);
        unsafe {
            let tiles = |cw: *mut Ja1World| std::slice::from_raw_parts_mut((*cw).tiles, 3136 * 8).to_vec();
            let fx = |cw: *mut Ja1World| std::slice::from_raw_parts((*cw).fx_anims, crate::world::FX_ANIMS).to_vec();
            let (mut t0, mut f0, mut a0) = (tiles(cw), fx(cw), (*hp).anims.clone());
            // Locked: no animation step.
            (*hp).timer_in_pass(7, true);
            assert_eq!((tiles(cw), fx(cw), format!("{:?}", (*hp).anims)), (t0.clone(), f0.clone(), format!("{:?}", a0)));
            // The release wait: 7 steps, as 7 steps of AnimTimers::step on the same bytes.
            (*hp).timer_in_pass(7, false);
            for _ in 0..7 {
                a0.step(&mut t0, &mut f0);
            }
            assert_eq!((tiles(cw), fx(cw), format!("{:?}", (*hp).anims)), (t0.clone(), f0.clone(), format!("{:?}", a0)));
            assert_ne!(format!("{:?}", (*hp).anims.fx_timers), format!("{:?}", [3, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0]));
            // The ripple ends in a long wait: byte 3 of its tile (1000, beyond the first 392 bytes) goes back to 0.
            assert_ne!(t0[tile as usize * 8 + 3], 0);
            (*hp).timer_in_pass(2000, false);
            for _ in 0..2000 {
                a0.step(&mut t0, &mut f0);
            }
            assert_eq!(t0[tile as usize * 8 + 3], 0, "the ripple did not end in 20 s");
            assert_eq!(tiles(cw), t0, "the wait in the pass did not end the ripple of tile 1000");
        }
    }

    #[test]
    fn merc_quote_records() {
        // world_load.md "The last quote": the merc quotes (speaker profile, category, row, quote; member).
        let prof = |k: i32| [16u8, 32, 23][k as usize];
        let run = |op: i32, a: &[i32]| {
            let (mut q, mut m) = ([0xAA; 6], 0xEE);
            merc_quote(&mut q, &mut m, prof, op, a);
            (q, m)
        };
        assert_eq!(run(291, &[0, 32]), ([16, 2, 240, 0, 6, 0], 32));
        assert_eq!(run(289, &[2, 9, 32, 1]), ([23, 2, 89, 1, 9, 0], 32));
        assert_eq!(run(295, &[1, 23]), ([0xAA; 6], 23));
        assert_eq!(run(296, &[1, 3, 23]), ([32, 2, 224, 1, 3, 0], 0xEE));
        assert_eq!(run(298, &[1, 2]).0, [32, 2, 224, 1, 13, 0]);
        assert_eq!(run(298, &[1, 0]).0, [32, 2, 224, 1, 14, 0]);
        assert_eq!(run(328, &[0, 0, 0, 25]).0, [25, 0, 94, 1, 9, 0]);
        assert_eq!(run(252, &[25]).0, [25, 1, 57, 3, 6, 0]);
        assert_eq!(run(267, &[0, 0, 12]).0, [12, 2, 180, 0, 12, 0]);
        assert_eq!(run(276, &[12]).0, [12, 2, 180, 0, 0, 0]);
    }

    #[test]
    fn stash_ops() {
        let mut r = Vec::new();
        stash_put(&mut r, 5, 1, 50);
        stash_put(&mut r, 3, 1, 90);
        stash_put(&mut r, 5, 1, 80);
        assert_eq!(r, vec![[3, 1, 90], [5, 1, 80], [5, 1, 50]]);
        assert_eq!(stash_take(&mut r, 3, false), 1);
        assert_eq!(stash_take(&mut r, 9, true), 0);
        stash_take(&mut r, 5, true);
        assert_eq!(r, vec![[3, 1, 90], [5, 1, 50]]);
        // The last record with take = 0 is missed (as the original).
        assert_eq!(stash_take(&mut r, 5, false), 0);
        assert_eq!(stash_take(&mut r, 5, true), 1);
        assert!(r.len() == 1);
    }

    #[test]
    fn abort_lines_parse() {
        assert_eq!(parse_script("abort 1\nabort 0\n").unwrap(), vec![Cmd::Abort(true), Cmd::Abort(false)]);
        assert_eq!(parse_script("trav N\ntrav w\n").unwrap(), vec![Cmd::Trav(10), Cmd::Trav(13)]);
        assert!(parse_script("trav X\n").is_err());
        assert!(parse_script("abort 2").is_err());
        assert!(parse_script("abort").is_err());
    }

    fn tiles() -> Vec<u8> {
        vec![0x55; 3136 * 8]
    }

    #[test]
    fn fire_starts_on_a_bare_tile_and_shows_its_frames() {
        // combat.md "Drawing the effect records": kind 3, count 5, tile byte 3 = 0 and byte 0 below 0x6C.
        let mut fx = vec![0u8; FX_REC * FX_ANIM_COUNT];
        let mut tiles = vec![0u8; 3136 * 8];
        fx[FX_REC * 2] = 5;
        fx[FX_REC * 2 + 0xB] = 3;
        fx[FX_REC * 2 + 9..FX_REC * 2 + 11].copy_from_slice(&700i16.to_le_bytes());
        assert_eq!(fire_tile(&fx, 2, &tiles), Some(700));
        tiles[700 * 8] = 0x6C;
        assert_eq!(fire_tile(&fx, 2, &tiles), None);
        tiles[700 * 8] = 0x10;
        fx[FX_REC * 2 + 0xB] = 2;
        assert_eq!(fire_tile(&fx, 2, &tiles), None);
        fx[FX_REC * 2 + 0xB] = 3;
        let mut a = AnimTimers::default();
        assert_eq!(a.start_table(700, FIRE_FRAMES, FIRE_TABLE_COUNT, &mut tiles), Some(1));
        assert_eq!(tiles[700 * 8 + 3], 1);
        assert_eq!(fire_tile(&fx, 2, &tiles), None);
        // tactical_map.md "Animated tiles": frame = table[count]; count 0x2A -> 0x80, count 1 -> 0x89.
        assert_eq!(a.sprite_frames()[1], Some(0x80));
        a.tiles[1].count = 1;
        assert_eq!(a.sprite_frames()[1], Some(0x89));
        a.tiles[1].count = 0;
        assert_eq!(a.sprite_frames()[1], None);
        // The ripple keeps its own table.
        assert_eq!(a.start(900, 0, &mut tiles), Some(1));
        assert_eq!(a.sprite_frames()[1], Some(0xD0));
    }

    #[test]
    fn tile_anim_counts_down_and_restarts_at_140() {
        let mut a = AnimTimers::default();
        a.tiles[0] = TileAnim { count: 2, timer: 20, tile: 100 };
        let (mut t, mut fx) = (tiles(), vec![0u8; 16 * 0x12]);
        a.step(&mut t, &mut fx);
        assert_eq!(a.tiles[0], TileAnim { count: 2, timer: 10, tile: 100 });
        a.step(&mut t, &mut fx);
        // Timer 0 (<= 0): count 1, still > 0: timer restarts at 140.
        assert_eq!(a.tiles[0], TileAnim { count: 1, timer: 140, tile: 100 });
        assert_eq!(t[100 * 8 + 3], 0x55);
        for _ in 0..13 {
            a.step(&mut t, &mut fx);
        }
        assert_eq!(a.tiles[0], TileAnim { count: 1, timer: 10, tile: 100 });
        assert_eq!(t[100 * 8 + 3], 0x55);
        a.step(&mut t, &mut fx);
        // The count runs out: tile byte 3 = 0, no tile.
        assert_eq!(a.tiles[0].count, 0);
        assert_eq!(a.tiles[0].tile, NO_TILE);
        assert_eq!(t[100 * 8 + 3], 0);
        assert_eq!(t[100 * 8 + 2], 0x55);
        assert_eq!(t[100 * 8 + 4], 0x55);
        // Count 0: nothing more happens.
        a.step(&mut t, &mut fx);
        assert_eq!(a.tiles[0].timer, 0);
    }

    #[test]
    fn tile_anim_tile_is_signed_16_bit() {
        let (mut t, mut fx) = (tiles(), vec![0u8; 16 * 0x12]);
        let mut a = AnimTimers::default();
        // 0xC41 and above (as s16): byte 3 stays, the tile stays.
        a.tiles[1] = TileAnim { count: 1, timer: 10, tile: 0xC41 };
        a.tiles[2] = TileAnim { count: 1, timer: 10, tile: 3135 };
        // 0xC40 (3136) is below 0xC41 but off the map: nothing is written, the animation has no tile.
        a.tiles[5] = TileAnim { count: 1, timer: 10, tile: 0xC40 };
        // 0x8000 as s16 is negative (below 0xC41): off the map, nothing is written, but the animation has no tile.
        a.tiles[3] = TileAnim { count: 1, timer: 10, tile: 0x8000u16 as i16 };
        // A negative count does not run.
        a.tiles[4] = TileAnim { count: -1, timer: 10, tile: 5 };
        a.step(&mut t, &mut fx);
        assert_eq!(a.tiles[1], TileAnim { count: 0, timer: 0, tile: 0xC41 });
        assert_eq!(a.tiles[2], TileAnim { count: 0, timer: 0, tile: NO_TILE });
        assert_eq!(t[3135 * 8 + 3], 0);
        assert_eq!(a.tiles[5].tile, NO_TILE);
        assert_eq!(a.tiles[3].tile, NO_TILE);
        assert_eq!(a.tiles[4], TileAnim { count: -1, timer: 10, tile: 5 });
        assert_eq!(t.iter().filter(|&&b| b == 0).count(), 1);
    }

    #[test]
    fn fx_anim_counts_down_8_bit() {
        let (mut t, mut fx) = (tiles(), vec![0u8; 16 * 0x12]);
        let mut a = AnimTimers::default();
        fx[0] = 2;
        fx[0x12] = 0x80; // -128: != 0, wraps to 0x7F
        fx[2 * 0x12] = 0; // count 0: timer stays
        a.fx_timers[0] = 20;
        a.fx_timers[2] = 30;
        a.step(&mut t, &mut fx);
        assert_eq!((fx[0], a.fx_timers[0]), (2, 10));
        // fx 1: timer 0 - 10 <= 0: restarts at 140, count - 1 (8-bit).
        assert_eq!((fx[0x12], a.fx_timers[1]), (0x7F, 140));
        assert_eq!(a.fx_timers[2], 30);
        a.step(&mut t, &mut fx);
        assert_eq!((fx[0], a.fx_timers[0]), (1, 140));
        for _ in 0..14 {
            a.step(&mut t, &mut fx);
        }
        assert_eq!((fx[0], a.fx_timers[0]), (0, 140));
        a.step(&mut t, &mut fx);
        assert_eq!((fx[0], a.fx_timers[0]), (0, 140));
        assert!(t.iter().all(|&b| b == 0x55));
    }

    #[test]
    fn panel_view_rule() {
        let mut ui = SelectUi { mouse_y: 190, ..SelectUi::default() };
        // Mouse at y >= 182 and room below: 14 pixels down, the mouse 14 up.
        assert_eq!(ui.panel_view(100), 114);
        assert_eq!((ui.mouse_y, ui.panel, ui.panel_timer), (176, Some((20, 13)), 3000));
        // The mouse is now above 182: no scroll.
        assert_eq!(ui.panel_view(100), 100);
        // No room: view_y + 14 must stay below 0x310 - 182.
        ui.mouse_y = 200;
        assert_eq!(ui.panel_view(0x310 - 182 - 14), 0x310 - 182 - 14);
        assert_eq!(ui.panel_view(0x310 - 182 - 15), 0x310 - 182 - 1);
    }

    #[test]
    fn message_ops_and_sees_line() {
        let mut h = PlayHost::new(1);
        h.log = false;
        let ctx = &mut h as *mut PlayHost as *mut c_void;
        let call = |op: i32, a: &[i32]| unsafe { cb_call(ctx, op, a.as_ptr(), a.len() as i32, std::ptr::null_mut(), 0) };
        assert_eq!(call(203, &[]), 0);
        assert_eq!(h.msg_box, Some(("Enemies nearby!".to_string(), 1500)));
        assert_eq!(call(326, &[]), 0);
        assert_eq!(h.msg_box, Some(("Guard sees a new enemy!".to_string(), 25000)));
        // The line text: nickname up to the zero byte, "1 enemy" / "N enemies" from +0x1D5 (signed).
        let mut rec = vec![0u8; 512];
        rec[0x16E..0x174].copy_from_slice(b"Grunty");
        rec[0x1D5] = 1;
        assert_eq!(sees_text(&rec), b"Grunty sees 1 enemy!".to_vec());
        rec[0x1D5] = 3;
        assert_eq!(sees_text(&rec), b"Grunty sees 3 enemies!".to_vec());
        rec[0x1D5] = 0;
        assert_eq!(sees_text(&rec), b"Grunty sees 0 enemies!".to_vec());
        // Op 57 merc_text: "NAME TEXT" for a merc (+0x17A < 8) only.
        assert_eq!(merc_text(&rec, 6).as_deref(), Some("Grunty is all bandaged!"));
        assert_eq!(merc_text(&rec, 10).as_deref(), Some("Grunty doesn't have enough Action Points to traverse!"));
        assert_eq!(merc_text(&rec, 11).as_deref(), Some("Grunty not healthy enough!"));
        assert_eq!(merc_text(&rec, 12).as_deref(), Some("Grunty hit in head, causing extra damage!"));
        assert_eq!(merc_text(&rec, 13), None);
        rec[0x17A] = 8;
        assert_eq!(merc_text(&rec, 1), None);
    }

    /// MsgHearsNoise? 0x72BE0..0x72C3B: the text goes to StatusMessage? 0x27D24 (a box of 25 s).
    #[test]
    fn hears_op_shows_a_status_box() {
        let mut h = PlayHost::new(1);
        h.log = false;
        let mut cw: Ja1World = unsafe { std::mem::zeroed() };
        let mut soldiers = vec![0u8; 48 * 512];
        soldiers[2 * 512 + 0x16E..2 * 512 + 0x16E + 5].copy_from_slice(b"Ivan\0");
        cw.soldiers = soldiers.as_mut_ptr();
        h.world = &mut cw;
        let ctx = &mut h as *mut PlayHost as *mut c_void;
        let a = [2, 2, 1, 3];
        unsafe { cb_call(ctx, 213, a.as_ptr(), 4, std::ptr::null_mut(), 0) };
        let want = "Ivan hears a  LOUD MOVEMENT noise coming from the east".to_string();
        assert_eq!(h.msg_box, Some((want, 25000)));
    }

    #[test]
    fn select_ops_keep_ui_state() {
        let mut h = PlayHost::new(1);
        h.log = false;
        let ctx = &mut h as *mut PlayHost as *mut c_void;
        let call = |op: i32, a: &[i32]| unsafe { cb_call(ctx, op, a.as_ptr(), a.len() as i32, std::ptr::null_mut(), 0) };
        assert_eq!(call(195, &[3, 0xC]), 0);
        assert_eq!(call(195, &[0x103, 5]), 0); // byte slot: 0x103 -> 3
        assert_eq!(call(197, &[2]), 0);
        assert_eq!(call(200, &[]), 0);
        assert_eq!(call(198, &[]), 0);
        // op 196 answers the new view_y (here the mouse is at 0: unchanged).
        assert_eq!(call(196, &[77]), 77);
        assert_eq!(call(199, &[4]), 0); // no world bound: only the cursor slot
        assert_eq!(h.select.lights[3], 5);
        assert_eq!(h.select.sees_msg, Some(2));
        assert_eq!(h.select.cursor, Some((1, 0x8E)));
        assert_eq!(h.select.cursor_slot, Some(4));
        assert_eq!(h.select.panel_timer, 3000);
    }

    #[test]
    fn ops_55_and_199_hide_path_marks() {
        let mut tiles = vec![0x33u8; 3136 * 8];
        let mut marks = vec![0u8; 256];
        marks[0..4].copy_from_slice(&[10, 0, 20, 0]);
        // SAFETY: every pointer field is null except tiles and marks; the library reads only those here.
        let mut cw: Ja1World = unsafe { std::mem::zeroed() };
        cw.tiles = tiles.as_mut_ptr();
        cw.marks = marks.as_mut_ptr();
        cw.marks_shown = 1;
        cw.marks_n = 2;
        cw.opt_path_marks = 1;
        cw.path_preview = 3;
        let mut h = PlayHost::new(1);
        h.log = false;
        h.world = &mut cw;
        let ctx = &mut h as *mut PlayHost as *mut c_void;
        let call = |op: i32, a: &[i32]| unsafe { cb_call(ctx, op, a.as_ptr(), a.len() as i32, std::ptr::null_mut(), 0) };
        // Op 199 = Hide path marks (0): a preview that stays on (path_preview > 0) keeps its marks.
        call(199, &[1]);
        assert_eq!((cw.marks_shown, cw.marks_n, tiles[10 * 8 + 7]), (1, 2, 0x33));
        // Op 55 ui_refresh (1) forces it: the marked tiles' byte 7 and the list are cleared.
        call(55, &[1]);
        assert_eq!((cw.marks_shown, cw.path_preview, cw.marks_n), (0, 0, 0));
        assert_eq!((tiles[10 * 8 + 7], tiles[20 * 8 + 7], tiles[10 * 8 + 6]), (0, 0, 0x33));
        assert!(marks.iter().all(|&b| b == 0));
    }

    fn soldiers() -> Vec<u8> {
        vec![0u8; 48 * crate::soldier::REC]
    }

    fn rec(s: &mut [u8], k: usize) -> &mut [u8] {
        &mut s[k * crate::soldier::REC..(k + 1) * crate::soldier::REC]
    }

    #[test]
    fn script_eod_line() {
        assert_eq!(parse_script("eod 3\nwait 2\n# c\neod 0").unwrap(), vec![Cmd::Eod(3), Cmd::Wait(2), Cmd::Eod(0)]);
        assert_eq!(parse_script("hover 1483\nhover -1").unwrap(), vec![Cmd::Hover(1483), Cmd::Hover(-1)]);
        assert_eq!(parse_script("dragto 302 0 -20\ndragto up").unwrap(), vec![Cmd::DragTo(302, 0, -20), Cmd::DragUp]);
        assert!(parse_script("dragto 302 0").is_err());
        assert_eq!(parse_script("day\nmap 59 0 2\nstart 1").unwrap(), vec![Cmd::Day, Cmd::Map(59, 0, 2), Cmd::Start(1, 0)]);
        assert!(parse_script("day 1").is_err());
        assert!(parse_script("map 59 0").is_err());
        for bad in ["eod", "eod -1", "eod x", "eod 1 2"] {
            assert!(parse_script(bad).is_err(), "{}", bad);
        }
    }

    #[test]
    fn choose_merc_condition() {
        let mut s = soldiers();
        let r = rec(&mut s, 3);
        r[0x182] = 1;
        r[0x138] = 1;
        r[0x183] = 30;
        assert!(can_choose(&s, 3));
        assert!(!can_choose(&s, 2));
        assert!(!can_choose(&s, 8)); // keys give 0..7 only
        for (off, v) in [(0x182, 0u8), (0x138, 0), (0x183, 0), (0x183, 0x90), (0x188, 1)] {
            let mut c = s.clone();
            rec(&mut c, 3)[off] = v;
            assert!(!can_choose(&c, 3), "+{:#x} = {}", off, v);
        }
    }

    #[test]
    fn script_parsing() {
        let text = "# demo\n\nwait 3\n  click 1234  \nkey D\nkey d\nkey 1\nkey 8\n   # indented comment\nwait 0\n";
        assert_eq!(
            parse_script(text).unwrap(),
            vec![
                Cmd::Wait(3),
                Cmd::Do(Input::Click { tile: 1234, shift: false, alt: false }),
                Cmd::Do(Input::EndTurn),
                Cmd::Do(Input::EndTurn),
                Cmd::Do(Input::Merc(0, false)),
                Cmd::Do(Input::Merc(7, false)),
                Cmd::Wait(0),
            ]
        );
        for bad in ["key 0", "key 9", "key Q", "key 12", "wait", "wait -1", "click x", "walk 3", "wait 1 2"] {
            assert!(parse_script(bad).is_err(), "{}", bad);
        }
    }

    #[test]
    fn fx_timer_in_the_record_round_trip() {
        let mut a = AnimTimers::default();
        a.fx_timers[1] = 70;
        let mut fx = vec![0u8; FX_ANIM_COUNT * FX_REC];
        fx[FX_REC] = 5;
        fx[FX_REC + 5] = 0x11; // the record's tile stays
        a.fx_timers_to(&mut fx);
        assert_eq!(fx[FX_REC..FX_REC + 6], [5, 70, 0, 0, 0, 0x11]);
        let mut c = AnimTimers::default();
        c.fx_timers_from(&fx);
        assert_eq!(c.fx_timers, a.fx_timers);
        // The restored record loses a count after the rest of its 70 ms, not after its first step.
        let mut tiles = vec![0u8; 3136 * 8];
        for _ in 0..6 {
            c.step(&mut tiles, &mut fx);
        }
        assert_eq!(fx[FX_REC], 5);
        c.step(&mut tiles, &mut fx);
        assert_eq!((fx[FX_REC], c.fx_timers[1]), (4, 140));
    }

    #[test]
    fn tile_anim_block_round_trip() {
        // The bytes of QUICK_t10.SAV for a slot that ended: count 0, timer 0, tile 0xC41, the ripple table.
        let mut a = AnimTimers::default();
        let mut tiles = vec![0u8; 3136 * 8];
        a.start(500, 0, &mut tiles);
        a.tiles[1].timer = 70;
        a.start_table(600, FIRE_FRAMES, FIRE_TABLE_COUNT, &mut tiles);
        let b = a.to_block();
        assert_eq!(b[11..22], [0x11, 70, 0, 0, 0, 0xF4, 1, 0x59, 0x66, 0x28, 0]);
        assert_eq!(b[22..33], [0x2A, 0, 0, 0, 0, 0x58, 2, 0x2E, 0x66, 0x28, 0]);
        assert_eq!(b[..11], [0; 11]);
        let mut c = AnimTimers::default();
        c.from_block(&b);
        assert_eq!(c.tiles[1], a.tiles[1]);
        assert_eq!(c.frames[2], FIRE_FRAMES);
        assert_eq!(c.sprite_frames(), a.sprite_frames());
        // The restored ripple runs out and clears the tile's byte 3.
        tiles[500 * 8 + 3] = 1;
        let mut fx = vec![0u8; FX_ANIM_COUNT * 0x12];
        for _ in 0..0x12 * 14 + 7 {
            c.step(&mut tiles, &mut fx);
        }
        assert_eq!(tiles[500 * 8 + 3], 0);
    }

    #[test]
    fn tile_anim_start_ripple() {
        let mut t = vec![0u8; 3136 * 8];
        let mut a = AnimTimers::default();
        a.tiles[1].timer = 77;
        // Animation 0 is never used; the first free one of 1..9 gets count 0x11, the tile, byte 3 = its number.
        assert_eq!(a.start(500, 0, &mut t), Some(1));
        assert_eq!(a.tiles[1], TileAnim { count: 0x11, timer: 77, tile: 500 });
        assert_eq!((a.frames[1], t[500 * 8 + 3]), (RIPPLE_FRAMES, 1));
        assert_eq!(a.tiles[0].count, 0);
        a.tiles[2].count = -1; // not free (count != 0)
        assert_eq!(a.start(501, 0, &mut t), Some(3));
        assert_eq!(t[501 * 8 + 3], 3);
        // Kind != 0: nothing.
        assert_eq!(a.start(502, 1, &mut t), None);
        for k in 4..TILE_ANIMS {
            a.tiles[k].count = 1;
        }
        // No free animation: nothing.
        assert_eq!(a.start(503, 0, &mut t), None);
        assert_eq!(t[503 * 8 + 3], 0);
    }

    #[test]
    fn banner_table() {
        assert_eq!(Banner::from_kind(0), Some(Banner { kind: 0, text: "OPPONENTS' TURN...", colour: 0x72 }));
        assert_eq!(Banner::from_kind(3).map(|b| (b.text, b.colour)), Some(("GUARD INTERRUPTS!", 0xA2)));
        assert_eq!(Banner::from_kind(4).map(|b| b.colour), Some(0x72));
        assert_eq!(Banner::from_kind(5), None);
    }

    /// Ledger row 0b4d529:ja1front:17757: PlayWalterQuote 0x3C574 opens WPRO\wpro2 for quote 0x3C and up
    /// (asm 0x3C998 `cmp esi,0x3c; jl`), so quote 0x3C plays WP60.8 of WPRO2.
    #[test]
    fn walter_quote_60_comes_from_wpro2() {
        let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
        let (iso, exe) = (root.join("rom/JA1.gog"), root.join("build/game113b/JA.EXE"));
        if !iso.exists() || !exe.exists() {
            return;
        }
        let (iso, exe) = (iso.to_str().unwrap(), exe.to_str().unwrap());
        let member = |arc: &str, q: i32| {
            let a = crate::iso::Iso::open(iso).unwrap().read(arc).unwrap();
            crate::panel::archive_member(&a, &format!("WP{}.8", q)).ok()
        };
        let want60 = member("WPRO\\WPRO2", 0x3C).expect("WP60.8 in WPRO2");
        assert_eq!(super::walter_files(iso, exe, 0x3C).0.as_deref(), Some(&want60[..]));
        assert_eq!(super::walter_files(iso, exe, 0x3B).0, member("WPRO\\WPRO1", 0x3B));
        assert_ne!(member("WPRO\\WPRO1", 0x3C).as_deref(), Some(&want60[..]), "the two files differ at quote 60");
    }

    /// Ledger row ja1front:16761: op 195 face_light for a slot past the 48 lights changes no light and does not
    /// stop (the slot is a byte).
    #[test]
    fn a_face_light_past_the_last_slot_changes_nothing() {
        let mut ui = super::SelectUi::default();
        ui.op(195, &[47, 2]);
        ui.op(195, &[48, 3]);
        ui.op(195, &[200, 3]);
        assert_eq!(ui.lights[47], 2);
        assert!(ui.lights[..47].iter().all(|&b| b == 0));
    }

    /// Ledger row ja1front:16998: a tile animation for a tile whose byte 3 is past the map gets no byte 3 and
    /// does not stop.
    #[test]
    fn a_tile_animation_past_the_map_writes_no_byte() {
        let mut t = super::AnimTimers::default();
        let mut tiles = [0u8; 19];
        assert_eq!(t.start_table(1, super::RIPPLE_FRAMES, 4, &mut tiles), Some(1));
        assert_eq!(tiles[11], 1);
        assert_eq!(t.start_table(2, super::RIPPLE_FRAMES, 4, &mut tiles), Some(2));
        assert_eq!(tiles.iter().filter(|&&b| b != 0).count(), 1);
    }

    /// Ledger rows ja1front:17042/17085 (world_load.md "The last quote"): the quote is a u16 field, low byte first.
    #[test]
    fn the_last_quote_keeps_the_high_byte() {
        let mut lq = [0u8; 6];
        super::npc_quote(&mut lq, 302, &[0x123, 1]);
        assert_eq!(lq, [0x40, 1, 0, 0, 0x23, 1]);
        super::set_quote(&mut lq, 3, 1, 0x234);
        assert_eq!(lq, [3, 1, 99, 0, 0x34, 2]);
    }

    /// Ledger row ja1front:17117 (cursor.md "Off the map", 0x14308): the mouse off the map clears the cursor tile
    /// (0xC41) and the over-map flag.
    #[test]
    fn the_mouse_off_the_map_clears_the_cursor_tile() {
        let (exe, jag, save) = crate::world::tests::files(&[]);
        let mut w = World::from_files(&exe, &jag, &save).unwrap();
        let mut p = Play::new(1);
        let (mut st, mut hs) = (crate::cursor::CursorState::default(), crate::cursor::HoverState::default());
        (st.over_map, st.tile, st.ap) = (1, 0x123, 5);
        Session::new(&mut w, &mut p.host).off_map(&mut st, &mut hs);
        assert_eq!((st.over_map, st.tile, st.ap), (0, 0xC41, 0));
    }

    /// Ledger rows ja1front:17244/17246: the walter_say cursor reset (0x11418) and the op 139 marker clear (0x336B4)
    /// reach the window once each.
    #[test]
    fn the_cursor_reset_and_marker_clear_reach_the_window_once() {
        let (exe, jag, save) = crate::world::tests::files(&[]);
        let mut w = World::from_files(&exe, &jag, &save).unwrap();
        let mut p = Play::new(1);
        (p.host.cursor_reset, p.host.marker_clear) = (true, true);
        let mut s = Session::new(&mut w, &mut p.host);
        assert!(s.take_cursor_reset());
        assert!(!s.take_cursor_reset());
        assert!(s.take_marker_clear());
        assert!(!s.take_marker_clear());
    }

    /// Ledger rows ja1front:17513/17521/17529 (overview.md, OverviewMouse 0x24645): a mouse point that is not on
    /// the cursor's overview tile moves to that tile's point (4 x 3 pixels per tile from (48, 14)); the view window
    /// rectangle of mode 0 is (48, 14)..(271, 181) (14 x 12 tiles of 16 x 14).
    #[test]
    fn the_overview_cursor_takes_the_tile_point_and_the_view_rectangle() {
        let (exe, jag, save) = crate::world::tests::files(&[]);
        let mut w = World::from_files(&exe, &jag, &save).unwrap();
        w.opt_onscreen = 0;
        let mut p = Play::new(1);
        (p.mouse, p.mouse_px) = (Some(2 * 56 + 1), (0, 0));
        let o = p.ov_cursor(&w);
        assert_eq!((o.mouse_x, o.mouse_y), (54, 21));
        assert_eq!((o.left, o.top, o.right, o.bottom), (48, 14, 271, 181));
    }

    /// Ledger row ja1front:17620: only an op 344 pick-up opens the screen with the item info (0x54EC8); a strip
    /// click (op 349) opens it without; a stale info flag without a slot opens nothing with info.
    /// Ledger row ja1front:17637 (0x54D50 / 0x54EC8): after a pick-up screen with an item still on the cursor, the
    /// mouse limit goes on (cursor_drop 1) and the mouse goes to row 191.
    #[test]
    fn the_pick_up_screen_and_the_cursor_after_it() {
        assert!(super::pick_shows_info(true, Some(2)));
        assert!(!super::pick_shows_info(false, Some(2)));
        assert!(!super::pick_shows_info(true, None));
        let (exe, jag, save) = crate::world::tests::files(&[]);
        let mut w = World::from_files(&exe, &jag, &save).unwrap();
        let mut p = Play::new(1);
        w.held[0] = 7;
        w.inv.cursor_drop = 0;
        p.mouse_px = (100, 50);
        p.cursor_after(&mut w, false, true);
        assert_eq!((w.inv.cursor_drop, p.mouse_px, p.host.mouse_to), (1, (100, 191), Some((100, 191))));
    }

    /// Ledger rows c2aea50:ja1front:18421/18425 (TacticalKeyHandler 0x13DE4, cursor.md "Ctrl"): Ctrl down with no
    /// merc selected (0x40, also 48) runs MoveCursorState? 0x1C464 as for a merc (0x1411B has no slot test): the
    /// hand cursor 0x94, cursor tile 0xC41 and AP 0.
    #[test]
    fn ctrl_with_no_merc_selected_shows_the_hand() {
        let (exe, jag, save) = crate::world::tests::files(&[]);
        for sel in [48u8, 0x40] {
            let mut w = World::from_files(&exe, &jag, &save).unwrap();
            w.selected = sel;
            let mut p = Play::new(1);
            let mut cursor = crate::cursor::CursorState::default();
            let mut latches = crate::cursor::KeyLatches::default();
            (cursor.value, cursor.tile, cursor.ap) = (0x11, 0x123, 7);
            let mut s = Session::new(&mut w, &mut p.host);
            Play::key_flags_step(&mut s, &mut cursor, &mut latches, 0x104);
            assert_eq!((cursor.value, cursor.mode, cursor.tile, cursor.ap), (0x94, 3, 0xC41, 0), "selected {sel}");
        }
    }

    /// Ledger row c2aea50:ja1front:18937: a click panel request is read only through a host with a PlayHost; a null
    /// host or a host without one gives none.
    #[test]
    fn the_click_panel_request_needs_a_play_host() {
        assert_eq!(super::take_click_panel_now(std::ptr::null()), None);
        let none = crate::host::Ja1Host { ctx: std::ptr::null_mut(), random: super::cb_random, call: super::cb_call, host_ops: 0 };
        assert_eq!(super::take_click_panel_now(&none), None);
        let mut p = Play::new(1);
        p.host.click_panel = Some((2, 3));
        let h = crate::host::Ja1Host { ctx: &mut p.host as *mut PlayHost as *mut std::ffi::c_void, random: super::cb_random,
                                       call: super::cb_call, host_ops: 0 };
        assert_eq!(super::take_click_panel_now(&h), Some((2, 3)));
        assert_eq!(super::take_click_panel_now(&h), None);
    }

    /// Ledger row c2aea50:ja1front:17881: an overview press with no merc record selected (48) reads no record (hp 0)
    /// and does not stop; off the map (no occupant 0x40) the press walks.
    #[test]
    fn an_overview_press_with_no_merc_reads_no_record() {
        let (exe, jag, save) = crate::world::tests::files(&[]);
        let mut w = World::from_files(&exe, &jag, &save).unwrap();
        w.selected = 48;
        let mut p = Play::new(1);
        assert!(Session::new(&mut w, &mut p.host).overview_press_walks(-1));
    }

    /// Ledger row d057aca:ja1front:18198: key X swaps places only for a selected merc (< 8); out of a fight always,
    /// in a fight only after the screen is drawn.
    #[test]
    fn key_x_needs_a_selected_merc() {
        assert!(super::swap_runs(2, 0, 0) && super::swap_runs(2, 1, 1));
        assert!(!super::swap_runs(2, 1, 0));
        assert!(!super::swap_runs(8, 0, 0) && !super::swap_runs(0x40, 1, 1));
    }

    /// Ledger rows d057aca:ja1front:18608 / 0b4d529:ja1front:18875: the window runs 20 hover steps for each tick of a
    /// pass (#29 H: ~20 loop passes per 10 ms tick in the original), at least one; the script runs one.
    #[test]
    fn the_hover_steps_follow_the_ticks() {
        assert_eq!(super::hover_steps(20, 3), 60);
        assert_eq!(super::hover_steps(20, 1), 20);
        assert_eq!(super::hover_steps(20, 0), 1);
        assert_eq!(super::hover_steps(0, 5), 1);
    }

    /// Ledger row 0b4d529:ja1front:18881 (smoke detector_far): the walk-end detector click opens the confirm wait for
    /// the selected merc with no wait open; not for another merc, not over an open wait.
    #[test]
    fn the_detector_wait_needs_the_selected_merc_and_no_wait() {
        assert!(super::detector_wait_opens(1, 1, false));
        assert!(!super::detector_wait_opens(2, 1, false));
        assert!(!super::detector_wait_opens(1, 1, true));
    }

    /// Ledger row 0b4d529:ja1front:18739 (section 8 "Click"): before a press on the map the mouse comes to the
    /// press tile (one hover step there).
    #[test]
    fn a_map_press_moves_the_mouse_to_its_tile_first() {
        let (exe, jag, save) = crate::world::tests::files(&[]);
        let mut w = World::from_files(&exe, &jag, &save).unwrap();
        w.opt_onscreen = 1;
        let mut p = Play::new(1);
        p.gv_t3 = Some(Box::new([0u8; 256]));
        p.press_walks(&mut w, 0x123, false, false);
        assert_eq!(p.mouse, Some(0x123));
    }

    /// Ledger row ja1front:17530 (cursor.md "Clicks", right step 3.4): a posture confirm with no hold check after it
    /// keeps its own result (the merc text and the wait).
    #[test]
    fn a_posture_confirm_without_a_hold_keeps_its_result() {
        let (exe, jag, save) = crate::world::tests::files(&[]);
        let mut w = World::from_files(&exe, &jag, &save).unwrap();
        let mut p = Play::new(1);
        let (mut cursor, mut hover) = (crate::cursor::CursorState::default(), crate::cursor::HoverState::default());
        let o = crate::cursor::ClickOut { status: Some("Crouch".into()), traverse: Some(3), ..Default::default() };
        let mut lines = Vec::new();
        let mut s = Session::new(&mut w, &mut p.host);
        let r = Play::posture_end(&mut s, &mut cursor, &mut hover, &mut lines, false, &[0u8; 256], o.clone());
        assert_eq!(r, o);
    }

    /// Ledger row ja1front:17534 (panel.md "The small panel in the full view"): op 196's Show sets the small panel
    /// only in view mode 3; in another mode, or with no op 196, the panel stays.
    #[test]
    fn the_small_panel_shows_only_after_op_196_in_mode_3() {
        let (exe, jag, save) = crate::world::tests::files(&[]);
        for (op196, mode, shows) in [(true, 3u8, true), (true, 0, false), (false, 3, false)] {
            let mut w = World::from_files(&exe, &jag, &save).unwrap();
            let mut p = Play::new(1);
            (w.opt_onscreen, w.small_panel, w.strip_flag) = (mode, 0, 0);
            p.host.select.panel_timer = 3000;
            p.host.select.panel = op196.then_some((1, 0));
            p.small_panel(&mut w, 0, false);
            assert_eq!(w.strip_flag == 1, shows, "op 196 {op196} mode {mode}");
            assert!(p.host.select.panel.is_none());
        }
    }
}
