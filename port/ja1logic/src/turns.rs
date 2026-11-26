//! Turns (docs/spec/turns.md; C interface in docs/spec/port_api.md "Turn entry points"). A child module of `ai`:
//! the procedures view the records through `Ai` and make every op through `Ai::call` (the one dispatch point that
//! applies the host_ops rule). First merc, Turn sightings, Interrupt queue, Enemies' turn, End turn and Age
//! knowledge are plain library procedures here, never host calls. World data is read again after every host call.
use super::search::s16;
use super::{op, run, Ai, Ja1Host, Ja1World, NONE, NO_SLOT, TILES};

const NONE_I: i32 = NONE as i32;
const ITQ_LEN: usize = 50;
const BLED_LEN: usize = 48;
const FX_LEN: usize = 65;
const CALLS_LEN: usize = 8;
const KNOWLEDGE_LEN: usize = 48;
const REMEMBER_M_LEN: usize = 128;
const REMEMBER_E_LEN: usize = 320;
const GROUND: usize = 200;
const SECTOR_STATE_LEN: usize = 720;
const SECTORS: usize = 60;

/// Byte i of a world buffer of `len` bytes; null or out of range reads as 0.
// SPEC GAP: an index outside a buffer (e.g. itq_len above 50) reads 0 and a write there is dropped.
fn get(p: *const u8, len: usize, i: usize) -> u8 {
    if p.is_null() || i >= len {
        return 0;
    }
    unsafe { p.add(i).read() }
}
fn put(p: *mut u8, len: usize, i: usize, v: u8) {
    if p.is_null() || i >= len {
        return;
    }
    unsafe { p.add(i).write(v) }
}

/// The turn procedures' view of the world: `Ai` on slot 0 for host calls and record reads of any slot.
struct T {
    w: *mut Ja1World,
    h: *const Ja1Host,
    a: Ai,
}

impl T {
    fn new(w: *mut Ja1World, h: *const Ja1Host) -> Option<T> {
        Ai::new(w, h, 0).map(|a| T { w, h, a })
    }
    fn rec(&self, k: usize) -> Option<Ai> {
        Ai::new(self.w, self.h, k as i32)
    }
    fn g8(&self, k: usize, off: usize) -> u8 {
        self.a.rd8_of(k, off)
    }
    fn s8(&self, k: usize, off: usize) -> i32 {
        self.g8(k, off) as i8 as i32
    }
    fn g16(&self, k: usize, off: usize) -> u16 {
        self.a.rd16_of(k, off)
    }
    fn p8(&self, k: usize, off: usize, v: u8) {
        if let Some(r) = self.rec(k) {
            r.wr8(off, v);
        }
    }
    fn p16(&self, k: usize, off: usize, v: u16) {
        if let Some(r) = self.rec(k) {
            r.wr16(off, v);
        }
    }
    fn call(&self, opc: i32, args: &[i32]) -> u32 {
        self.a.call(opc, args)
    }
    fn in_play(&self, k: usize) -> bool {
        self.g8(k, 0x182) != 0 && self.g8(k, 0x138) != 0 && self.s8(k, 0x183) >= 15
    }
    fn w(&self) -> &mut Ja1World {
        unsafe { &mut *self.w }
    }

    // ---- First merc ----
    fn first_merc(&self) -> i32 {
        if crate::fanfix::on(38) {
            // #70 F38: 1.15m FirstMerc? 0x2AC69 + tail 0x8AE73 / 0x8AFAD: health >= 15 first, then >= 1 (signed);
            // duty 8 counts only while +0x194 is 0.
            for min in [15, 1] {
                for k in 0..8 {
                    let duty = self.g8(k, 0x17E);
                    if self.g8(k, 0x182) != 0
                        && (duty == 0 || (duty == 8 && self.g8(k, 0x194) == 0))
                        && self.s8(k, 0x183) as i32 >= min
                    {
                        return k as i32;
                    }
                }
            }
            return NO_SLOT as i32;
        }
        for k in 0..8 {
            if self.g8(k, 0x182) != 0 && matches!(self.g8(k, 0x17E), 0 | 8) && self.g8(k, 0x183) != 0 {
                return k as i32;
            }
        }
        NO_SLOT as i32
    }

    // ---- Turn sightings ----
    fn turn_sightings(&self, both: u8) {
        // 1.
        if both != 0 {
            for k in 0x28..0x30 {
                if self.in_play(k) {
                    self.call(op::ENEMY_TURNED, &[k as i32]);
                }
            }
        }
        // 2.
        for k in 0..16 {
            if self.in_play(k) {
                self.call(op::GUARD_TURNED, &[k as i32]);
                self.call(op::SHARE_SIGHTINGS, &[k as i32, 0]);
            }
        }
        // 3.
        for k in 8..16 {
            if self.in_play(k) {
                for j in 0..8 {
                    if get(self.w().remember_m, REMEMBER_M_LEN, 8 * k + j) == 0xFF {
                        self.call(op::ASK_MERC_INTERRUPT, &[k as i32, 0x28 + j as i32]);
                    }
                }
            }
        }
        // 4.
        if both != 0 {
            for k in 0..8 {
                if self.in_play(0x28 + k) {
                    for j in 0..0x28 {
                        if get(self.w().remember_e, REMEMBER_E_LEN, 0x28 * k + j) == 0xFF {
                            self.call(op::ASK_INTERRUPT, &[0x28 + k as i32, j as i32]);
                        }
                    }
                }
            }
        }
    }

    // ---- Interrupt queue (op 131) ----
    fn itq_pop(&self) -> u8 {
        // 1.
        let mut n = self.w().itq_len;
        let mut i = n as i32;
        let mut r = NO_SLOT;
        // 2.
        while i > 0 {
            let q = self.w().itq;
            r = get(q, ITQ_LEN, (i - 1) as usize);
            if r == NO_SLOT {
                i -= 1;
                if i <= 0 {
                    r = NO_SLOT;
                }
                continue;
            }
            n = n.wrapping_sub(1);
            put(q, ITQ_LEN, n as usize, NO_SLOT);
            let k = r as usize;
            if self.g8(k, 0x182) != 0 && self.s8(k, 0x183) >= 15 && self.g8(k, 0x1B0) == 0 {
                break;
            }
            i -= 1;
            if i == 0 {
                break;
            }
        }
        // 3.
        self.w().itq_len = n;
        r
    }

    // ---- Age knowledge ----
    fn age_knowledge(&self, side: u8) {
        // 1.
        let (lo, hi, c) = if side != 0 { (0x28, 0x30, 0) } else { (0, 0x28, 4) };
        // 2.
        let calls = self.w().calls;
        let t = get(calls, CALLS_LEN, c + 2);
        if t != 0 {
            put(calls, CALLS_LEN, c + 2, t - 1);
            if t == 1 {
                let [l, h] = NONE.to_le_bytes();
                put(calls, CALLS_LEN, c, l);
                put(calls, CALLS_LEN, c + 1, h);
            }
        }
        // 3.
        for k in lo..hi {
            if self.g8(k, 0x182) == 0 || self.g8(k, 0x138) == 0 || self.g8(k, 0x183) == 0 {
                continue;
            }
            let idx = self.g8(k, 0x17A) as usize;
            let kn = self.w().knowledge;
            let mut a = get(kn, KNOWLEDGE_LEN, idx);
            let v = a as i8;
            if v >= 2 {
                let n = v.wrapping_add(1);
                a = if n > 4 { 0 } else { n as u8 };
            } else if v <= -1 {
                let n = v.wrapping_sub(1);
                a = if n < -3 { 0 } else { n as u8 };
            }
            put(kn, KNOWLEDGE_LEN, idx, a);
        }
    }

    // ---- Enemies' turn (op 133) ----
    fn enemies_turn(&self) {
        // 1.
        for i in 0..BLED_LEN {
            put(self.w().bled, BLED_LEN, i, 0);
        }
        self.w().interrupt_turn = 1;
        // 2.
        for k in 0x28..0x30 {
            if self.g8(k, 0x182) != 0 && self.g8(k, 0x183) != 0 {
                self.call(op::START_TURN, &[k as i32]);
                self.p8(k, 0x9C, 0xFF);
            }
        }
        // 3.
        let (ms, al, mt, msl) = {
            let w = self.w();
            (w.mission_step, w.alarm as i8, w.mission_target as i16 as i32, w.mission_slot)
        };
        if ms > 0 && al == 1 && mt != NONE_I && msl == NO_SLOT {
            self.call(op::MISSION_CANCEL, &[]);
        }
        // 4.
        self.turn_sightings(0);
        let sd = self.w().screen_drawn;
        self.age_knowledge(sd);
        // 5.
        self.w().selected = 8;
        self.call(op::TURN_BANNER, &[]);
        // 6.
        for k in 0x28..0x30 {
            let id = self.g8(k, 0x17A);
            // F15 (1.15m EnemiesTurn? 0x124CA + cave 0x18E98): also with see-all 0x93458, and a down enemy
            // (+0x186 > 0) first gets SlideViewToTile? 0x3F578 on its tile +0x114.
            let fan = crate::fanfix::on(15);
            // (the cave adds the two bytes: AL = (+0x9C > 0) + see-all, so 1 + 0xFF counts as not seen)
            let vis = (self.s8(k, 0x9C) > 0) as u8;
            let seen = if fan { vis.wrapping_add(self.w().follow_all) != 0 } else { vis != 0 };
            if get(self.w().bled, BLED_LEN, id as usize) != 0 && seen {
                if fan && self.s8(k, 0x186) > 0 {
                    let t = self.g16(k, 0x114) as i16 as i32;
                    self.call(op::SLIDE_VIEW, &[t]);
                }
                let on = self.rec(k).is_some_and(|r| r.on_screen());
                if on {
                    self.call(op::EFFECT_SLOT, &[id as i32, 3]);
                }
            }
        }
    }

    // ---- End turn ----
    fn end_turn(&self) {
        // 1.
        for f in 0..5usize {
            let fx = self.w().timed_fx;
            let b = 13 * f;
            let e32 = i32::from_le_bytes([get(fx, FX_LEN, b), get(fx, FX_LEN, b + 1), get(fx, FX_LEN, b + 2), get(fx, FX_LEN, b + 3)]);
            if e32 < 0 || (get(fx, FX_LEN, b + 9) as i8) > 0 {
                continue;
            }
            let t = i16::from_le_bytes([get(fx, FX_LEN, b + 6), get(fx, FX_LEN, b + 7)]) as i32;
            for o in 0..4 {
                put(fx, FX_LEN, b + o, 0xFF);
            }
            if crate::fanfix::on(18) {
                // F18 (1.15m EndTurn? 0x1220D): each effect that goes off turns time compression off.
                crate::ai::clock::time_compress(self.w, self.h, 0);
            }
            let kind = get(fx, FX_LEN, b + 8);
            let owner = get(fx, FX_LEN, b + 0xB);
            if t < NONE_I {
                // SPEC GAP: a negative t (off the map) reads tile byte 6 as 0.
                let g = self.a.tb(t, 6);
                if (g as usize) < GROUND {
                    let ty = get(self.w().ground_items, GROUND * 12, 12 * g as usize);
                    let k = crate::sector::blast_kind(ty);
                    if k == kind {
                        self.call(op::EXPLODE, &[owner as i32, t, k as i32, 1]);
                    }
                }
            } else {
                let a = get(fx, FX_LEN, b + 0xA) as i8 as i32;
                let bb = i16::from_le_bytes([get(fx, FX_LEN, b + 4), get(fx, FX_LEN, b + 5)]) as i32;
                let c = get(fx, FX_LEN, b + 0xC) as i8 as i32;
                self.call(op::TIMED_EFFECT, &[kind as i32, a, bb, owner as i32, c]);
            }
        }
        // 2.
        {
            let w = self.w();
            w.shown_slot = NO_SLOT as i32;
            w.spotted = NO_SLOT as i32;
            w.view_tile = NONE_I;
        }
        // 3.
        if self.w().quote_slot < NO_SLOT {
            self.w().quote_delay = 0;
        }
        // 4.
        if self.w().screen_drawn == 0 {
            return;
        }
        // 5.
        self.w().new_round = 0;
        self.call(op::MOUSE_HIDE, &[]);
        self.w().show_ai = 0;
        let tiles = self.w().tiles;
        for t in 0..TILES {
            put(tiles, TILES * 8, 8 * t + 7, 0);
        }
        // 6.
        {
            let w = self.w();
            w.merc_to_show = w.selected;
            w.screen_drawn = 0;
            w.turn_timer = 0;
            w.kept_merc = NO_SLOT;
        }
        Ai::deselect(self.w, self.h, 0);
        self.call(op::PANEL_REDRAW, &[0x18]);
        // 7.
        for k in 0..8 {
            if self.g8(k, 0x182) != 0 && self.g8(k, 0x138) != 0 && self.g8(k, 0x188) == 0 {
                self.p8(k, 0x17E, 8);
                self.p8(k, 0x194, 0);
                self.p8(k, 0x1B0, 1);
                self.call(op::PORTRAIT_REFRESH, &[self.g8(k, 0x17A) as i32, 1]);
            }
        }
        // 8.
        self.call(op::MOUSE_OFF, &[]);
        if self.w().opt_onscreen < 3 {
            self.call(op::MOUSE_MOVE, &[0x50, 8]);
        }
        self.call(op::MOUSE_SAVE, &[]);
        // 9.
        let r = self.itq_pop();
        if r < NO_SLOT {
            self.w().selected = r;
            self.call(op::TURN_BANNER, &[]);
        } else if self.w().interrupt_turn == 0 {
            self.enemies_turn();
        } else {
            self.w().selected = 8;
            self.call(op::TURN_BANNER, &[]);
        }
    }

    // ---- Mercs' turn (op 132) ----
    fn mercs_turn(&self) {
        // 1.
        if self.w().new_round == 0 {
            self.w().turn_timer = 20;
            self.w().turn_timer2 = 0;
        }
        self.w().screen_drawn = 1;
        if self.w().show_ai != 0 {
            self.after_pause();
        } else {
            self.new_mercs_turn();
        }
    }
    /// Mercs' turn step 2: after a pause.
    fn after_pause(&self) {
        // 1.
        self.w().show_ai = 0;
        let km = self.w().kept_merc;
        if km < 8 {
            self.w().selected = km;
        }
        // 2.
        let s = self.w().selected as usize;
        self.p8(s, 0x1A6, self.g8(s, 0x95));
        self.p16(s, 0x134, self.g16(s, 0x96));
        // 3.
        let t = s16(self.g16(s, 0x120) as i32);
        if t == NONE_I || self.g16(s, 0x114) == self.g16(s, 0x120) {
            return;
        }
        // 4. Path AP: the library's procedure with the soldier's flags, or host op 40 (selected, t).
        let lib = unsafe { (*self.h).host_ops } & (1u64 << op::PATH_AP) == 0;
        match self.rec(s) {
            Some(r) if lib => {
                r.path_ap_op(t, self.s8(s, 0x1A8) as u8, self.s8(s, 0x177) as u8, self.s8(s, 0x196) as u8);
            }
            // SPEC GAP: a selected slot of 48 or more names no record; op 40 then goes to the host.
            _ => {
                self.call(op::PATH_AP, &[s as i32, t]);
            }
        }
        // 5.
        if self.s8(s, 0x1A5) < self.w().walk_ap {
            return;
        }
        // 6.
        let (ks, ka) = (self.w().key_shift, self.w().key_alt);
        self.call(op::MOVE_ORDER, &[s as i32, t, 0, ks as i32, self.g8(s, 0x177) as i32, ka as i32, 0, 0]);
    }
    /// Mercs' turn step 3: a new mercs' turn.
    fn new_mercs_turn(&self) {
        // 1.
        self.call(op::PANEL_SLOTS, &[1]);
        // 2.
        for k in 0..8 {
            if self.g8(k, 0x182) != 0 && self.g8(k, 0x17E) == 8 {
                let left = self.g8(k, 0x1B0);
                if left == 0 {
                    self.p8(k, 0x17E, 0);
                }
                if crate::fanfix::on(38) {
                    // #70 F38: 1.15m MercsTurn? 0x12F24 writes +0x194 = +0x1B0 (0 too) on both ways.
                    self.p8(k, 0x194, left);
                } else if left != 0 {
                    self.p8(k, 0x194, 1);
                }
                if self.w().kept_merc != self.g8(k, 0x17A) {
                    self.call(op::PORTRAIT_REFRESH, &[k as i32, 1]);
                }
            }
        }
        // 3.
        self.w().shown_slot = NO_SLOT as i32;
        let km = self.w().kept_merc;
        if km < 8 {
            // #70 F38: 1.15m MercsTurn? 0x12F6B reads the kept merc's record, 1.13b the selected one's.
            let s = if crate::fanfix::on(38) { km } else { self.w().selected } as usize;
            if self.g8(s, 0x182) != 0 && self.s8(s, 0x183) >= 10 {
                self.w().shown_slot = km as i32;
                self.p8(s, 0x1B0, 0);
                if s16(self.g16(s, 0x120) as i32) != NONE_I && self.g16(s, 0x120) != self.g16(s, 0x114) {
                    self.p8(s, 0x16D, 1);
                }
            }
            self.w().kept_merc = NO_SLOT;
        }
        // 4.
        if self.w().shown_slot == NO_SLOT as i32 {
            let m = self.first_merc();
            self.w().shown_slot = m;
            if m >= NO_SLOT as i32 {
                self.end_turn();
                return;
            }
        }
        // 5.
        let low = self.w().shown_slot as u8;
        self.w().focus_slot = low;
        self.call(op::FOCUS, &[low as i32, 0]);
        // 6.
        if self.w().shown_slot >= 8 {
            self.w().shown_slot = self.first_merc();
        }
        // 7.
        self.w().turn_shown = 1;
        self.w().view_hold = 0;
        // 8.
        let ss = self.w().shown_slot;
        // #70 F61 (row 87): 1.15m 0x13033 calls cave 0x8AF04 = CursorItemBack? 0x53660 before SelectMerc? 0x1148C.
        if crate::fanfix::on(61) {
            self.call(op::CURSOR_BACK, &[]);
        }
        if self.call(op::LOW_AP_STOP, &[ss, 1, 1]) == 0 {
            self.w().selected = self.first_merc() as u8;
        }
    }

    /// Byte `off` of "record v" (New turn): records 0..47; for v of 0x30 or more the original reads `tiles` byte
    /// 0x200 v + off − 0x60C0 (turns.md names v = 0x40, +0x183: tile byte 0x20C3).
    // SPEC GAP: the page gives the tiles read only for v = 0x40 (+0x183) and says 0x30..0x3F do not occur; the same
    // formula is used for every v ≥ 0x30 and offset; outside `tiles` it reads 0.
    fn rec_byte(&self, v: usize, off: usize) -> u8 {
        if v < 0x30 {
            return self.g8(v, off);
        }
        let i = 0x200 * v + off;
        if i < 0x60C0 {
            return 0;
        }
        get(self.w().tiles, TILES * 8, i - 0x60C0)
    }
    fn sector_state_at(&self, i: usize) -> u8 {
        get(self.w().sector_state, SECTOR_STATE_LEN, i)
    }
    /// Mouse check (turns.md New turn).
    fn mouse_check(&self) {
        self.call(op::MOUSE_REFRESH, &[]);
        self.call(op::MOUSE_CHECK, &[]);
    }
    fn unlock(&self) {
        self.w().round_lock2 = 0;
        self.w().round_lock = 0;
    }

    // ---- New turn (op 134) ----
    fn new_turn(&self, focus: i32) -> i32 {
        // 1.
        let mut keep = false;
        self.w().view_hold = 0;
        if self.w().combat != 0 {
            self.call(op::MOUSE_REFRESH, &[]);
        }
        // 2.
        if self.w().redraw_pending != 0 {
            keep = true;
            self.w().redraw_pending = 0;
        }
        // 3. (F20: 1.15m drops this call to the ret-only stub 0x6A960 for room; nothing a player sees.)
        if !crate::fanfix::on(20) {
            self.call(op::TURN_UI, &[]);
        }
        // 4.
        if self.w().combat != 0 {
            self.w().round_lock = 1;
            self.w().round_lock2 = 1;
        }
        // 5.
        for i in 0..BLED_LEN {
            put(self.w().bled, BLED_LEN, i, 0);
        }
        // 6.
        let q = self.w().quote_slot;
        if q < NO_SLOT {
            self.w().quote_delay = 0;
            if let Some(r) = self.rec(q as usize) {
                r.merc_quote();
            }
        }
        // 7.
        if self.w().handed_over == 0 {
            // a.
            if self.call(op::GUARDS_ARRIVE, &[5]) != 0 {
                self.unlock();
                return 1;
            }
            // b.
            if self.w().turn_moves != 0 {
                let sel = self.w().selected;
                if sel < 8 {
                    self.w().kept_merc = sel;
                }
                self.call(op::NEXT_TURN, &[1]);
                let w = self.w();
                w.interrupt_turn = 1;
                w.kept_merc = NO_SLOT;
                w.handed_over = 1;
                w.round_lock2 = 0;
                w.round_lock = 0;
                return 0;
            }
        } else {
            self.w().handed_over = 0;
        }
        // F20 (1.15m 0x127E2 -> 0x12BEB): redraw_next is handled here, not at step 24. In combat the kept turn start
        // runs at the end of this New turn; out of combat redraw_pending = 1 and the turn hands over as in step 7b.
        if crate::fanfix::on(20) && self.w().redraw_next != 0 {
            keep = true;
            let pending = self.w().combat as i32 ^ 1;
            let w = self.w();
            w.redraw_next = 0;
            w.redraw_pending = pending;
            if pending != 0 {
                w.interrupt_turn = 1;
                w.kept_merc = NO_SLOT;
                w.handed_over = 1;
                w.round_lock2 = 0;
                w.round_lock = 0;
                return 0;
            }
        }
        // 8.
        {
            let w = self.w();
            w.interrupt_turn = 0;
            w.turn_clock = 20000;
            w.turn_flag = 0;
            w.show_ai = 0;
            w.anim_signal = -1;
            w.spot_stop = 1;
            w.screen_drawn = 1;
        }
        // 9.
        if self.w().combat != 0 {
            for i in 0..8 {
                put(self.w().ap_warned, 8, i, 0);
            }
            {
                let w = self.w();
                w.itq_len = 0;
                w.turn_moves = 0;
                w.interrupt_go = 0;
                w.interrupter = NO_SLOT;
                w.kept_merc = NO_SLOT;
            }
            for i in 0..ITQ_LEN {
                put(self.w().itq, ITQ_LEN, i, NO_SLOT);
            }
            self.call(op::PANEL_SLOTS, &[1]);
        } else {
            for k in 8..16 {
                self.p8(k, 0x197, 0);
                self.call(op::AI_THINK, &[k as i32]);
            }
        }
        // 10.
        self.call(op::SECTOR_TICK, &[]);
        self.call(op::GAS_TICK, &[]);
        // 11.
        if self.sector_state_at(9 * self.w().cur_sector as usize + 2) == 0 {
            self.w().combat = 0;
        }
        // 12.
        for k in 0..0x28 {
            if self.g8(k, 0x182) != 0 && self.g8(k, 0x138) != 0 && self.g8(k, 0x183) != 0 {
                self.call(op::START_TURN, &[k as i32]);
            }
        }
        // 13.
        for k in 0x28..0x30 {
            self.p8(k, 0x198, self.g8(k, 0x9C));
        }
        // 14.
        if self.w().combat != 0 {
            // a.
            let sd = self.w().screen_drawn;
            self.age_knowledge(sd);
            // b.
            if self.w().merc_to_show >= 0x30 {
                self.w().merc_to_show = self.first_merc() as u8;
            }
            // c.
            let v = self.w().merc_to_show;
            self.w().shown_slot = v as i32;
            self.w().show_timer = 5000;
            // d.
            if (self.rec_byte(v as usize, 0x183) as i8) <= 0 {
                let r = self.first_merc();
                self.w().shown_slot = r;
                if r >= 8 {
                    let m = self.w().merc_to_show;
                    self.w().selected = m;
                    self.unlock();
                    self.mouse_check();
                    return 0;
                }
            }
            // e.
            if self.sector_state_at(9 * self.w().cur_sector as usize + 2) != 0 {
                if focus != 0 {
                    self.call(op::VIEW_SAVE, &[]);
                    let low = self.w().shown_slot as u8;
                    self.call(op::FOCUS, &[low as i32, 0]);
                    self.w().spotted = NO_SLOT as i32;
                }
                let ss = self.w().shown_slot;
                // #70 F61 (row 87): 1.15m 0x12A17 calls cave 0x8AF04 = CursorItemBack? first.
                if crate::fanfix::on(61) {
                    self.call(op::CURSOR_BACK, &[]);
                }
                self.call(op::LOW_AP_STOP, &[ss, 0, 0]);
                let sel = self.w().selected;
                if sel < 8 {
                    self.call(op::ITEM_PANEL, &[sel as i32, 0, 1]);
                }
                if focus != 0 {
                    self.call(op::VIEW_RESTORE, &[]);
                }
                self.mouse_check();
            }
        } else if self.w().selected > 8 {
            // 15.
            self.w().selected = 8;
            self.call(op::CURSOR_RESET, &[]);
        }
        // 16.
        self.w().new_round = 1;
        self.w().bomb_count = 0;
        // 17.
        for f in 0..5 {
            let fx = self.w().timed_fx;
            let t = get(fx, FX_LEN, 13 * f + 9);
            if t != 0 {
                put(fx, FX_LEN, 13 * f + 9, t.wrapping_sub(1));
            }
        }
        // 18.
        if self.w().speech_wait != 0 {
            self.w().speech_wait -= 1;
        }
        // 19.
        let x = self.w().speaker;
        // SPEC GAP: a cur_sector of 60 or more reads sector_spoken as 0.
        if x < NO_SLOT
            && get(self.w().sector_spoken, SECTORS, self.w().cur_sector as usize) == 0
            && self.w().speech_wait == 0
            && self.call(op::SPEECH_READY, &[]) != 0
            && !self.a.u_merc_busy()
            && (self.rec_byte(x as usize, 0x183) as i8) >= 15
            && self.rec_byte(x as usize, 0x138) != 0
        {
            let cs = self.w().cur_sector as i8 as i32;
            self.call(op::SECTOR_SPEECH, &[x as i32, cs]);
        }
        // 20.
        // F19 (1.15m 0x12B50): the stages start only after the loop pass that follows Santino's quote (spoke = 2);
        // with spoke = 1 the stage blasts in sector 0 hit cache tiles that Explode dat still skips.
        let spoke = self.w().santino_spoke;
        let spoke_ok = if crate::fanfix::on(19) { spoke == 2 } else { spoke != 0 };
        if self.w().cur_sector == 0 && spoke_ok && self.w().stage < 5 {
            let st = self.w().stage.wrapping_add(1);
            self.w().stage = st;
            if st < 4 {
                self.call(op::STAGE_EFFECTS, &[]);
            }
            if self.w().stage == 5 {
                let i = 9 * self.w().cur_sector as usize + 1;
                let b = self.sector_state_at(i);
                put(self.w().sector_state, SECTOR_STATE_LEN, i, b.wrapping_sub(1));
                self.call(op::STAGE_FIVE, &[]);
            }
        }
        // 21.
        self.unlock();
        // 22.
        for k in 0..0x28 {
            let sl = self.g8(k, 0x17A);
            if get(self.w().bled, BLED_LEN, sl as usize) != 0 {
                self.call(op::EFFECT_SLOT, &[sl as i32, 3]);
            }
        }
        // 23.
        if keep {
            self.call(op::KEPT_TURN_START, &[]);
        }
        // 24. (F20: gone; handled before step 8.)
        if !crate::fanfix::on(20) && self.w().redraw_next != 0 {
            self.w().redraw_next = 0;
            self.w().redraw_pending = 1;
        }
        // 25.
        0
    }

    // ---- The kept turn start (op 171; original 0x31550, NewTurn? after a traverse or arriving guards) ----
    /// The soldiers just changed sector, so all sides look again. The seen count is new here (hp 15 or more, unlike
    /// the frame count in TacticalLoop), and the next tactical pass reads it before it counts again. A won interrupt
    /// hands the turn over.
    fn kept_turn_start(&self) {
        // 1.
        self.turn_sightings(1);
        // 2.
        let mut seen = 0u16;
        for k in 0x28..0x30 {
            if self.g8(k, 0x182) != 0
                && self.g8(k, 0x9C) as i8 > 0
                && self.g8(k, 0x183) as i8 >= 15
                && self.g8(k, 0x138) != 0
            {
                seen += 1;
            }
        }
        self.w().seen_enemies = seen;
        // 3.
        if self.w().turn_moves != 0 {
            let sel = self.w().selected;
            if sel < 8 {
                self.w().kept_merc = sel;
            }
            self.call(op::NEXT_TURN, &[1]);
        }
    }

    // ---- Next turn (op 114; original 0x12D58 calls panel redraw 0x12CBC) ----
    fn next_turn(&self, mode: u8) {
        // #70 F29 (row 18): 1.15m enters at 0x12D55, CursorItemBack? 0x53660 first (also before the busy return).
        if crate::fanfix::on(29) {
            self.call(op::CURSOR_BACK, &[]);
        }
        // 1.
        if self.w().busy > 0 {
            return;
        }
        // 2.
        self.w().turn_moves = 0;
        // 3.
        if self.w().merc_to_show == NO_SLOT {
            let sel = self.w().selected;
            let m = if sel < 8 { sel } else { self.call(op::ENEMY_FOCUS, &[]) as u8 };
            self.w().merc_to_show = m;
        }
        // 4.
        if self.w().screen_drawn != 0 {
            Ai::enemy_panel(self.w, self.h);
            self.w().screen_drawn = 0;
            self.w().show_ai = 1;
        }
        // 5.
        if mode as i8 == 1 {
            let r = self.itq_pop();
            self.w().selected = r;
            self.call(op::TURN_BANNER, &[]);
        } else {
            let r = self.itq_pop();
            self.w().next_selected = r;
        }
    }

    // ---- Merc interrupt turn (op 113) ----
    fn interrupt_turn(&self) {
        // 1.
        if self.w().busy > 0 {
            return;
        }
        // 2.
        {
            let w = self.w();
            w.screen_drawn = 1;
            w.turn_timer = 20;
            w.turn_timer2 = 0;
            w.interrupt_go = 0;
            w.show_ai = 0;
        }
        self.call(op::PANEL_SLOTS, &[1]);
        // 3.
        for k in 0..16 {
            if self.g8(k, 0x182) != 0 && self.g8(k, 0x183) != 0 && self.g8(k, 0x17E) == 8 {
                let left = self.g8(k, 0x1B0);
                if left == 0 {
                    self.p8(k, 0x17E, 0);
                }
                if crate::fanfix::on(29) {
                    // #70 F29: 1.15m 0x130C7/0x130C9 writes +0x194 = +0x1B0 (0 too) on both ways.
                    self.p8(k, 0x194, left);
                } else if left != 0 {
                    self.p8(k, 0x194, 1);
                }
                let id = self.g8(k, 0x17A);
                if self.w().interrupter != id {
                    self.call(op::PORTRAIT_REFRESH, &[id as i32, 1]);
                }
            }
        }
        // 4.
        let v = self.w().interrupter;
        if crate::fanfix::on(29) {
            // #70 F29: 1.15m 0x13103 compares the signed byte; > 7 -> shown slot and interrupter 0x40, return.
            if v as i8 > 7 {
                self.w().shown_slot = 0x40;
                self.w().interrupter = 0x40;
                return;
            }
            self.w().shown_slot = v as i32;
        } else if v > 7 {
            self.w().shown_slot = 0;
            self.w().interrupter = 0;
        } else {
            self.w().shown_slot = v as i32;
        }
        // 5.
        self.w().show_timer = 5000;
        let low = self.w().shown_slot as u8;
        self.call(op::FOCUS, &[low as i32, 0]);
        self.w().view_hold = 0;
        let ss = self.w().shown_slot;
        if crate::fanfix::on(29) {
            // #70 F29: 1.15m 0x13141 turn_shown 0x92DCA = 1, cave 0x8AF04 = CursorItemBack? before 0x1148C.
            self.w().turn_shown = 1;
            self.call(op::CURSOR_BACK, &[]);
        }
        self.call(op::LOW_AP_STOP, &[ss, 1, 1]);
        self.call(op::MOUSE_REFRESH, &[]);
    }
}

/// turns.md Rest on spare AP: the rate of each animation word 0x00..0x3F (rest < 0, tire > 0).
const REST_RATE: [i16; 64] = [
    -200, -200, -200, -200, -200, -200, -200, -200, -200, -200, -200, -100, -100, -200, -100, -50, // 0x00..0x0F
    -200, -100, -100, -100, -100, -50, -100, -100, -100, -100, -50, -50, -100, -100, -50, -50, // 0x10..0x1F
    -100, -100, -50, -50, -100, -100, -100, -100, -200, -100, -100, -200, -100, -50, -100, -200, // 0x20..0x2F
    -100, -50, -50, -200, -200, -200, -200, -200, -200, 25, 50, -100, 50, 100, 100, 100, // 0x30..0x3F
];

// ---- Start of a soldier's turn (op 141), Bleed, Drown at the turn start, Spend AP (op 31), Rest ----
impl Ai {
    /// Start of a soldier's turn (turns.md; library op 141).
    pub(super) fn start_turn(&self) {
        let slot = self.slot();
        // 1.
        match self.s8(0x1C4) {
            1 => self.wr8(0x1C4, 0xFF),
            -1 => self.wr8(0x1C4, 0),
            _ => {}
        }
        // 2.
        self.wr8(0x1E1, 0);
        self.wr8(0x1E2, 0);
        self.wr8(0x1E3, 0);
        self.wr8(0x16D, 0);
        // 3.
        if self.rd8(0x1B9) != 0 {
            self.call(op::AID_TICK, &[slot]);
        }
        // 4.
        self.call(op::TURN_REST, &[slot]);
        self.wr8(0x1DB, 0);
        // 5.
        if self.rd8(0x185) != 0 || self.s8(0x186) == 1 {
            self.bleed();
        }
        // 6.
        if self.rd8(0x183) != 0 && self.rd8(0x18A) == 0 {
            self.turn_drown();
        }
        // 7.
        if self.rd8(0x183) == 0 {
            return;
        }
        // 8.
        self.wr8(0x1F2, 0);
        if unsafe { (*self.w).combat } != 0 {
            // 9 a.
            self.call(op::PERCEIVE, &[slot, 1]);
            // 9 b.
            if s16(self.rd16(0x122) as i32) != NONE_I && self.rd8(0x1C0) != 0 {
                let c = self.rd8(0x1C0).wrapping_sub(1);
                self.wr8(0x1C0, c);
                if c == 0 {
                    self.wr16(0x122, NONE);
                }
            }
            // 9 c.
            let saved = if self.s8(0x1A5) > 5 { 5 } else { self.rd8(0x1A5) };
            let m = self.max_ap(self.s) as u8;
            self.wr8(0x1A5, if m != 0 { m.wrapping_add(saved) } else { 0 });
            if self.s8(0x1A5) > 25 {
                self.wr8(0x1A5, 25);
            }
            // 9 d.
            if self.s < 8 {
                // "Then" = still inside the +0x17E = 8 branch.
                if self.rd8(0x17E) == 8 {
                    self.wr8(0x17E, 0);
                    self.wr8(0x1B0, 0);
                    if s16(self.rd16(0x120) as i32) != NONE_I && self.rd16(0x120) != self.rd16(0x114) {
                        self.wr8(0x16D, 1);
                    }
                    self.call(op::RESERVE_AP, &[slot]);
                    self.call(op::PORTRAIT_REFRESH, &[slot, 1]);
                }
            } else {
                self.wr8(0x1B0, 0);
            }
        } else {
            // 10.
            let m = self.max_ap(self.s) as u8;
            self.wr8(0x1A5, m);
        }
        // 11.
        let st = unsafe { (*self.w).story };
        let story = |k: usize| if st.is_null() { 0 } else { unsafe { *st.add(k) } };
        let (cs, ss, so) = (unsafe { (*self.w).cur_sector }, story(0x17), story(0x16));
        if self.s < 8 && self.s8(0x183) >= 15 && cs as i32 == ss as i8 as i32 && so == 0 && !self.u_merc_busy() {
            self.call(op::REVEAL_VIEW, &[slot, self.rd8(0x1A1) as i32, self.rd8(0x19E) as i32, 0]);
        }
        // 12.
        self.anim_delay();
    }

    /// Bleed (turns.md).
    pub(super) fn bleed(&self) {
        let slot = self.slot();
        // 1.
        if !(self.s8(0x185) > 12 || self.s8(0x186) == 1) {
            return;
        }
        if matches!(self.rd8(0x1B9), 4 | 6) {
            return;
        }
        // 2.
        let t = self.rd16(0x1F0).wrapping_sub(1);
        self.wr16(0x1F0, t);
        if t as i16 > 0 {
            return;
        }
        // 3.
        let lost = self.rd8(0x184).wrapping_sub(self.rd8(0x185)).wrapping_sub(self.rd8(0x183));
        if lost != 0 && self.rd8(0x186) == 0 {
            self.wr8(0x185, self.rd8(0x185).wrapping_add(1));
            self.call(op::WOUND_BAR, &[slot, 1]);
            self.wr8(0x189, 7);
            self.call(op::PORTRAIT, &[slot, 1]);
        } else {
            // 4 a.
            if self.s8(0x186) == 1 {
                if self.s8(0x183) >= 10 && self.rd8(0x187) == 0 {
                    let keep = unsafe { (*self.w).screen_drawn };
                    if unsafe { (*self.w).combat } != 0 {
                        unsafe { (*self.w).screen_drawn = 0 };
                    }
                    self.call(op::MERC_MESSAGE, &[slot, 9, 0x40, 0, 1]);
                    unsafe { (*self.w).screen_drawn = keep };
                    self.wr8(0x187, 1);
                }
                if self.s8(0x184) > 15 {
                    self.wr8(0x184, self.rd8(0x184).wrapping_sub(1));
                    self.wr8(0x185, self.rd8(0x185).wrapping_sub(1));
                }
            }
            // 4 b.
            self.call(op::TAKE_DAMAGE, &[slot, 0x1356, 1, 3, 0x40, NONE_I]);
        }
        // 5.
        let id = self.rd8(0x17A) as usize;
        put(unsafe { (*self.w).bled }, BLED_LEN, id, 1);
        // 6.
        let q = self.cdiv(self.s8(0x183), self.s8(0x1F2) + 10);
        self.wr16(0x1F0, q.wrapping_add(1) as u16);
    }

    /// Drown at the turn start (turns.md).
    pub(super) fn turn_drown(&self) {
        let slot = self.slot();
        // 1.
        if self.cur_anim() < 0x39 {
            return;
        }
        // 2.
        if self.s8(0x9C) > 0 || unsafe { (*self.w).follow_all } != 0 {
            self.call(op::FOCUS, &[slot, 0]);
        }
        // 3.
        if self.rd8(0x1BA) == 0 {
            self.call(op::MERC_MESSAGE, &[slot, 0xA, 0x40, 0, 1]);
            self.wr8(0x1BA, 1);
            self.call(op::STAT_PRACTICE, &[slot, 4, 5, 0]);
            return;
        }
        // 4.
        let mut d = (self.random(10) as u8).wrapping_add(2);
        d = d.wrapping_add(self.random(10) as u8);
        if self.rd8(0x19C) == 5 {
            d = d.wrapping_mul(2);
        }
        self.call(op::TAKE_DAMAGE, &[slot, 0x1356, d as i32, 2, 0x40, NONE_I]);
        if self.rd8(0x183) == 0 {
            self.call(op::KILL, &[slot, 0]);
        }
    }

    /// Rest on spare AP (turns.md; library op 152).
    pub(super) fn turn_rest(&self) {
        // 1.
        let a = self.s8(0x1A5);
        if a <= 5 {
            return;
        }
        let n = a - 5;
        // 2.
        let cur = self.cur_anim();
        let rate = if cur < 0x40 { REST_RATE[cur as usize] as i32 } else { -100 };
        // 3. / 4.
        let f = if rate >= 0 {
            (25 - n) * rate
        } else {
            if self.rd8(0x1DB) != 0 {
                return;
            }
            let t = self.rd16(0x114) as i16;
            let gassed = t >= 0 && self.tile_byte(t as u16, 5) & 0x80 != 0;
            // Fan fix F43 (#70 row 93; 1.15m TurnRest? 0x32D90 -> tail 0x8ADA6): the gas mask needs condition >= 10.
            let mask = self.rd8(0x24) == 0x28 && (!crate::fanfix::on(43) || self.rd8(0x26) as i8 >= 10);
            if gassed && !mask {
                return;
            }
            n * rate
        };
        // 5.
        self.call(op::SPEND_AP, &[self.slot(), 0, f as i16 as i32]);
    }

    /// Spend AP (turns.md; library op 31): amount = low byte, tire = low 16 bits.
    pub(super) fn spend_ap(&self, amount: i32, tire: i32) {
        let slot = self.slot();
        // 1.
        if self.s8(0x183) <= 0 || self.rd8(0x188) != 0 {
            return;
        }
        // 2.
        let mut d = amount as u8;
        if d != 0x63 && d != 0 {
            let selected = unsafe { (*self.w).selected } as i32;
            // a.
            let cur = self.cur_anim();
            let free = if cur >= 256 {
                0
            } else {
                get(unsafe { (*self.w).anim_free }, 256, cur as usize)
            };
            if slot != selected || free != 0 {
                if d as i8 > self.s8(0x1A5) as i8 {
                    d = self.rd8(0x1A5);
                }
            }
            // b.
            if unsafe { (*self.w).combat } != 0 {
                let r = self.rd8(0x1E2);
                if d as i8 <= r as i8 {
                    self.wr8(0x1E2, r.wrapping_sub(d));
                } else {
                    self.wr8(0x1E2, 0);
                    self.wr8(0x1A5, self.rd8(0x1A5).wrapping_sub(d.wrapping_sub(r)));
                }
            }
            // c.
            if self.s8(0x1A5) < 0 && slot == selected {
                self.wr8(0x1A5, 0);
            }
        }
        // 3.
        let mut t = tire & 0xFFFF;
        if t != 0 {
            t = self.fatigue_scale(t);
        }
        // 4.
        if self.rd8(0x181) == 0 {
            self.call(op::MOVE_SOUND, &[slot, 0]);
        } else if self.rd8(0x182) == 0 || self.rd8(0x183) == 0 || self.rd8(0x138) == 0 {
            return;
        } else {
            self.call(op::PANEL_UPDATE, &[slot]);
        }
        // 5.
        self.rest(-(s16(t)), 1);
    }

    /// Fatigue scale (turns.md).
    fn fatigue_scale(&self, amount: i32) -> i32 {
        let wt = unsafe { (*self.w).weather_tire };
        // 1.
        let mut c = (200 - self.s8(0x18A)) & 0xFFFF;
        // 2.
        let h = (self.rd8(0x184).wrapping_sub(self.rd8(0x183)).wrapping_sub(self.rd8(0x185)) / 2) as i32 + self.s8(0x183);
        c = s16(c) + 100 - h;
        // 3.
        if self.rd8(0x1B3) == 3 || self.rd8(0x1B2) >= 0x97 || self.rd8(0x19C) == 7 {
            c = s16(c).wrapping_add(self.cdiv(wt, 5));
        } else {
            c = c.wrapping_add(wt & 0xFFFF) & 0xFFFF;
        }
        // 4.
        if self.s < 0x28 {
            let mt = unsafe { (*self.w).merc_tire } as i32;
            c = c.wrapping_add(mt & 0xFFFF);
        }
        // 5.
        if self.rd8(0x19C) == 5 && matches!(self.cur_anim() as i16, 0x39 | 0x3A | 0x3C) {
            c = c.wrapping_mul(5);
        }
        // 6.
        let c = s16(c);
        let a = s16(amount);
        if a > 0 {
            self.cdiv(a.wrapping_mul(c), 100)
        } else {
            self.cdiv(a.wrapping_mul(100), c)
        }
    }

    /// Rest (turns.md): fatigue +0x10C goes down by amount; redraw = low byte.
    pub(super) fn rest(&self, amount: i32, redraw: i32) {
        let slot = self.slot();
        let os = unsafe { (*self.w).opt_onscreen };
        // 1.
        let f = s16((self.rd16(0x10C) as i32).wrapping_sub(amount));
        self.wr16(0x10C, if f > 0 { f as u16 } else { 0 });
        if self.rd16(0x10C) as i16 > 10000 {
            self.wr16(0x10C, 10000);
        }
        // 2.
        let mut e = (100 - (self.rd16(0x10C) as i16 as i32) / 100) as u8;
        if e as i8 > 100 {
            e = 100;
        }
        if (e as i8) < 0 {
            e = 0;
        }
        // 3.
        if e != self.rd8(0x18A) {
            self.wr8(0x18A, e);
            if self.s8(0x18A) >= 50 {
                self.wr8(0x1BA, 0);
            }
            // The strip slot byte +0x17A decides (0x327EA), not the record number.
            let k = self.rd8(0x17A) as i32;
            if os < 2 && k <= 7 {
                self.call(op::ENERGY_BAR, &[slot, redraw as i8 as i32]);
            }
        } else if redraw as u8 != 0 && os < 2 && self.rd8(0x17A) < 8 {
            // 4. CopySlotToScreen 0x33204 (the strip slot +0x17A)
            self.call(op::MERC_PANEL, &[self.rd8(0x17A) as i32]);
        }
    }
}

// SPEC GAP: on a null world, host or soldier buffer the entry points do nothing (results: 0x40).
// ---- Timed effects and the home-sector stages (turns.md "Start a timed effect", "Home-sector stages") ----
/// Sector 0's stage records: (tile, kind, stage, delay, lowest difficulty).
const STAGE_FX: [(i16, u8, u8, i16, u8); 12] = [
    (0x15B, 3, 0, 0, 0),
    (0x192, 3, 0, 800, 0),
    (0x1C8, 3, 0, 1600, 0),
    (0x1C6, 3, 0, 2400, 0),
    (0x1FC, 3, 0, 3200, 0),
    (0x194, 3, 1, 30000, 1),
    (0x202, 3, 1, 10000, 2),
    (0x274, 3, 1, 2500, 0),
    (0x203, 3, 2, 1000, 2),
    (0x2AA, 3, 2, 20000, 1),
    (0x15D, 3, 2, 8000, 0),
    (0x159, 0xA, 3, 500, 0),
];

impl T {
    fn effect_start(&self, tile: i32, kind: u8, timer: u8, power: i32, owner: u8, flag: u8) -> u32 {
        let fx = self.w().timed_fx;
        for f in 0..5usize {
            let b = 13 * f;
            let e = i32::from_le_bytes([get(fx, FX_LEN, b), get(fx, FX_LEN, b + 1), get(fx, FX_LEN, b + 2), get(fx, FX_LEN, b + 3)]);
            if e != -1 {
                continue;
            }
            for (o, v) in (power as u32).to_le_bytes().into_iter().enumerate() {
                put(fx, FX_LEN, b + o, v);
            }
            put(fx, FX_LEN, b + 4, power as u8);
            put(fx, FX_LEN, b + 5, (power >> 8) as u8);
            put(fx, FX_LEN, b + 6, tile as u8);
            put(fx, FX_LEN, b + 7, (tile >> 8) as u8);
            put(fx, FX_LEN, b + 8, kind);
            put(fx, FX_LEN, b + 9, timer);
            put(fx, FX_LEN, b + 0xA, timer);
            put(fx, FX_LEN, b + 0xB, owner);
            put(fx, FX_LEN, b + 0xC, flag);
            if flag != 0 {
                self.w().fuse_beep_timer = 700;
            }
            return 1;
        }
        0
    }
    fn stage_effects(&self) {
        let st = self.w().stage;
        let d = self.w().difficulty as i8 as i32;
        for &(t, kind, stage, delay, low) in STAGE_FX.iter() {
            if stage == st && low as i32 <= d {
                self.effect_start(t as i32, kind, 0, delay as i32, 0x41, 0);
            }
        }
        if st == 0 {
            self.call(op::LOSS_MSG, &[0, 0xFE, 4]);
        }
    }
    fn stage_five(&self) {
        for k in 0x29..0x30usize {
            if self.g8(k, 0x182) != 0
                && self.g8(k, 0x138) != 0
                && self.s8(k, 0x183) >= 15
                && self.s8(k, 0x9C) <= 0
                && self.g8(k, 0x1C4) == 0
            {
                if let Some(r) = self.rec(k) {
                    r.forget_soldier();
                }
                self.call(op::MERC_DIES, &[k as i32, 0]);
            }
        }
    }
}

/// Op 91 effect for `Ai::dispatch`.
pub(super) fn effect_start(w: *mut Ja1World, h: *const Ja1Host, a: &[i32]) -> u32 {
    let g = |k: usize| a.get(k).copied().unwrap_or(0);
    with(w, h, 0, |t| t.effect_start(g(0), g(1) as u8, g(2) as u8, g(3), g(4) as u8, g(5) as u8))
}
/// Op 169 stage_effects for `Ai::dispatch`.
pub(super) fn stage_effects(w: *mut Ja1World, h: *const Ja1Host) {
    with(w, h, (), |t| t.stage_effects())
}
/// Op 170 stage_five for `Ai::dispatch`.
pub(super) fn stage_five(w: *mut Ja1World, h: *const Ja1Host) {
    with(w, h, (), |t| t.stage_five())
}
#[no_mangle]
pub extern "C" fn ja1_effect_start(
    w: *mut Ja1World,
    h: *const Ja1Host,
    tile: i32,
    kind: i32,
    timer: i32,
    power: i32,
    owner: i32,
    flag: i32,
) -> i32 {
    effect_start(w, h, &[tile, kind, timer, power, owner, flag]) as i32
}
#[no_mangle]
pub extern "C" fn ja1_stage_effects(w: *mut Ja1World, h: *const Ja1Host) {
    stage_effects(w, h)
}
#[no_mangle]
pub extern "C" fn ja1_stage_five(w: *mut Ja1World, h: *const Ja1Host) {
    stage_five(w, h)
}

fn with<R>(w: *mut Ja1World, h: *const Ja1Host, dflt: R, f: impl FnOnce(&T) -> R) -> R {
    match T::new(w, h) {
        Some(t) => std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| f(&t))).unwrap_or(dflt),
        None => dflt,
    }
}

/// Op 131 itq_pop for `Ai::dispatch`.
pub(super) fn itq_pop(w: *mut Ja1World, h: *const Ja1Host) -> u8 {
    T::new(w, h).map_or(NO_SLOT, |t| t.itq_pop())
}
/// Op 171 kept_turn_start for `Ai::dispatch`.
pub(super) fn kept_turn_start(w: *mut Ja1World, h: *const Ja1Host) {
    if let Some(t) = T::new(w, h) {
        t.kept_turn_start()
    }
}
/// Op 134 new_turn for `Ai::dispatch`.
pub(super) fn new_turn(w: *mut Ja1World, h: *const Ja1Host, focus: i32) -> u8 {
    T::new(w, h).map_or(0, |t| t.new_turn(focus) as u8)
}
/// First merc (turns.md) for the other library procedures (soldier_update.md Loop head).
pub(super) fn first_merc(w: *mut Ja1World, h: *const Ja1Host) -> i32 {
    T::new(w, h).map_or(NO_SLOT as i32, |t| t.first_merc())
}
/// Op 114 next_turn for `Ai::dispatch`.
pub(super) fn next_turn(w: *mut Ja1World, h: *const Ja1Host, mode: i32) {
    if let Some(t) = T::new(w, h) {
        t.next_turn(mode as u8);
    }
}
/// Op 113 merc_interrupt for `Ai::dispatch`.
pub(super) fn interrupt_turn(w: *mut Ja1World, h: *const Ja1Host) {
    if let Some(t) = T::new(w, h) {
        t.interrupt_turn();
    }
}
/// Op 132 turn_hand_over for `Ai::dispatch`.
pub(super) fn mercs_turn(w: *mut Ja1World, h: *const Ja1Host) {
    if let Some(t) = T::new(w, h) {
        t.mercs_turn();
    }
}
/// Op 133 round_end for `Ai::dispatch`.
pub(super) fn enemies_turn(w: *mut Ja1World, h: *const Ja1Host) {
    if let Some(t) = T::new(w, h) {
        t.enemies_turn();
    }
}

#[no_mangle]
pub extern "C" fn ja1_end_turn(w: *mut Ja1World, h: *const Ja1Host) {
    with(w, h, (), |t| t.end_turn())
}
#[no_mangle]
pub extern "C" fn ja1_first_merc(w: *mut Ja1World, h: *const Ja1Host) -> i32 {
    with(w, h, NO_SLOT as i32, |t| t.first_merc())
}
#[no_mangle]
pub extern "C" fn ja1_turn_sightings(w: *mut Ja1World, h: *const Ja1Host, both: i32) {
    with(w, h, (), |t| t.turn_sightings(both as u8))
}
#[no_mangle]
pub extern "C" fn ja1_itq_pop(w: *mut Ja1World, h: *const Ja1Host) -> i32 {
    with(w, h, NO_SLOT as i32, |t| t.itq_pop() as i32)
}
#[no_mangle]
pub extern "C" fn ja1_enemies_turn(w: *mut Ja1World, h: *const Ja1Host) {
    with(w, h, (), |t| t.enemies_turn())
}
#[no_mangle]
pub extern "C" fn ja1_mercs_turn(w: *mut Ja1World, h: *const Ja1Host) {
    with(w, h, (), |t| t.mercs_turn())
}
#[no_mangle]
pub extern "C" fn ja1_new_turn(w: *mut Ja1World, h: *const Ja1Host, focus: i32) -> i32 {
    with(w, h, 0, |t| t.new_turn(focus))
}
#[no_mangle]
pub extern "C" fn ja1_kept_turn_start(w: *mut Ja1World, h: *const Ja1Host) {
    with(w, h, (), |t| t.kept_turn_start())
}
#[no_mangle]
pub extern "C" fn ja1_next_turn(w: *mut Ja1World, h: *const Ja1Host, mode: i32) {
    with(w, h, (), |t| t.next_turn(mode as u8))
}
#[no_mangle]
pub extern "C" fn ja1_interrupt_turn(w: *mut Ja1World, h: *const Ja1Host) {
    with(w, h, (), |t| t.interrupt_turn())
}

// ---- Start of a soldier's turn, Spend AP (turns.md; port_api.md "Turn entry points") ----
// SPEC GAP: a slot outside 0..47 (or a null world / host / soldier buffer) does nothing.
#[no_mangle]
pub extern "C" fn ja1_start_turn(w: *mut Ja1World, h: *const Ja1Host, slot: i32) {
    run(w, h, slot, (), |a| a.start_turn())
}
#[no_mangle]
pub extern "C" fn ja1_bleed(w: *mut Ja1World, h: *const Ja1Host, slot: i32) {
    run(w, h, slot, (), |a| a.bleed())
}
#[no_mangle]
pub extern "C" fn ja1_turn_drown(w: *mut Ja1World, h: *const Ja1Host, slot: i32) {
    run(w, h, slot, (), |a| a.turn_drown())
}
#[no_mangle]
pub extern "C" fn ja1_turn_rest(w: *mut Ja1World, h: *const Ja1Host, slot: i32) {
    run(w, h, slot, (), |a| a.turn_rest())
}
#[no_mangle]
pub extern "C" fn ja1_spend_ap(w: *mut Ja1World, h: *const Ja1Host, slot: i32, amount: i32, tire: i32) {
    run(w, h, slot, (), |a| a.spend_ap(amount, tire))
}
#[no_mangle]
pub extern "C" fn ja1_rest(w: *mut Ja1World, h: *const Ja1Host, slot: i32, amount: i32, redraw: i32) {
    run(w, h, slot, (), |a| a.rest(amount, redraw))
}

/// ai.md "Max AP" of record `slot` (cursor.md "Debug keys (level 4)", Alt+L).
#[no_mangle]
pub extern "C" fn ja1_max_ap(w: *mut Ja1World, h: *const Ja1Host, slot: i32) -> i32 {
    run(w, h, slot, 0, |a| a.max_ap(slot as usize))
}

/// cursor.md "Debug keys (level 4)" 2, Alt+L (heal, TacticalKeyHandler 0x13950): +0x183 = +0x184, +0x185 = +0x186 =
/// 0, Rest (amount = the fatigue i16 +0x10C, redraw 1), +0x1A5 = MercMaxAP. The caller redraws the strip portrait
/// (RedrawMercSlot? 0x331A4 (+0x17A, 1)).
#[no_mangle]
pub extern "C" fn ja1_debug_heal(w: *mut Ja1World, h: *const Ja1Host, slot: i32) {
    run(w, h, slot, (), |a| {
        a.wr8(0x183, a.rd8(0x184));
        a.wr8(0x185, 0);
        a.wr8(0x186, 0);
        a.rest(a.rd16(0x10C) as i16 as i32, 1);
        let ap = a.max_ap(slot as usize);
        a.wr8(0x1A5, ap as u8);
    })
}

// ---- Turn start (op 123 turn_banner) and Ally alert (turns.md) ----
impl Ai {
    /// Turn start on s = record selected (self is that record).
    fn turn_start(&self) {
        let slot = self.slot();
        // 1.
        if self.rd8(0x182) == 0 || self.rd8(0x138) == 0 || self.s8(0x183) < 15 {
            self.wr8(0x1B0, 1);
            return;
        }
        // 2.
        if self.rd8(0x1B0) != 0 || self.s8(0xBA) == 2 {
            return;
        }
        // 3.
        let w = self.w;
        let mut interrupt = unsafe { (*w).kept_merc } < 8;
        if unsafe { (*w).itq_len } != 0 {
            let q = get(unsafe { (*w).itq }, ITQ_LEN, 0) as i32;
            if (q >= 0x28 && slot < 0x28) || (q < 0x28 && slot >= 0x28) {
                interrupt = true;
            }
        }
        // 4.
        let ba = self.s8(0xBA);
        let kind = if ba < 3 {
            if interrupt { 3 } else { 1 }
        } else if interrupt {
            if ba == 3 { 2 } else { 4 }
        } else {
            0
        };
        self.call(op::BANNER, &[kind]);
        // 5.
        self.wr8(0x197, 0);
        if slot >= 0x28 {
            self.call(op::ENEMY_TURNED, &[slot]);
        } else if slot < 0x10 {
            self.call(op::GUARD_TURNED, &[slot]);
            self.call(op::SHARE_SIGHTINGS, &[slot, 0]);
        }
        // 6.
        self.update_alert();
        // 7.
        if matches!(self.s8(0xBB), 2 | 3) {
            self.wr8(0x1C1, 1);
        } else {
            self.ally_alert();
        }
        // 8.
        unsafe {
            (*w).fast_timer = 15000;
            (*w).turn_based = 0;
        }
        // 9.
        self.call(op::REVEAL_VIEW, &[slot, self.rd8(0x1A1) as i32, self.rd8(0x19E) as i32, 1]);
    }

    /// Ally alert: the soldier looks at its own side.
    fn ally_alert(&self) {
        let slot = self.slot();
        let ks = if slot < 0x28 { 0..16 } else { 0x28..0x30 };
        for k in ks {
            let o = |off: usize| self.rd8_of(k, off);
            // 1.
            if k as i32 == slot || o(0x182) == 0 || o(0x138) == 0 || o(0x183) == 0 {
                continue;
            }
            // 2.
            let g = |off: usize| s16(self.rd16_of(k, off) as i32);
            if self.call(op::TILE_DIST, &[s16(self.rd16(0x114) as i32), g(0x114)]) as i32 > 14 {
                continue;
            }
            // 3.
            let args = [
                slot,
                s16(self.rd16(0x116) as i32) + 2,
                s16(self.rd16(0x118) as i32) + 4,
                g(0x114),
                g(0x116) + 2,
                g(0x118) + 4,
                1,
                0x0E,
                k as i32,
            ];
            if self.call(op::SIGHT_LINE, &args) == 0 {
                continue;
            }
            // 4.
            let o = |off: usize| self.rd8_of(k, off);
            if o(0xBB) as i8 >= 2 || o(0x1C4) != 0 || (o(0x183) as i8) < 15 {
                self.wr8(0xBB, 2);
                self.orders_bump();
                self.wr8(0x1C1, 1);
                return;
            }
            // 5.
            if o(0xBB) as i8 == 1 && self.s8(0xBB) < 1 {
                self.wr8(0xBB, 1);
                self.wr8(0x1C1, 1);
                self.wr8(0x1C0, 3);
                let t = self.rd16_of(k, 0x114);
                self.wr16(0x122, t);
            }
        }
    }
}

/// Turn start with selected = 0x40 (turns.md "Turn start", the empty interrupt queue): "record 0x40" lies in
/// `tiles`, field f at byte f + 0x1F40. Steps 1 and 2 only (step 2 always ends it: ground bytes are never 0).
fn turn_start_none(w: *mut Ja1World) {
    let t = unsafe { (*w).tiles };
    if t.is_null() {
        return;
    }
    let rd = |f: usize| unsafe { *t.add(f + 0x1F40) };
    if rd(0x182) == 0 || rd(0x138) == 0 || (rd(0x183) as i8) < 15 {
        unsafe { *t.add(0x1B0 + 0x1F40) = 1 };
    }
}

/// Op 123 turn_banner for `Ai::dispatch`.
// A selected slot outside 0..47 other than 0x40 (or a null world / host / soldier buffer) does nothing.
pub(super) fn turn_start(w: *mut Ja1World, h: *const Ja1Host) {
    if w.is_null() {
        return;
    }
    let sel = unsafe { (*w).selected } as i32;
    if sel == 0x40 {
        return turn_start_none(w);
    }
    if let Some(a) = Ai::new(w, h, sel) {
        a.turn_start();
    }
}

#[no_mangle]
pub extern "C" fn ja1_turn_start(w: *mut Ja1World, h: *const Ja1Host) {
    if w.is_null() {
        return;
    }
    let sel = unsafe { (*w).selected } as i32;
    if sel == 0x40 {
        return turn_start_none(w);
    }
    run(w, h, sel, (), |a| a.turn_start())
}
#[no_mangle]
pub extern "C" fn ja1_ally_alert(w: *mut Ja1World, h: *const Ja1Host, slot: i32) {
    run(w, h, slot, (), |a| a.ally_alert())
}

impl T {
    // ---- Swap places (key X) ----
    /// The health test of SwapPlaces 0x40568 on soldier k: hp >= 15, current animation (+0x1E8 queue at +0x1F3)
    /// >= 4, and +0x18A != 0.
    fn swap_fit(&self, k: usize) -> bool {
        // (an index past the queue reads on into the next records, as the original does; SPEC GAP: past the 48th
        // record it reads 0)
        let at = k * 0x200 + 0x1E8 + 2 * self.g8(k, 0x1F3) as usize;
        let q = |o: usize| if o / 0x200 < 48 { self.g8(o / 0x200, o % 0x200) as u16 } else { 0 };
        self.s8(k, 0x183) >= 15 && (q(at) | q(at + 1) << 8) >= 4 && self.g8(k, 0x18A) != 0
    }
    /// MsgNotEnoughAP 0x15960 with BL = 99: false (and the merc text, or +0x1B0 = 1 for a non-merc) when in combat
    /// the soldier's AP (+0x1A5 + +0x1E2) are below `ap`.
    fn swap_ap_ok(&self, k: usize, ap: u8) -> bool {
        if self.w().combat != 0 && self.s8(k, 0x1A5) + self.s8(k, 0x1E2) < ap as i32 {
            if self.g8(k, 0x17A) < 8 {
                self.call(op::MERC_TEXT, &[k as i32, 1, 0x4B0]);
            } else {
                self.p8(k, 0x1B0, 1);
            }
            return false;
        }
        true
    }
    /// StepApCost 0x39B30 of soldier k into tile t.
    fn swap_step_ap(&self, k: usize, t: i32) -> u8 {
        self.a.step_cost(t, self.g8(k, 0x1A8), self.g8(k, 0x177), self.g8(k, 0x196)) as u8
    }
    /// 0x3F8A4 PlaceAt: soldier k stands at view pixel (x, y): its old tile freed, tile = ViewTile(x, y), StopAnim
    /// (k, 1), walk order none, pixel pairs +0x116/+0x118 and +0x126/+0x128 = (x, y), the tile taken, the tile bytes
    /// and their classes re-read, ReplaceSoldier.
    fn place_at(&self, k: usize, x: i32, y: i32) {
        let Some(r) = self.rec(k) else { return };
        r.vacate(r.s16_at(0x114));
        // SPEC GAP: a pixel pair off the map gives a tile off the map; the port's tile reads give 0 there.
        let t = (y / 14) * 56 + x / 16;
        r.wr16(0x114, t as u16);
        if crate::fanfix::on(13) {
            // Fan fix F13 (#70 row 5; changelog 1.15m, found by MitchShudlem, fixed by DarzaR; 1.15m PlaceAt
            // 0x3F8D0..0x3F908): SnapToTile(k, 1) instead of StopAnim(k, 1), and the merc stands on the centre point
            // of its new tile (+0x126/+0x128 copied to +0x116/+0x118), not on the raw pixel pair.
            r.wr16(0x126, super::search::px(t as i32) as u16);
            r.wr16(0x128, super::search::py(t as i32) as u16);
            self.call(op::STOP_ANIM, &[k as i32, 1]);
            r.wr16(0x120, 0xC41);
            r.wr16(0x116, r.rd16(0x126));
            r.wr16(0x118, r.rd16(0x128));
        } else {
            self.call(op::STOP_ANIM, &[k as i32, 1]);
            r.wr16(0x120, 0xC41);
            r.wr16(0x116, x as u16);
            r.wr16(0x118, y as u16);
            r.wr16(0x126, x as u16);
            r.wr16(0x128, y as u16);
        }
        let t = r.s16_at(0x114);
        r.occupy_for(t, r.rd8(0x17A));
        let w = self.w();
        r.wr8(0x1B4, r.tb(t, 1));
        r.wr8(0x1B5, r.u_class(w.t1_class, r.rd8(0x1B4)));
        r.wr8(0x1B2, r.tb(t, 0));
        r.wr8(0x1B3, r.u_class(w.t0_class, r.rd8(0x1B2)));
        r.wr8(0x1B6, r.tb(t, 2));
        r.wr8(0x1B7, r.u_class(w.t2_class, r.rd8(0x1B6)));
        self.call(op::REPLACE, &[k as i32]);
    }
    /// SwapPlaces 0x40568 (key X): merc `me` swaps tiles with the merc or guard on tile `t` next to it.
    fn swap_places(&self, me: usize, t: i32) {
        // 1.
        let mt = self.g16(me, 0x114) as i16 as i32;
        // SPEC GAP: a tile off the map (the cursor tile is always on it) does nothing.
        if mt == t || !(0..TILES as i32).contains(&t) {
            return;
        }
        let o = self.a.tb(t, 5) as usize & 0x7F;
        if o >= 0x10 || self.g8(me, 0x17A) >= 8 {
            return;
        }
        // 2.
        if !self.swap_fit(me) {
            self.call(op::MERC_TEXT, &[me as i32, 11, 0x4B0]);
            return;
        }
        if (self.call(op::TILE_DIST, &[mt, t]) as i32) > 1 {
            self.call(op::MESSAGE, &[16]);
            return;
        }
        if !self.swap_fit(o) {
            self.call(op::MESSAGE, &[17]);
            return;
        }
        // 3.
        let a1 = self.swap_step_ap(me, t);
        if !self.swap_ap_ok(me, a1) {
            return;
        }
        let a2 = self.swap_step_ap(o, mt);
        if self.g8(o, 0x17A) < 8 {
            if !self.swap_ap_ok(o, a2) {
                return;
            }
        } else if self.s8(o, 0x1A5) < a2 as i32 {
            self.call(op::MESSAGE, &[18]);
            return;
        }
        // 4.
        self.call(op::SPEND_AP, &[o as i32, a2 as i8 as i32, 0]);
        self.call(op::SPEND_AP, &[me as i32, a1 as i8 as i32, 0]);
        // 5.
        let (xo, yo) = (self.g16(o, 0x116) as i16 as i32, self.g16(o, 0x118) as i16 as i32);
        self.a.vacate(t);
        self.a.vacate(mt);
        self.p16(o, 0x114, self.g16(me, 0x114));
        self.p16(me, 0x114, t as u16);
        self.place_at(o, self.g16(me, 0x116) as i16 as i32, self.g16(me, 0x118) as i16 as i32);
        self.place_at(me, xo, yo);
        // 6.
        self.turn_sightings(1);
        if self.w().turn_moves != 0 {
            let sel = self.w().selected;
            if sel < 8 {
                self.w().kept_merc = sel;
            }
            self.call(op::NEXT_TURN, &[1]);
        }
    }
}
/// Key X on the tactical screen (TacticalKeyHandler case 0x58): SwapPlaces 0x40568, merc `slot` with the merc or
/// guard on tile `tile` (the tile under the cursor).
#[no_mangle]
pub extern "C" fn ja1_swap_places(w: *mut Ja1World, h: *const Ja1Host, slot: i32, tile: i32) {
    if (0..48).contains(&slot) {
        with(w, h, (), |t| t.swap_places(slot as usize, tile))
    }
}

#[cfg(test)]
mod guard_tests {
    use super::*;

    /// NewTurn step 6 (turns.md "New turn" step 6): quote_delay goes to 0 only when quote_slot < 0x40; slot 0x40
    /// (no quote) keeps its delay. Ledger row c2aea50:ja1logic:8385.
    #[test]
    fn new_turn_no_quote_slot_keeps_delay() {
        let mut sol = vec![0u8; 48 * 0x200];
        let mut w: Box<Ja1World> = Box::new(unsafe { std::mem::zeroed() });
        w.soldiers = sol.as_mut_ptr();
        w.quote_slot = 0x40;
        w.quote_delay = 5;
        let h = Ja1Host { ctx: std::ptr::null_mut(), random: None, call: None, host_ops: 0 };
        ja1_new_turn(&mut *w, &h, 0);
        assert_eq!(w.quote_delay, 5);
        w.quote_slot = 0x3F;
        ja1_new_turn(&mut *w, &h, 0);
        assert_eq!(w.quote_delay, 0);
    }

    /// Op 91 effect start: an entry is free only when its i32 +0 is exactly −1 (turns.md "effect start" step 1,
    /// "other negative values are not free"). Entry 0 holds −65281 (bytes FF 01 FF FF), so the effect goes to
    /// entry 1. Ledger row c2aea50:ja1logic:8765.
    #[test]
    fn effect_start_free_only_minus_one() {
        let mut sol = vec![0u8; 48 * 0x200];
        let mut fx = vec![0u8; FX_LEN];
        fx[0..4].copy_from_slice(&[0xFF, 0x01, 0xFF, 0xFF]);
        fx[13..17].copy_from_slice(&[0xFF; 4]);
        let mut w: Box<Ja1World> = Box::new(unsafe { std::mem::zeroed() });
        w.soldiers = sol.as_mut_ptr();
        w.timed_fx = fx.as_mut_ptr();
        let h = Ja1Host { ctx: std::ptr::null_mut(), random: None, call: None, host_ops: 0 };
        assert_eq!(ja1_effect_start(&mut *w, &h, 0x123, 1, 2, 5, 0x41, 0), 1);
        assert_eq!(&fx[0..4], &[0xFF, 0x01, 0xFF, 0xFF]);
        assert_eq!(&fx[13..17], &[5, 0, 0, 0]);
        assert_eq!(&fx[19..21], &[0x23, 0x01]);
    }

    unsafe extern "C" fn ops(ctx: *mut std::os::raw::c_void, op: i32, _a: *const i32, _n: i32, _o: *mut u8, _s: i32) -> u32 {
        (*(ctx as *mut Vec<i32>)).push(op);
        0
    }

    /// Rest 0x32740 with no energy change: CopySlotToScreen 0x33204 only when g_redrawOff 0x92DDF < 2 (asm 0x3280D
    /// `cmp eax,2; jge 0x32824`) and strip slot +0x17A < 8 (0x3281A). Ledger row ja1logic:8749.
    #[test]
    fn rest_panel_redraw_needs_onscreen_below_2() {
        let mut sol = vec![0u8; 48 * 0x200];
        sol[0x18A] = 100;
        let mut w: Box<Ja1World> = Box::new(unsafe { std::mem::zeroed() });
        w.soldiers = sol.as_mut_ptr();
        let mut log: Vec<i32> = vec![];
        let h = Ja1Host {
            ctx: &mut log as *mut Vec<i32> as *mut std::os::raw::c_void,
            random: None,
            call: Some(ops),
            host_ops: !0,
        };
        let wp: *mut Ja1World = &mut *w;
        for (os, want) in [(1u8, 1usize), (2, 0)] {
            unsafe { (*wp).opt_onscreen = os };
            Ai { w: wp, h: &h, s: 0 }.rest(0, 1);
            assert_eq!(log.iter().filter(|&&o| o == op::MERC_PANEL).count(), want, "opt_onscreen {os}");
            log.clear();
        }
    }

    /// "Record v" of New turn: v of 0x30 and up reads on into the tile map (byte 0x200 v + off - 0x60C0), as in the
    /// original's memory (turns.md: v = 0x40, +0x183 = tile byte 0x20C3).
    #[test]
    fn rec_byte_past_soldiers_reads_tiles() {
        let mut sol = vec![0u8; 48 * 0x200];
        let mut tiles = vec![0u8; TILES * 8];
        tiles[1] = 6;
        tiles[0x20C3] = 9;
        let mut w: Box<Ja1World> = Box::new(unsafe { std::mem::zeroed() });
        w.soldiers = sol.as_mut_ptr();
        w.tiles = tiles.as_mut_ptr();
        let h = Ja1Host { ctx: std::ptr::null_mut::<std::os::raw::c_void>(), random: None, call: None, host_ops: 0 };
        let wp: *mut Ja1World = &mut *w;
        let t = T { w: wp, h: &h, a: Ai { w: wp, h: &h, s: 0 } };
        assert_eq!((t.rec_byte(0x40, 0x183), t.rec_byte(0x30, 0xC1), t.rec_byte(0x30, 0xBF)), (9, 6, 0));
    }

    /// MsgNotEnoughAP 0x15960 (asm 0x159A4 cmp +0x17A, 8 / jge 0x159BA): a soldier whose +0x17A is 8 or more gets
    /// +0x1B0 = 1 instead of the merc text.
    #[test]
    fn swap_ap_ok_non_merc_flag() {
        let mut sol = vec![0u8; 48 * 0x200];
        sol[0x200 + 0x17A] = 8;
        sol[0x400 + 0x17A] = 7;
        let mut w: Box<Ja1World> = Box::new(unsafe { std::mem::zeroed() });
        w.soldiers = sol.as_mut_ptr();
        w.combat = 1;
        let h = Ja1Host { ctx: std::ptr::null_mut::<std::os::raw::c_void>(), random: None, call: None, host_ops: 0 };
        let wp: *mut Ja1World = &mut *w;
        let t = T { w: wp, h: &h, a: Ai { w: wp, h: &h, s: 0 } };
        assert!(!t.swap_ap_ok(1, 5));
        assert!(!t.swap_ap_ok(2, 5));
        assert_eq!((sol[0x200 + 0x1B0], sol[0x400 + 0x1B0]), (1, 0));
    }

    #[test]
    fn guard_get_range() {
        // Port rule (SPEC GAP): an index outside the table or a null table reads 0; the byte past the end is 9.
        let buf = [7u8, 1, 2, 3, 4, 9];
        let p = buf[1..].as_ptr();
        assert_eq!(get(p, 4, 3), 4);
        assert_eq!(get(p, 4, 4), 0);
        assert_eq!(get(std::ptr::null(), 4, 4), 0);
    }

    #[test]
    fn guard_put_range() {
        // Port rule (SPEC GAP): a write outside the table or to a null table is dropped.
        let mut buf = [0u8; 6];
        let p = unsafe { buf.as_mut_ptr().add(1) };
        put(p, 4, 3, 5);
        put(p, 4, 4, 6);
        put(std::ptr::null_mut(), 4, 4, 6);
        assert_eq!(buf, [0, 0, 0, 0, 5, 0]);
    }
}
