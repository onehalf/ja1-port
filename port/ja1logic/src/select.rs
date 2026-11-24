//! Selecting a merc (docs/spec/select.md): Select merc (library op 120 low_ap_stop), Select soldier, Hide path
//! marks. Path AP with marks lives with Path AP in update.rs. A child module of `ai`: every op goes through
//! `Ai::call` (the host_ops rule). World data is read again after every host call.
use super::search::s16;
use super::{op, run, Ai, Ja1Host, Ja1World, NONE, NO_SLOT, TILES};

const NONE_I: i32 = NONE as i32;
const MARKS_LEN: usize = 256;

impl Ai {
    fn sw(&self) -> &mut Ja1World {
        unsafe { &mut *self.w }
    }
    /// The path preview of a merc with an open walk order: Path AP with marks (mark = extra = m), then the AP check.
    fn sel_preview(&self, t: i32, m: u8) {
        self.path_ap_marks(t, m, m, self.s8(0x1A8) as u8, self.s8(0x177) as u8, self.s8(0x196) as u8, true);
    }
    /// Select merc (op 120): 1 = selected, 0 = not.
    pub(super) fn select_merc(&self, slot: i32, force: u8, quote: u8) -> i32 {
        // 1.
        if self.sw().screen_drawn == 0 {
            return 0;
        }
        // 2.
        if self.s8(0x183) <= 0 {
            return 0;
        }
        // 3.
        let vis = {
            let w = self.sw();
            w.view_hold == 0 && (force != 0 || (slot == w.shown_slot || w.opt_onscreen == 0))
        };
        if vis && (force != 0 || self.on_screen()) {
            // 3.1, 3.2
            let r = self.select_soldier(1, quote);
            self.call(op::CURSOR_STATE, &[slot]);
            // 3.3
            if r == 0 {
                return 0;
            }
            // 3.4
            if self.sw().quote_slot < NO_SLOT {
                self.sw().quote_delay = 0;
                self.merc_quote();
            }
            // 3.5
            if self.sw().sees_slot < NO_SLOT && self.rd8(0x17A) < 8 {
                self.call(op::SEES_MSG, &[slot]);
            }
            // 3.6
            if self.sw().path_preview != 0 {
                self.call(op::UI_REFRESH, &[1]);
            }
            // 3.7
            let t = s16(self.rd16(0x120) as i32);
            if t != NONE_I && self.rd16(0x120) != self.rd16(0x114) && self.rd8(0x16D) != 0 {
                let m = if self.sw().preview_slot as i32 == slot || self.sw().spot_stop != 0 {
                    self.sw().spot_stop = 0;
                    1
                } else {
                    0
                };
                self.sel_preview(t, m);
                if self.s8(0x1A5) < self.sw().walk_ap {
                    let pp = self.sw().path_preview as u8 as i8 as i32;
                    self.call(op::UI_REFRESH, &[pp]);
                    self.wr8(0x16D, 0);
                }
            }
            // 3.8
            return r;
        }
        // 4.
        if self.sw().view_hold != 0 {
            return 0;
        }
        // 5.
        self.call(op::PORTRAIT_REFRESH, &[slot, 1]);
        self.call(op::FOCUS, &[slot, 0]);
        // 6.
        let t = s16(self.rd16(0x120) as i32);
        if self.w_combat() && self.rd8(0x16D) == 0 && t != NONE_I && self.rd16(0x120) != self.rd16(0x114) {
            self.wr8(0x16D, 1);
            self.sel_preview(t, 1);
            if self.s8(0x1A5) < self.sw().walk_ap {
                let pp = self.sw().path_preview as u8 as i8 as i32;
                self.call(op::UI_REFRESH, &[pp]);
                self.sw().merc_shown = NO_SLOT;
                self.wr8(0x16D, 0);
            }
        }
        // 7.
        self.call(op::MOUSE_WAIT, &[]);
        0
    }
    /// NextMerc? 0x1315C step 3 on the selected merc's record: the view focus; in a fight with an open walk order
    /// (tile120 not none and not the own tile) the path preview, and when the AP do not reach, the marks go again.
    pub(super) fn next_merc_focus(&self, sel: i32) {
        self.call(op::FOCUS, &[sel, 0]);
        let t = s16(self.rd16(0x120) as i32);
        if self.w_combat() && t != NONE_I && self.rd16(0x120) != self.rd16(0x114) {
            self.wr8(0x16D, 1);
            self.sel_preview(t, 1);
            if self.s8(0x1A5) < self.sw().walk_ap {
                let pp = self.sw().path_preview as u8 as i8 as i32;
                self.call(op::UI_REFRESH, &[pp]);
                self.sw().merc_shown = NO_SLOT;
                self.wr8(0x16D, 0);
            }
        }
    }
    /// DeselectSoldier 0x17F10: the selected merc (slot < 8) gets mark 0x6A and frame picture 5, then selected = 0x40,
    /// then his portrait redraw. With no merc selected it does nothing.
    pub(super) fn deselect(w: *mut Ja1World, h: *const Ja1Host, show: i32) {
        let o = unsafe { (*w).selected };
        if o >= 8 {
            return;
        }
        if let Some(r) = Ai::new(w, h, o as i32) {
            r.wr8(0x1BD, 0x6A);
            r.call(op::SELECT_LIGHT, &[o as i32, 5]);
            r.sw().selected = 0x40;
            r.call(op::PORTRAIT_REFRESH, &[o as i32, show]);
        }
    }
    /// RedrawTacticalPanel? 0x12CBC, the first enemy action after a pause (AiThink 0x6D230, 0x6D54D; NextTurn? 0x12D9A): with
    /// show_ai set the pause turns into a shown enemies' turn. show_ai and turn_timer clear, the kept merc loses the frame
    /// light, all buttons shade, and each merc in the sector and in play with his turn not done goes to duty 8.
    /// MercsTurn? 0x12DD4 then takes the "new turn" road, which undoes duty 8.
    pub(super) fn enemy_panel(w: *mut Ja1World, h: *const Ja1Host) {
        unsafe {
            if (*w).show_ai == 0 {
                return;
            }
            (*w).show_ai = 0;
            (*w).turn_timer = 0;
        }
        let Some(a) = Ai::new(w, h, 0) else { return };
        let km = unsafe { (*w).kept_merc };
        if km < 8 {
            if let Some(r) = Ai::new(w, h, km as i32) {
                r.wr8(0x1BD, 0x6A);
            }
            a.call(op::SELECT_LIGHT, &[km as i32, 5]);
        }
        a.call(op::PANEL_REDRAW, &[0x18]);
        for k in 0..8 {
            let Some(r) = Ai::new(w, h, k) else { return };
            if r.rd8(0x182) != 0 && r.rd8(0x138) != 0 && r.rd8(0x1B0) == 0 {
                r.wr8(0x17E, 8);
                r.call(op::PORTRAIT_REFRESH, &[k, 1]);
            }
        }
    }
    /// Select soldier on this record (k = its slot): 1 = selected, 0 = not.
    pub(super) fn select_soldier(&self, show: u8, quote: u8) -> i32 {
        let k = self.s;
        let sh = show as i8 as i32;
        // 1.
        if k > 7 || self.rd8(0x182) == 0 || self.rd8(0x17E) != 0 {
            return 0;
        }
        // 2.
        let o = self.sw().selected;
        if k != o as usize {
            // 2.1
            if o < 8 {
                if let Some(r) = Ai::new(self.w, self.h, o as i32) {
                    r.wr8(0x1BD, 0x6A);
                }
                self.call(op::SELECT_LIGHT, &[o as i32, 5]);
                self.sw().selected = NO_SLOT;
                self.call(op::PORTRAIT_REFRESH, &[o as i32, sh]);
            }
            // 2.2
            self.sw().selected = k as u8;
            self.wr8(0x1BD, 0x6E);
            self.call(op::SELECT_LIGHT, &[k as i32, 0xC]);
            // 2.3
            if self.s8(0x183) >= 15 {
                self.call(op::CURSOR_SELECT, &[]);
                if quote != 0 {
                    self.call(op::EFFECT_SLOT, &[k as i32, 0]);
                }
                // (op 58 runs whatever quote is: ai_diff select_soldier cases 7, 51, 55, quote 0.)
                self.call(op::RESERVE_AP, &[k as i32]);
            } else {
                self.call(op::CURSOR_RESET, &[]);
            }
            // 2.4
            self.call(op::PORTRAIT_REFRESH, &[k as i32, sh]);
        }
        // 3.
        if show != 0 {
            if self.sw().opt_onscreen == 3 {
                let vy = self.sw().view_y;
                let r = self.call(op::PANEL_VIEW, &[vy]) as i32;
                self.sw().view_y = r;
            }
            self.call(op::ITEM_PANEL, &[k as i32, 0, 1]);
        }
        // 4.
        1
    }
}

/// Hide path marks (select.md).
pub(super) fn hide_path_marks(w: *mut Ja1World, force: u8) {
    if w.is_null() {
        return;
    }
    let w = unsafe { &mut *w };
    // 1.
    if w.marks_shown == 0 {
        return;
    }
    // 2.
    if w.path_preview > 0 && force == 0 {
        return;
    }
    // 3.
    w.path_preview = 0;
    w.marks_shown = 0;
    // 4.
    if w.opt_path_marks != 0 {
        let m = w.marks;
        // SPEC GAP: a null marks buffer, or marks_n past the 128 entries, reads tile 0 for the missing entries.
        let entry = |i: i32| -> i32 {
            if m.is_null() || !(0..128).contains(&i) {
                return 0;
            }
            unsafe { s16(m.add(2 * i as usize).read() as i32 | (m.add(2 * i as usize + 1).read() as i32) << 8) }
        };
        for i in 0..w.marks_n.max(0) {
            let t = entry(i);
            if (0..TILES as i32).contains(&t) && !w.tiles.is_null() {
                unsafe { w.tiles.add(t as usize * 8 + 7).write(0) };
            }
        }
        w.marks_n = 0;
        if !m.is_null() {
            unsafe { std::ptr::write_bytes(m, 0, MARKS_LEN) };
        }
    }
}

/// DeselectSoldier 0x17F10 (AL show): the `slot` argument is not read, the routine works on `selected`.
#[no_mangle]
pub extern "C" fn ja1_deselect(w: *mut Ja1World, h: *const Ja1Host, _slot: i32, show: i32) {
    Ai::deselect(w, h, show)
}
#[no_mangle]
pub extern "C" fn ja1_select_merc(w: *mut Ja1World, h: *const Ja1Host, slot: i32, force: i32, quote: i32) -> i32 {
    run(w, h, slot, 0, |a| a.select_merc(slot, force as u8, quote as u8))
}
#[no_mangle]
pub extern "C" fn ja1_select_soldier(w: *mut Ja1World, h: *const Ja1Host, slot: i32, show: i32, quote: i32) -> i32 {
    // SPEC GAP: k = the low byte of slot; a k of 48 or more has no record and gives 0 (k > 7 gives 0 anyway).
    run(w, h, slot & 0xFF, 0, |a| a.select_soldier(show as u8, quote as u8))
}
/// Space on the tactical screen (TacticalKeyHandler case 0x20): NextMerc? 0x1315C from merc `slot` (the caller
/// passes the selected merc's record slot and checks combat = 0 or (screen_drawn != 0 and view_hold = 0), and no
/// cursor item). The next merc after `slot` (0..7, wrapping) that Select merc (k, 1, 1) takes becomes the selected
/// one; then the view focus and, in a fight, the path preview of its open walk order.
#[no_mangle]
pub extern "C" fn ja1_next_merc(w: *mut Ja1World, h: *const Ja1Host, slot: i32) {
    if w.is_null() {
        return;
    }
    // 1.
    let mercs = {
        let ww = unsafe { &*w };
        let s = ww.cur_sector as usize;
        // SPEC GAP: a current sector of 80 and up (or a null table) reads 0 mercs.
        if ww.sector_state.is_null() || s >= super::SECTORS { 0 } else { unsafe { ww.sector_state.add(s * 9 + 2).read() } }
    };
    if unsafe { (*w).selected } >= 8 || mercs == 0 {
        return;
    }
    // 2. (Hide path marks (0) as host op 55, as Select merc calls it)
    run(w, h, slot, (), |a| {
        a.call(op::UI_REFRESH, &[0]);
    });
    let me = slot as u8 as i32;
    let mut k = me;
    for tries in 1.. {
        k = if k + 1 > 7 { 0 } else { k + 1 };
        if tries > 10 || k == me {
            break;
        }
        if run(w, h, k, 0, |a| a.select_merc(k, 1, 1)) != 0 {
            break;
        }
    }
    // 3.
    let sel = unsafe { (*w).selected } as i32;
    run(w, h, sel, (), |a| a.next_merc_focus(sel));
}

/// Enter on the tactical screen (TacticalKeyHandler case 0x0D): NextEnemy 0x3F7C8 on merc `slot` (the caller passes
/// the selected merc's record slot and checks selected < 8, combat != 0 and screen_drawn != 0). From the enemy after
/// focus_last (0x28..0x2F, wrapping), at most 8 tries: the first enemy in play (+0x182, +0x138) that the player sees
/// (+0x9C > 0 or follow_all) and that this merc sees (when sees_slot is this merc: its sight byte +0x112 + k = 1) or
/// any such enemy (sees_slot = 0x40) becomes focus_last = focus_slot and gets the view focus; none: "No enemies
/// visible!" (op 125 text 15).
#[no_mangle]
pub extern "C" fn ja1_next_enemy(w: *mut Ja1World, h: *const Ja1Host, slot: i32) {
    run(w, h, slot, (), |a| {
        let last = a.sw().focus_last;
        let mut k: u8 = if last == 0x2F { 0x28 } else { last.wrapping_add(1) };
        for _ in 0..8 {
            if k >= 0x30 {
                k = 0x28;
            }
            let o = k as usize;
            if a.rd8_of(o, 0x182) != 0
                && a.rd8_of(o, 0x138) != 0
                && (a.rd8_of(o, 0x9C) as i8 > 0 || a.sw().follow_all != 0)
            {
                let sees = a.sw().sees_slot;
                if (a.rd8(0x17A) == sees && a.rd8(0x112 + o) == 1) || (a.rd8(0x17A) != sees && sees == 0x40) {
                    let s = a.rd8_of(o, 0x17A);
                    a.sw().focus_last = s;
                    a.sw().focus_slot = s;
                    a.call(op::FOCUS, &[s as i32, 0]);
                    return;
                }
            }
            k = k.wrapping_add(1);
        }
        a.call(op::MESSAGE, &[15]);
    })
}

/// OnScreen 0x1FC34 of the soldier in `slot`: 1 = its picture is in the view (always 1 when opt_onscreen = 0).
#[no_mangle]
pub extern "C" fn ja1_on_screen(w: *mut Ja1World, h: *const Ja1Host, slot: i32) -> i32 {
    run(w, h, slot, 0, |a| a.on_screen() as i32)
}
#[no_mangle]
pub extern "C" fn ja1_hide_path_marks(w: *mut Ja1World, _h: *const Ja1Host, force: i32) {
    hide_path_marks(w, force as u8)
}

#[cfg(test)]
mod guard_world_tests {
    use super::*;
    use std::os::raw::c_void;

    unsafe extern "C" fn count(ctx: *mut c_void, _op: i32, _a: *const i32, _n: i32, _o: *mut u8, _os: i32) -> u32 {
        *(ctx as *mut u32) += 1;
        0
    }

    /// Port rule (SPEC GAP): a current sector of 80 and up reads 0 mercs, so Next merc does nothing (no host call).
    #[test]
    fn guard_next_merc_sector_range() {
        let mut ss = vec![0u8; 81 * 9];
        ss[79 * 9 + 2] = 1;
        ss[80 * 9 + 2] = 1;
        let mut sol = vec![0u8; 48 * 0x200];
        let mut w: Box<Ja1World> = Box::new(unsafe { std::mem::zeroed() });
        w.sector_state = ss.as_mut_ptr();
        w.soldiers = sol.as_mut_ptr();
        let mut calls = 0u32;
        let h = Ja1Host { ctx: &mut calls as *mut u32 as *mut c_void, random: None, call: Some(count), host_ops: !0 };
        w.cur_sector = 80;
        ja1_next_merc(&mut *w, &h, 0);
        assert_eq!(calls, 0);
        w.cur_sector = 79;
        ja1_next_merc(&mut *w, &h, 0);
        assert_ne!(calls, 0);
    }
}
