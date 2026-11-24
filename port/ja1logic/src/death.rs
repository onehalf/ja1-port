//! Shooting batch 4: death (docs/spec/shooting.md "Batch 4"; C interface in port_api.md): Kill (library op 78),
//! Merc dies (library op 82), Forget soldier, Forget tiles and Enemy died. A child module of `ai`: every op goes
//! through `Ai::call` (the host_ops rule); Forget soldier is a plain procedure here, never a host call. World data
//! is read again after every host call.
use super::damage::{get8, put16, put8, KNOWLEDGE_LEN, LAST_SEEN_E_LEN, LAST_SEEN_M_LEN, REMEMBER_E_LEN, REMEMBER_M_LEN};
use super::search::s16;
use super::{op, run, Ai, Ja1Host, Ja1World, NONE, NO_SLOT, SLOTS, TILES};

const NONE_I: i32 = NONE as i32;
const SECTOR_STATE_LEN: usize = 80 * 9;
const SECTORS_LEN: usize = 60 * 0xB1;

impl Ai {
    fn ss_get(&self, i: i64) -> u8 {
        get8(unsafe { (*self.w).sector_state }, SECTOR_STATE_LEN, i)
    }
    fn ss_dec(&self, i: i64) -> u8 {
        let n = self.ss_get(i).wrapping_sub(1);
        put8(unsafe { (*self.w).sector_state }, SECTOR_STATE_LEN, i, n);
        n
    }
    fn selected(&self) -> u8 {
        unsafe { (*self.w).selected }
    }
    /// Drop entry E_i: the 12 bytes of entry i = 0.
    fn drop_entry(&self, i: usize) {
        for b in 0..12 {
            self.wr8(12 * i + b, 0);
        }
    }
    /// Put item (sector_start.md batch 3) of entry i of this record.
    fn put_entry(&self, tile: i32, i: usize, note: u8) {
        // The entry is read in place: Put item copies its 12 bytes before Drop entry clears them.
        let p = unsafe { (*self.w).soldiers.add(self.s * 0x200 + 12 * i) };
        crate::sector::put_item_at(self.w, self.h, tile, p, 1, 0, note, 0);
    }

    // ---- Kill (op 78) ----
    pub(super) fn kill(&self, quiet: i32) {
        let slot = self.slot();
        // 1.
        if self.selected() as i32 == slot {
            self.call(op::CURSOR_BACK, &[]);
        }
        // 2.
        if self.s8(0x186) == -1 {
            return;
        }
        // 3.
        self.wr16(0x126, self.rd16(0x116));
        self.wr16(0x128, self.rd16(0x118));
        if self.selected() as i32 == slot {
            unsafe { (*self.w).view_hold = 0 };
        }
        // 4.
        self.wr8(0x185, 0);
        self.wr8(0x184, 0);
        self.wr8(0x183, 0);
        self.wr8(0x1A5, 0);
        self.wr16(0x10C, 10000);
        // 5.
        if quiet == 0 {
            self.call(op::STOP_ANIM, &[slot, 1]);
            self.rest(0, 0);
            self.call(op::END_TASK, &[slot, 1]);
        } else {
            self.wr8(0x1B9, 0);
        }
        // 6.
        for o in [0x17F, 0x181, 0x189, 0x1D5] {
            self.wr8(o, 0);
        }
        // 7.
        if self.selected() as i32 == slot && quiet == 0 {
            self.call(op::CLICK_PANEL, &[slot, 0]);
        }
        // 8.
        self.forget_soldier();
        // 9.
        let c = self.w_cur_sector();
        let sc = c as i8 as i32;
        let row = 9 * c as i64;
        if slot < 8 {
            self.wr32(0x1DC, 0);
            self.wr8(0x189, 0);
            if quiet == 0 {
                self.call(op::MOVE_SOUND, &[slot, 1]);
            }
            self.ss_dec(row + 2);
            self.call(op::REPLAY_EVENT, &[sc, self.s8(0x17A), self.s8(0x179)]);
            let t = s16(self.rd16(0x114) as i32);
            for i in 0..12 {
                if quiet == 0 {
                    self.put_entry(t, i, 0);
                    self.drop_entry(i);
                } else if !crate::sector::listed(self.w, self.h, self.rd8(12 * i)) {
                    self.drop_entry(i);
                }
            }
            let sel = self.selected();
            if quiet == 0 && (sel as i32 == slot || sel >= 8 && slot == unsafe { (*self.w).merc_to_show } as i32) {
                self.call(op::ITEM_PANEL, &[slot, 1, 1]);
            }
            unsafe {
                (*self.w).team_on_duty = (*self.w).team_on_duty.wrapping_sub(1);
                if self.rd8(0x179) >= 0x3C {
                    (*self.w).speaker = NO_SLOT;
                    (*self.w).dead_npc = (*self.w).dead_npc.wrapping_add(1);
                } else {
                    (*self.w).dead_mercs = (*self.w).dead_mercs.wrapping_add(1);
                }
            }
        } else if slot <= 15 {
            self.ss_dec(row + 4);
            unsafe { (*self.w).dead_npc = (*self.w).dead_npc.wrapping_add(1) };
            self.call(op::REPLAY_EVENT, &[sc, sc, 0x52]);
        } else if slot <= 0x27 {
            self.ss_dec(row + 3);
            unsafe { (*self.w).dead_allies = (*self.w).dead_allies.wrapping_add(1) };
            self.call(op::REPLAY_EVENT, &[sc, sc, 0x53]);
        } else {
            if unsafe { (*self.w).mission_slot } as i32 == slot {
                unsafe { (*self.w).mission_slot = NO_SLOT };
                if self.w_mission_step() > 0 && self.w_mission_target() as i16 as i32 != NONE_I {
                    self.call(op::MISSION_CANCEL, &[]);
                }
            }
            let t = s16(self.rd16(0x114) as i32);
            let secs = unsafe { (*self.w).sectors };
            let si = SECTORS_LEN_ROW * c as i64 + 0x9E;
            for i in 7..12 {
                if i == 9 && self.rd8(0x6C) == get8(secs, SECTORS_LEN, si) {
                    put8(secs, SECTORS_LEN, si, 0);
                }
                self.put_entry(t, i, 1);
                self.drop_entry(i);
            }
            // Enemy died.
            for k in 0..8 {
                if let Some(m) = Ai::new(self.w, self.h, k) {
                    if m.rd8(0x182) != 0 && m.s8(0x183) >= 15 && m.cur_anim() >= 4 && m.rd8(0x138) != 0 {
                        m.wr8(0x9D, 1);
                    }
                }
            }
        }
        // 10.
        self.wr8(0x186, 0xFF);
        // 11.
        if quiet != 0 {
            self.call(op::MERC_DIES, &[slot, 1]);
            return;
        }
        // 12.
        self.wr8(0x195, 0);
        let code = if self.in_water() { 0x40 } else { 0x43 };
        self.call(op::SET_ANIM, &[slot, code, 0, 1]);
        // 13.
        let k = self.rd8(0x1DA);
        if k < 8 {
            if let Some(ka) = Ai::new(self.w, self.h, k as i32) {
                ka.wr8(0x1D9, ka.rd8(0x1D9).wrapping_add(1));
            }
            let pr = s16(self.s8(0x192) * 5);
            self.call(op::STAT_PRACTICE, &[k as i32, 9, pr, 0]);
            unsafe { (*self.w).last_killer = k };
        }
    }

    // ---- Forget soldier ----
    pub(super) fn forget_soldier(&self) {
        let slot = self.s;
        // 1.
        self.wr16(0x108, NONE);
        // 2.
        for k in 0..SLOTS {
            if let Some(r) = Ai::new(self.w, self.h, k as i32) {
                for off in [0x1D8, 0x1F7, 0x1C2] {
                    if r.rd8(off) as usize == slot {
                        r.wr8(off, NO_SLOT);
                    }
                }
            }
        }
        let (lsm, lse, rm, re) = unsafe { ((*self.w).last_seen_m, (*self.w).last_seen_e, (*self.w).remember_m, (*self.w).remember_e) };
        if slot < 0x28 {
            // 3.
            for k in 0x28..0x30usize {
                let kr = Ai::new(self.w, self.h, k as i32).unwrap_or_else(|| unreachable!());
                if kr.s8(0x13A + slot) == 1 {
                    kr.wr8(0x1D5, kr.rd8(0x1D5).wrapping_sub(1));
                }
                kr.wr8(0x13A + slot, 0);
                put16(lse, LAST_SEEN_E_LEN, 0x50 * (k as i64 - 0x28) + 2 * slot as i64, NONE);
                put8(re, REMEMBER_E_LEN, 0x28 * (k as i64 - 0x28) + slot as i64, 0);
            }
            if slot < 16 {
                self.d_forget_tiles();
            }
        } else {
            // 4.
            for k in 0..16usize {
                let kr = Ai::new(self.w, self.h, k as i32).unwrap_or_else(|| unreachable!());
                if kr.s8(0x112 + slot) == 1 {
                    kr.wr8(0x1D5, kr.rd8(0x1D5).wrapping_sub(1));
                }
                kr.wr8(0x112 + slot, 0);
                put8(rm, REMEMBER_M_LEN, 8 * k as i64 + slot as i64 - 0x28, 0);
                put16(lsm, LAST_SEEN_M_LEN, 16 * k as i64 + 2 * (slot as i64 - 0x28), NONE);
            }
            self.d_forget_tiles();
            unsafe {
                if (*self.w).last_spotter as usize == slot {
                    (*self.w).last_spotter = NO_SLOT;
                }
            }
        }
        // 5.
        put8(unsafe { (*self.w).knowledge }, KNOWLEDGE_LEN, slot as i64, 0);
    }

    /// Forget tiles (this record).
    fn d_forget_tiles(&self) {
        if self.s8(0xBA) == 2 {
            return;
        }
        let s = self.rd8(0x17A) as i64;
        if s < 0x28 {
            let p = unsafe { (*self.w).last_seen_m };
            for j in 0..8 {
                put16(p, LAST_SEEN_M_LEN, 16 * s + 2 * j, NONE);
            }
        } else {
            let p = unsafe { (*self.w).last_seen_e };
            for j in 0..0x28 {
                put16(p, LAST_SEEN_E_LEN, 0x50 * (s - 0x28) + 2 * j, NONE);
            }
        }
    }

    // ---- Merc dies (op 82) ----
    pub(super) fn merc_dies(&self, quiet: i32) {
        let slot = self.slot();
        // 1.
        if self.rd8(0x188) == 1 {
            return;
        }
        // 2.
        self.wr8(0x195, 0);
        self.wr8(0x1AC, 4);
        self.wr8(0x185, 0);
        self.wr8(0x184, 0);
        self.wr8(0x183, 0);
        self.wr8(0x1A5, 0);
        self.wr16(0x10C, 10000);
        if slot < 8 {
            // 3.
            self.call(op::DEATH_SHOW, &[slot, quiet]);
            if self.ss_get(9 * self.w_cur_sector() as i64 + 2) == 0 {
                unsafe { (*self.w).squad_lost = 1 };
                for k in 0..8 {
                    if k == slot {
                        continue;
                    }
                    if self.rd8_of(k as usize, 0x182) != 0 && self.rd8_of(k as usize, 0x138) != 0 {
                        unsafe { (*self.w).squad_lost = 0 };
                        break;
                    }
                }
            }
        } else {
            // 4.
            self.wr8(0x182, 0);
            let c = self.w_cur_sector() as i64;
            if slot >= 0x28 && self.ss_get(9 * c + 1) != 0 {
                // a.
                unsafe { (*self.w).enemy_deaths = (*self.w).enemy_deaths.wrapping_add(1) };
                let n = self.ss_dec(9 * c + 1);
                // b.
                if n == 0 {
                    // Original 0x3A0B0 chooses a living merc and shows its sector victory quote before CombatEnd.
                    self.call(op::SECTOR_WON, &[]);
                } else if self.rd8(0x1DA) < 8 {
                    let ki = self.rd8(0x1DA) as usize;
                    let kg = |off: usize| s16(self.rd16_of(ki, off) as i32);
                    let my = |off: usize| s16(self.rd16(off) as i32);
                    let args = [ki as i32, kg(0x116) + 2, kg(0x118) + 4, my(0x114), my(0x116) + 2, my(0x118) + 4, 1, 0x0E, 0x40];
                    if self.call(op::SIGHT_LINE, &args) != 0 {
                        self.call(op::QUOTE_EVENT, &[ki as i32, 0xFA, 0, NONE_I, 0]);
                        for j in 0..16usize {
                            let g = |off: usize| self.rd8_of(j, off);
                            if g(0x182) != 0 && g(0x138) != 0 && g(0x1D5) == 0 && g(0x1FE) != 0 {
                                self.call(op::IDLE, &[j as i32]);
                            }
                        }
                    }
                }
                // c.
                let c = self.w_cur_sector();
                put8(unsafe { (*self.w).sectors }, SECTORS_LEN, SECTORS_LEN_ROW * c as i64 + 0x6E + slot as i64, 0xFE);
                let sc = c as i8 as i32;
                self.call(op::REPLAY_EVENT, &[sc, sc, 0x54]);
            }
        }
        // 5.
        self.wr8(0x188, 1);
        self.wr8(0x138, 0);
        self.wr8(0x17E, 0);
        let t = s16(self.rd16(0x114) as i32);
        if (0..TILES as i32).contains(&t) {
            let tp = unsafe { (*self.w).tiles };
            let i = 8 * t as i64 + 5;
            let b = get8(tp, TILES * 8, i);
            put8(tp, TILES * 8, i, (b & 0x80) | 0x40);
        }
        // 6.
        self.wr16(0x114, NONE);
        self.wr16(0x118, 0);
        self.wr16(0x124, NONE);
        self.wr16(0x116, 0);
        self.wr16(0x128, 0);
        self.wr16(0x126, 0);
        self.wr8(0x1A8, 0);
        // 7.
        if slot < 8 && unsafe { (*self.w).squad_lost } != 0 && quiet == 0 {
            Ai::deselect(self.w, self.h, 1);
            unsafe { (*self.w).selected = slot as u8 };
            self.call(op::SQUAD_LOST, &[]);
        }
    }
}

const SECTORS_LEN_ROW: i64 = 0xB1;

// ---- entry points (shooting.md batch 4) ----
// SPEC GAP: a slot outside 0..47 (or a null world / host / soldier buffer) does nothing.
#[no_mangle]
pub extern "C" fn ja1_kill(w: *mut Ja1World, h: *const Ja1Host, slot: i32, quiet: i32) {
    run(w, h, slot, (), |a| a.kill(quiet))
}
#[no_mangle]
pub extern "C" fn ja1_merc_dies(w: *mut Ja1World, h: *const Ja1Host, slot: i32, quiet: i32) {
    run(w, h, slot, (), |a| a.merc_dies(quiet))
}
#[no_mangle]
pub extern "C" fn ja1_forget_soldier(w: *mut Ja1World, h: *const Ja1Host, slot: i32) {
    run(w, h, slot, (), |a| a.forget_soldier())
}
