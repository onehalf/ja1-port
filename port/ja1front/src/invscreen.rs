//! The inventory screen in play (docs/spec/world_load.md section 8 "Inventory screen", key I): the drawing of
//! inventory.md "Drawing the screen" (invdraw.rs) on the library's world, the host ops of `ja1_inventory_screen`,
//! and the input of each poll (the window or a play script).
use crate::host::{Ja1Host, Ja1World};
use crate::invdraw::{self, InvEnv};
use crate::panel::{self, Painter, PH, PW};
use crate::soldier::REC;
use crate::{common, tileset_data};
use std::collections::VecDeque;

/// What one poll of the screen reads: the mouse (tactical screen pixels), buttons (bit 0 left, bit 1 right),
/// key flags (0x1 Shift, 0x4 Ctrl) and the keys that came since the last poll.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct InvPoll {
    pub x: i32,
    pub y: i32,
    pub buttons: u8,
    pub flags: u16,
    pub keys: Vec<u8>,
}

/// The input side of the screen: shows a frame (320 x 200), then gives the input of the poll; None = stop (the
/// window closes, or a play script has no `inv` line left).
pub trait InvInput {
    fn poll(&mut self, frame: &[u8]) -> Option<InvPoll>;
    /// The pointer of the next frames: true = the arrow (the cursor holds no item), false = the item picture only.
    fn pointer(&mut self, _arrow: bool) {}
    /// ShowMessage? 0x27B98: the pointer is off while the message box shows.
    fn pointer_box(&mut self, _on: bool) {}
    /// The mouse moves to this place (the window warps it; a script's next line gives the place).
    fn warp(&mut self, _at: (i32, i32)) {}
    /// The sound of one poll: one tick of 10 ms (a window passes its real time and sends it to the sound card).
    fn sound(&mut self, s: &mut crate::sound::Sound) {
        s.pass(1);
    }
}

/// A play script's `inv` lines, one per poll.
pub struct ScriptInput {
    pub polls: VecDeque<InvPoll>,
}

impl InvInput for ScriptInput {
    fn poll(&mut self, _frame: &[u8]) -> Option<InvPoll> {
        self.polls.pop_front()
    }
}

/// The screen's front-end state while it is open.
pub struct InvScreen<'a> {
    iso: String,
    exe: String,
    tileset: u8,
    faces: &'a [u8],
    table0: &'a mut [u8; 256],
    blink: &'a mut u8,
    input: &'a mut dyn InvInput,
    /// The inventory picture (the screen as the last drawing left it).
    pub picture: Vec<u8>,
    /// The last frame shown (the picture with the cursor item).
    pub frame: Vec<u8>,
    /// The panel buffer as the last drawing left it (ops 239 and 244 draw into it, then copy a part to the screen).
    panel: Vec<u8>,
    /// The merc shown (record index).
    pub shown: usize,
    keys: VecDeque<u8>,
    mouse: (i32, i32),
    /// The input stopped during an item info box: the next polls stop too, and the frame keeps the box.
    ended: bool,
    /// The Control Panel switches Item Info (6) and Subtitles (5) of the world.
    pub opt_info: bool,
    pub opt_sub: bool,
}

impl<'a> InvScreen<'a> {
    #[allow(clippy::too_many_arguments)]
    pub fn new(iso: &str, exe: &str, tileset: u8, faces: &'a [u8], table0: &'a mut [u8; 256], blink: &'a mut u8,
               input: &'a mut dyn InvInput, shown: usize) -> InvScreen<'a> {
        InvScreen {
            iso: iso.to_string(),
            exe: exe.to_string(),
            tileset,
            faces,
            table0,
            blink,
            input,
            picture: vec![0; (PW * PH) as usize],
            frame: vec![0; (PW * PH) as usize],
            panel: vec![0; (PW * PH) as usize],
            shown,
            keys: VecDeque::new(),
            mouse: (0, 0),
            ended: false,
            opt_info: crate::iteminfo::OPT_INFO,
            opt_sub: crate::iteminfo::OPT_SUB,
        }
    }

    /// Step 1 "Draw" for record `rec`: the inventory picture, item_rects, the armour value (result).
    ///
    /// # Safety
    /// `w`: a valid world whose buffers have their port_api.md sizes.
    pub unsafe fn draw(&mut self, w: *mut Ja1World, h: *const Ja1Host, rec: usize) -> Result<i32, i32> {
        let w = &mut *w;
        let c = common(&self.iso)?;
        let d = panel::panel_data(&self.iso, &self.exe)?;
        let id = invdraw::inv_data(&self.iso, &self.exe)?;
        let t = tileset_data(&self.iso, self.tileset)?;
        let so = std::slice::from_raw_parts_mut(w.soldiers, 48 * REC);
        let mut ap_warned = [0u8; 8];
        if !w.ap_warned.is_null() {
            ap_warned.copy_from_slice(std::slice::from_raw_parts(w.ap_warned, 8));
        }
        let sec = w.cur_sector as usize;
        let enemies = if w.sector_state.is_null() || sec >= 80 { 0 } else { *w.sector_state.add(9 * sec + 1) };
        // Sight answers (world_load.md section 8 step 1): only for the mercs that step 12 of the drawing asks about.
        let rd16 = |b: usize, o: usize| (so[b + o] as u16 | (so[b + o + 1] as u16) << 8) as i16 as i32;
        let sb = rec * REC;
        let (st, sx, sy) = (rd16(sb, 0x114), rd16(sb, 0x116) + 2, rd16(sb, 0x118) + 4);
        let mut sight = [0u8; 8];
        for k in 0..8usize {
            let ob = k * REC;
            let ot = rd16(ob, 0x114);
            let dist = ((st / 56 - ot / 56).abs()).max((st - 56 * (st / 56) - (ot - 56 * (ot / 56))).abs());
            let out = so[ob + 0x182] == 0 || so[ob + 0x17E] != 0 || (so[ob + 0x183] as i8) < 1 || dist > 2;
            if !out && k != rec {
                sight[k] =
                    (ja1_sight_line(w as *mut Ja1World, h, rec as i32, sx, sy, ot, rd16(ob, 0x116) + 2, rd16(ob, 0x118) + 4, 1, 14, 0x40)
                        != 0) as u8;
            }
        }
        let env = InvEnv {
            rec,
            selected: w.selected,
            combat: w.combat,
            opt: w.opt_onscreen,
            speaker: 0xFF,
            hover: 0xFF,
            ap_warned,
            faces: self.faces,
            tileset: &t,
            sector: w.cur_sector,
            enemies,
            blink: *self.blink,
            sight,
            small_panel: 0,
            time_w: w.day_bar,
            tiles: std::slice::from_raw_parts(w.tiles, 3136 * 8),
            ground: std::slice::from_raw_parts(w.ground_items, 200 * 12),
        };
        let mut p = Painter { d: &d, buf: &mut self.panel, table0: self.table0, table29: panel::table29(&c) };
        let out = invdraw::draw_inventory(&mut p, &id, &env, so, &mut self.picture);
        *self.blink = out.blink;
        std::slice::from_raw_parts_mut(w.item_rects as *mut u8, 56).copy_from_slice(&out.item_rects);
        self.shown = rec;
        Ok(out.armour)
    }

    /// Ops 238 draw_slot (s, k), 239 vestmap (s) and 244 inv_moved (s, dark, copy): one part of the picture
    /// (inventory.md "Drawing the screen": item place k onto the screen; the vest map (strategic.md "The vest map");
    /// the strip boxes into the panel buffer, then panel rows 182..199 to the screen when copy ≠ 0).
    unsafe fn part(&mut self, w: *mut Ja1World, op: i32, s: i32, a1: i32, a2: i32) -> Result<(), i32> {
        if !(0..48).contains(&s) {
            return Ok(());
        }
        let w = &mut *w;
        let c = common(&self.iso)?;
        let d = panel::panel_data(&self.iso, &self.exe)?;
        let id = invdraw::inv_data(&self.iso, &self.exe)?;
        let so = std::slice::from_raw_parts_mut(w.soldiers, 48 * REC);
        let r = std::slice::from_raw_parts(w.item_rects, 56);
        let mut rects = [(0i32, 0i32); 14];
        for (k, q) in rects.iter_mut().enumerate() {
            *q = (i16::from_le_bytes([r[4 * k], r[4 * k + 1]]) as i32, i16::from_le_bytes([r[4 * k + 2], r[4 * k + 3]]) as i32);
        }
        let tiles = std::slice::from_raw_parts(w.tiles, 3136 * 8);
        let ground = std::slice::from_raw_parts(w.ground_items, 200 * 12);
        let sb = s as usize * REC;
        let t29 = panel::table29(&c);
        match op {
            238 => {
                if (0..14).contains(&a1) {
                    let mut p = Painter { d: &d, buf: &mut self.picture, table0: self.table0, table29: t29 };
                    invdraw::item_place(&mut p, &id, &rects, tiles, ground, so, sb, a1 as usize);
                }
            }
            239 => {
                let (x7, y7) = rects[7];
                let mut p = Painter { d: &d, buf: &mut self.panel, table0: self.table0, table29: t29 };
                invdraw::load_pcx(&mut p, &id, "VESTMAP.PCC", x7, y7 + 1);
                for j in 7..12 {
                    if !invdraw::pocket_missing(&so[sb..sb + REC], j) {
                        invdraw::item_place(&mut p, &id, &rects, tiles, ground, so, sb, j);
                    }
                }
                // inventory.md "The vest map": on this screen the rectangle ends at (271, 181), not at the assignment
                // screen's (319, 140) (#20 A: pocket 11 was cut off at row 140).
                let x0 = x7.clamp(0, 272) as usize;
                for y in y7.max(0) as usize..=181 {
                    self.picture[y * 320 + x0..y * 320 + 272].copy_from_slice(&self.panel[y * 320 + x0..y * 320 + 272]);
                }
            }
            _ => {
                if so[sb + 0x17A] < 8 && w.opt_onscreen < 3 {
                    let mut p = Painter { d: &d, buf: &mut self.panel, table0: self.table0, table29: t29 };
                    for k in 0..7 {
                        invdraw::strip_box(&mut p, &id, so, sb, k, a1 & 0xFF != 0);
                    }
                    if a2 & 0xFF != 0 {
                        self.picture[182 * 320..].copy_from_slice(&self.panel[182 * 320..]);
                    }
                }
            }
        }
        Ok(())
    }

    /// The frame of op 221: the picture with the cursor item (`held`) at the mouse.
    unsafe fn compose(&mut self, w: *mut Ja1World) -> Result<(), i32> {
        self.frame.copy_from_slice(&self.picture);
        let w = &*w;
        let typ = if w.held.is_null() { 0 } else { *w.held };
        // After a quote the held item has the arrow as its pointer (`speech_arrow`), also on this screen.
        let shown = crate::pointer::held_is_pointer(typ);
        self.input.pointer(!shown);
        if shown {
            let c = common(&self.iso)?;
            let d = panel::panel_data(&self.iso, &self.exe)?;
            let id = invdraw::inv_data(&self.iso, &self.exe)?;
            let pic = std::slice::from_raw_parts(w.item_types, 12 * 136).get(12 * typ as usize + 4).copied().unwrap_or(0);
            let mut p = Painter { d: &d, buf: &mut self.frame, table0: self.table0, table29: panel::table29(&c) };
            crate::pointer::held_item(&mut p, &id, pic, self.mouse.0, self.mouse.1);
        }
        Ok(())
    }

    /// Host op 231 item_info (panel.md "Item info box"): the box on the frame, then the waits of step 7 (both buttons
    /// up, a button down, both up) with that frame shown; the frame without the box after it. Keys during the box do
    /// nothing. SPEC GAP front-end: the mouse does not go back to its kept position (step 7, m ≠ 0).
    unsafe fn item_info(&mut self, w: *mut Ja1World, host: &mut crate::play::PlayHost, a: &[i32]) -> Result<i32, i32> {
        let arg = |i: usize| a.get(i).copied().unwrap_or(0);
        self.compose(w)?;
        let c = common(&self.iso)?;
        let d = panel::panel_data(&self.iso, &self.exe)?;
        let id = invdraw::inv_data(&self.iso, &self.exe)?;
        let i = crate::iteminfo::Info {
            profile: arg(0) as u8, t: arg(1) as u8, count: arg(2) as i8, value: arg(3) as i16, numbers: arg(4) == 1,
            opt_info: self.opt_info, opt_sub: self.opt_sub,
        };
        let mut p = Painter { d: &d, buf: &mut self.frame, table0: self.table0, table29: panel::table29(&c) };
        let shown = crate::iteminfo::draw(&mut p, &id, i);
        if i.t == 0 {
            return Ok(0);
        }
        // panel.md "The item's voice": the name (and the condition, rounds or amount) while the box shows.
        let snd = &mut host.sound;
        let voice = crate::iteminfo::voice(&i, &id.item_types, arg(5) != 0, &|arc, m| snd.inv_has(arc, m));
        let names = snd.item_voice(voice.as_deref().unwrap_or(&[]));
        if host.log {
            eprintln!("item voice {}", if voice.is_none() { "none".to_string() } else { names.join(" ") });
        }
        if shown == crate::iteminfo::Shown::Nothing {
            // Item Info off (not a picture item): no box; the wait ends when the voice is done (never without a
            // name sample) or a button goes down; then every sound stops.
            for down in [false, true] {
                loop {
                    if down && voice.is_some() && !host.sound.speaking() {
                        break;
                    }
                    let Some(q) = self.input.poll(&self.frame) else {
                        self.ended = true;
                        host.sound.stop_all();
                        return Ok(0);
                    };
                    self.input.sound(&mut host.sound);
                    self.mouse = (q.x, q.y);
                    if (q.buttons & 3 != 0) == down {
                        break;
                    }
                }
            }
            host.sound.stop_all();
            return Ok(0);
        }
        // ItemInfoBox 0x260C6, 0x26BB0: the mouse goes into the box, and back at the end when it stayed there.
        let m = arg(6) != 0;
        let text_box = shown != crate::iteminfo::Shown::Picture;
        let moved = if text_box { crate::iteminfo::mouse_open(m, self.mouse) } else { None };
        if let Some(at) = moved {
            eprintln!("item box mouse {} {}", at.0, at.1);
            self.input.warp(at);
            self.mouse = at;
        }
        let after = self.mouse;
        // up, down, up; the frame stays the box when the input stops (a play script's PNG shows it)
        let t0 = host.sound.ticks();
        for down in [false, true, false] {
            loop {
                let Some(q) = self.input.poll(&self.frame) else {
                    self.ended = true;
                    return Ok(shown.result());
                };
                self.input.sound(&mut host.sound);
                self.mouse = (q.x, q.y);
                let wd = &mut *w;
                (wd.mouse_x, wd.mouse_y, wd.mouse_buttons, wd.key_flags) = (q.x, q.y, 0, q.flags);
                if (q.buttons & 3 != 0) == down {
                    break;
                }
            }
            // the button went down: the voice stops (every sound); stderr: the sound time of the wait
            if down {
                if let Some(at) = crate::iteminfo::mouse_close(m && text_box, after, self.mouse) {
                    eprintln!("item box mouse back {} {}", at.0, at.1);
                    self.input.warp(at);
                    self.mouse = at;
                }
                eprintln!("item voice ticks {}", host.sound.ticks() - t0);
                host.sound.stop_all();
            }
        }
        self.compose(w)?;
        Ok(shown.result())
    }

    /// Host op 237 inv_extra (sound, text) (inventory.md "inv_extra", port_api.md op 237): the sound, with Subtitles
    /// on the text in the message box for 1000 ms (or a key / a new click), then a wait while the sound plays until a
    /// button is down. stderr: `inv extra MEMBER TEXT box TICKS`.
    unsafe fn extra(&mut self, w: *mut Ja1World, host: &mut crate::play::PlayHost, snd: i32, txt: i32) -> Result<i32, i32> {
        let (member, text) = extra_names(snd, txt);
        if self.ended {
            return Ok(0);
        }
        self.compose(w)?;
        let start = (*w).mouse_buttons;
        let subtitles = self.opt_sub;
        let mut io = ExtraIo { s: self, sound: &mut host.sound, playing: false, boxed: None, t0: 0, shown: -1, box_ran: false, tail_t0: None };
        let live = crate::invextra::run(&mut io, member, text, subtitles, start)?;
        let (boxed, t0, shown, box_ran, tail_t0) = (io.boxed.take(), io.t0, io.shown, io.box_ran, io.tail_t0);
        if let Some(t) = tail_t0 {
            eprintln!("inv extra tail {}", host.sound.ticks() - t);
        }
        // ShowMessage? 0x27CCF: the cursor tile goes to 0xC41 at the end of the box.
        host.marker_clear |= box_ran;
        let box_ticks = if boxed.is_some() { (host.sound.ticks() - t0) as i64 } else { shown };
        eprintln!("inv extra {} {} box {}", if member.is_empty() { "-" } else { member }, if text.is_empty() { "-" } else { text }, box_ticks);
        if !live {
            // the input stopped: in the box, the frame keeps the box (a play script's PNG shows it)
            if let Some(b) = boxed {
                self.frame = b;
            }
            self.ended = true;
        }
        Ok(0)
    }

    /// The host op `op` of the screen, or None when the op is not one of the screen's.
    ///
    /// # Safety
    /// `w`: the world of the running library call; `h`: its host; `a`: the op's arguments.
    pub unsafe fn op(&mut self, w: *mut Ja1World, h: *const Ja1Host, op: i32, a: &[i32]) -> Option<u32> {
        let arg = |i: usize| a.get(i).copied().unwrap_or(0);
        Some(match op {
            221 => {
                if self.ended || self.compose(w).is_err() {
                    return Some(1);
                }
                let Some(p) = self.input.poll(&self.frame) else { return Some(1) };
                let wd = &mut *w;
                wd.mouse_x = p.x;
                wd.mouse_y = p.y;
                wd.mouse_buttons = p.buttons;
                wd.key_flags = p.flags;
                self.mouse = (p.x, p.y);
                self.keys.extend(p.keys);
                0
            }
            251 => {
                // MergeItemsPrompt 0x4AA26 from UseInventoryItem?: QuestionBox? over the picture; no window = no.
                let types = std::slice::from_raw_parts((*w).item_types, 12 * 136);
                let items = crate::question::merge_items(types, arg(0) as u8, arg(1) as u8);
                self.input.pointer(true);
                crate::question::merge_wait(&self.picture, items, false).unwrap_or(false) as u32
            }
            // SetMouseShape?(g_cursorItem) in UseInventoryItem?: also a move that keeps the same item type ends the arrow of a quote
            // (live, #31: a gun swap with a gun, and a full stack that leaves the item as it was).
            225 | 226 => {
                crate::pointer::shape_from_held();
                return None;
            }
            222 => (!self.keys.is_empty()) as u32,
            223 => self.keys.pop_front().unwrap_or(0) as u32,
            231 => self.item_info(w, &mut *((*h).ctx as *mut crate::play::PlayHost), a).unwrap_or(0) as u32,
            237 => self.extra(w, &mut *((*h).ctx as *mut crate::play::PlayHost), arg(0), arg(1)).unwrap_or(0) as u32,
            232 => {
                let rec = (arg(0) as usize).min(47);
                self.draw(w, h, rec).unwrap_or(0) as u32
            }
            238 | 239 | 244 => {
                let _ = self.part(w, op, arg(0), arg(1), arg(2));
                0
            }
            241 => {
                ja1_rest(w, h, arg(0), arg(1), arg(2));
                0
            }
            242 => ja1_select_soldier(w, h, arg(0), arg(1), arg(2)) as u32,
            248 => {
                let g = arg(2) & 0xFF;
                let wd = &*w;
                (g < 200 && !wd.ground_items.is_null() && *wd.ground_items.add(12 * g as usize + 2) as i8 >= 0) as u32
            }
            249 => {
                ja1_pick_up(w, h, arg(0), arg(1), 0);
                0
            }
            250 => {
                let mut item = [0u8; 12];
                for k in 0..3 {
                    item[4 * k..4 * k + 4].copy_from_slice(&arg(1 + k).to_le_bytes());
                }
                ja1_put_item(w, h, arg(0), item.as_ptr(), arg(4), arg(5), arg(6), arg(7));
                0
            }
            _ => return None,
        })
    }
}

/// The host of invextra::run on the inventory screen.
struct ExtraIo<'s, 'a, 'b> {
    s: &'s mut InvScreen<'a>,
    sound: &'b mut crate::sound::Sound,
    playing: bool,
    /// The frame with the box while the box shows.
    boxed: Option<Vec<u8>>,
    t0: u64,
    shown: i64,
    box_ran: bool,
    /// The sound ticks at the end of the box.
    tail_t0: Option<u64>,
}

impl crate::invextra::Host for ExtraIo<'_, '_, '_> {
    fn start(&mut self, member: &str, _rate: i32, _volume: i32) {
        // Sound::inv_extra plays at these numbers
        self.playing = self.sound.inv_extra(member);
    }
    fn show_box(&mut self, text: &str) -> Result<(), i32> {
        self.sound.speech_stall();
        // ShowMessage? 0x27BBE: the pointer goes, and a held item is the pointer: the box lies on the picture.
        self.s.input.pointer_box(true);
        let mut boxed = self.s.picture.clone();
        crate::msgbox::draw_tactical(&self.s.iso, &self.s.exe, self.s.tileset, text.as_bytes(), &mut boxed)?;
        self.boxed = Some(boxed);
        self.t0 = self.sound.ticks();
        Ok(())
    }
    fn box_end(&mut self) {
        self.sound.speech_resume();
        self.tail_t0 = Some(self.sound.ticks());
        self.boxed = None;
        self.s.input.pointer_box(false);
        self.box_ran = true;
        self.shown = (self.sound.ticks() - self.t0) as i64;
    }
    fn poll(&mut self, mode: i32) -> Option<crate::invextra::Poll> {
        let q = match &self.boxed {
            Some(b) => self.s.input.poll(b),
            None => self.s.input.poll(&self.s.frame),
        }?;
        // the box wait: the time of the poll passes here; the sound wait: refill below
        if mode == 0 {
            self.s.input.sound(self.sound);
        }
        self.s.mouse = (q.x, q.y);
        Some(crate::invextra::Poll { buttons: q.buttons, keys: q.keys })
    }
    fn elapsed(&mut self) -> i32 {
        box_ms(self.sound.ticks(), self.t0)
    }
    fn speaking(&mut self) -> bool {
        self.playing && self.sound.speaking()
    }
    fn refill(&mut self) {
        self.s.input.sound(self.sound);
    }
    fn queue(&mut self, key: u8) {
        self.s.keys.push_back(key);
    }
}

/// Host op 237 inv_extra (sound, text) (port_api.md): the member of `INV\EXTRA` and the text ("" = none).
pub fn extra_names(snd: i32, txt: i32) -> (&'static str, &'static str) {
    let member = match snd {
        1 => "EMPTY.8",
        2 => "HMM.8",
        3 => "HUH.8",
        4 => "NOROOM.8",
        _ => "",
    };
    let text = match txt {
        1 => "Empty",
        2 => "Drank...",
        3 => "OK",
        4 => "No Room",
        _ => "",
    };
    (member, text)
}

/// Step 3 "After the screen" (the wrapper 0x54D50): +0x1BB and +0x130 of the 8 mercs = 0, then reserve_ap for the
/// merc that opened the screen (ESI), also when the player switched to another merc inside the screen; the merc
/// shown last keeps its old reserved points (issue #2). The redraw after it sets opt_reload_msg (TacticalRedraw).
///
/// # Safety
/// As `InvScreen::op`.
pub unsafe fn after_screen(w: *mut Ja1World, h: *const Ja1Host, opener: usize) {
    let so = std::slice::from_raw_parts_mut((*w).soldiers, 48 * REC);
    for k in 0..8 {
        so[k * REC + 0x1BB] = 0;
        so[k * REC + 0x130] = 0;
    }
    ja1_reserve_ap(w, h, so[opener * REC + 0x17A] as i32);
    (*w).opt_reload_msg = 1;
}

/// panel.md "The small panel in the full view" step 5: after the inventory screen in mode 3 the panel ends.
pub fn wrapper_end(view_mode: u8, panel_ends: &mut u8) {
    if view_mode == 3 {
        *panel_ends = 1;
    }
}

/// What FUN_00054d50 0x54D50 (`screen_ran`) and then CursorItemToSoldier? 0x54EC8 (`picked`) do after the screen with an
/// item left on the cursor: the mouse limit under the panel and the mouse in row 191 (0xBF). The port keeps the limit in
/// the flag `cursor_drop`; only 0x54EC8 sets that flag. Result: the new flag and the position, if the mouse moves.
pub fn cursor_after(held: u8, drop: u8, mouse_x: i32, screen_ran: bool, picked: bool) -> (u8, Option<(i32, i32)>) {
    if held == 0 {
        return (drop, None);
    }
    let late = picked && drop == 0;
    (if late { 1 } else { drop }, (screen_ran || late).then_some((mouse_x, 191)))
}

/// Key I's rule (section 8): selected < 8, +0x138 ≠ 0, combat = 0 or screen_drawn ≠ 0, hp ≥ 15, +0x188 ≠ 1.
pub fn can_open(soldiers: &[u8], selected: u8, combat: u8, screen_drawn: u8) -> bool {
    if selected >= 8 {
        return false;
    }
    let s = &soldiers[selected as usize * REC..(selected as usize + 1) * REC];
    s[0x138] != 0 && (combat == 0 || screen_drawn != 0) && s[0x183] as i8 >= 15 && s[0x188] != 1
}

extern "C" {
    fn ja1_sight_line(w: *mut Ja1World, h: *const Ja1Host, slot: i32, x: i32, y: i32, tile: i32, tx: i32, ty: i32, flag: i32,
                      range: i32, seen: i32) -> i32;
    fn ja1_rest(w: *mut Ja1World, h: *const Ja1Host, slot: i32, amount: i32, redraw: i32);
    fn ja1_select_soldier(w: *mut Ja1World, h: *const Ja1Host, slot: i32, show: i32, quote: i32) -> i32;
    fn ja1_pick_up(w: *mut Ja1World, h: *const Ja1Host, slot: i32, tile: i32, cursor: i32);
    fn ja1_put_item(w: *mut Ja1World, h: *const Ja1Host, tile: i32, item: *const u8, b6: i32, exact: i32, note: i32,
                    near: i32);
    fn ja1_reserve_ap(w: *mut Ja1World, h: *const Ja1Host, slot: i32);
}

/// TimerElapsedMs 0x47040 for the remark box: sound ticks since the box showed, 10 ms each.
fn box_ms(now: u64, t0: u64) -> i32 {
    ((now - t0) * 10) as i32
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Ledger row d057aca:ja1front:11953: the remark box time counts up from the box's own start, 10 ms per tick.
    #[test]
    fn the_remark_box_time_counts_from_its_start() {
        assert_eq!(box_ms(1000, 1000), 0);
        assert_eq!(box_ms(1250, 1000), 2500);
    }

    /// The remark box host: each poll of the box wait and each refill pass one sound tick of 10 ms (the box time
    /// counts these ticks), and a key read in the wait goes back to the screen's key queue.
    #[test]
    fn extra_box_host_ticks_and_keys() {
        use crate::invextra::Host;
        let mut input = ScriptInput { polls: VecDeque::from(vec![InvPoll { x: 1, y: 2, buttons: 0, flags: 0, keys: vec![] }]) };
        let (mut t0, mut blink) = ([0u8; 256], 0u8);
        let mut s = InvScreen::new("", "", 0, &[], &mut t0, &mut blink, &mut input, 0);
        let mut snd = crate::sound::Sound::with_files("", "");
        let mut io = ExtraIo { s: &mut s, sound: &mut snd, playing: false, boxed: None, t0: 0, shown: -1, box_ran: false, tail_t0: None };
        let t = io.sound.ticks();
        io.refill();
        assert_eq!(io.sound.ticks(), t + 1);
        assert!(io.poll(0).is_some());
        assert_eq!(io.sound.ticks(), t + 2);
        io.queue(0x1C);
        assert_eq!(io.s.keys, VecDeque::from(vec![0x1C]));
    }

    #[test]
    fn can_open_rule() {
        let mut s = vec![0u8; 48 * REC];
        s[0x138] = 1;
        s[0x183] = 15;
        assert!(can_open(&s, 0, 0, 0));
        assert!(!can_open(&s, 8, 0, 0));
        assert!(!can_open(&s, 0, 1, 0)); // combat, screen not drawn
        assert!(can_open(&s, 0, 1, 1));
        s[0x183] = 14;
        assert!(!can_open(&s, 0, 0, 0));
        s[0x183] = 15;
        s[0x188] = 1;
        assert!(!can_open(&s, 0, 0, 0));
        s[0x188] = 0;
        s[0x138] = 0;
        assert!(!can_open(&s, 0, 0, 0));
    }
}
