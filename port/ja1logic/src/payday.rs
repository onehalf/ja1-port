//! Not enough funds (docs/spec/strategic.md "Not enough funds"; C interface in docs/spec/port_api.md "Not enough
//! funds entry point"): `ja1_payday_dialog`. A child module of `ai`; every host op goes through `Ai::call`.
//! Merc leaves and Close up the team are in `pay`, Pay the day is `ledger::apply_day_ledger`. World data is read
//! again after every host call; the only values kept across calls are the dialog's `blink` and `first`.
use super::damage::{get16, get8, put16, put8};
use super::ledger::{apply_day_ledger, BALANCE};
use super::pay::{merc_leaves, team_compact};
use super::{Ai, Ja1Host, Ja1World, REC};

const SOLDIERS_LEN: usize = 48 * REC;
const PROFILE: i64 = 0x5A;
const PROFILES_LEN: usize = 64 * 0x5A;
const ACCOUNTS_LEN: usize = 0xA8;
const TERMINATIONS: i64 = 0x9C;
const NON_PAYMENTS: i64 = 0xA1;

const SALARY2: usize = 0x112;
const PROFILE_ID: usize = 0x179;
const IN_CUR_SECTOR: usize = 0x182;
const HP: usize = 0x183;

const INV_POLL: i32 = 221;
const MOUSE_OFF: i32 = 145;
const MOUSE_SHOW: i32 = 224;
const CURSOR_PIC2: i32 = 226;
const TIP_ITEM: i32 = 233;
const TIP_MERC: i32 = 234;
const QUOTE_END: i32 = 246;
const ISLAND_MAP: i32 = 268;
const RUNS_OFF_END: i32 = 269;
const PD_MESSAGE: i32 = 270;
const PD_TIMER_START: i32 = 271;
const PD_ELAPSED: i32 = 272;
const PD_CLEAR: i32 = 273;
const PD_BLINK: i32 = 274;
const PD_ASK: i32 = 275;
const PD_QUOTE: i32 = 276;
const PD_BALLOON: i32 = 277;
const PD_END: i32 = 278;
const PD_FACE_FRAME: i32 = 279;
const PD_PANEL_SHOW: i32 = 280;

struct Stop;
type R<T> = Result<T, Stop>;

struct Pd {
    w: *mut Ja1World,
    h: *const Ja1Host,
    a: Ai,
}

impl Pd {
    fn call(&self, opc: i32, args: &[i32]) -> u32 {
        self.a.call(opc, args)
    }
    fn sb(&self, k: i32, off: usize) -> u8 {
        get8(unsafe { (*self.w).soldiers }, SOLDIERS_LEN, k as i64 * REC as i64 + off as i64)
    }
    fn sb_put(&self, k: i32, off: usize, v: u8) {
        put8(unsafe { (*self.w).soldiers }, SOLDIERS_LEN, k as i64 * REC as i64 + off as i64, v)
    }
    fn balance(&self) -> i32 {
        let p = unsafe { (*self.w).accounts };
        (get16(p, ACCOUNTS_LEN, BALANCE) as u32 | (get16(p, ACCOUNTS_LEN, BALANCE + 2) as u32) << 16) as i32
    }
    fn acc_inc(&self, off: i64) {
        let p = unsafe { (*self.w).accounts };
        put8(p, ACCOUNTS_LEN, off, get8(p, ACCOUNTS_LEN, off).wrapping_add(1));
    }
    fn mouse_in(&self, x0: i32, y0: i32, x1: i32, y1: i32) -> bool {
        let (x, y) = unsafe { ((*self.w).mouse_x, (*self.w).mouse_y) };
        x0 <= x && x <= x1 && y0 <= y && y <= y1
    }
    fn mouse_y(&self) -> i32 {
        unsafe { (*self.w).mouse_y }
    }
    fn tip_item(&self) -> i32 {
        unsafe { (*self.w).tip_item as i32 }
    }
    fn opt_step_text(&self) -> bool {
        unsafe { (*self.w).opt_step_text != 0 }
    }

    /// Team size: slots in use from slot 0 up to the first with inCurSector = 0 (at most 8).
    fn team_size(&self) -> i32 {
        (0..8).take_while(|&k| self.sb(k, IN_CUR_SECTOR) != 0).count() as i32
    }
    /// First guide: first slot k of 0..7 with inCurSector, hp and profileId 60..=64; 64 = none.
    fn first_guide(&self) -> i32 {
        (0..8)
            .find(|&k| self.sb(k, IN_CUR_SECTOR) != 0 && self.sb(k, HP) != 0 && (60..=64).contains(&self.sb(k, PROFILE_ID)))
            .unwrap_or(64)
    }

    fn poll(&self) -> R<()> {
        if self.call(INV_POLL, &[]) == 1 {
            return Err(Stop);
        }
        Ok(())
    }

    fn dialog(&self) -> R<i32> {
        let mut blink = 0i32;
        let mut first = true;
        loop {
            // 1. stop
            let size = self.team_size();
            if self.balance() >= 0 || size == 0 || (size == 1 && self.first_guide() == 0) {
                break;
            }
            // 2.
            if first {
                self.call(PD_MESSAGE, &[0]);
                self.call(PD_MESSAGE, &[1]);
                first = false;
                self.call(PD_TIMER_START, &[]);
            }
            // 3.
            self.call(PD_CLEAR, &[]);
            self.call(CURSOR_PIC2, &[0]);
            // 4. the click loop
            let mut dismissed = false;
            while !dismissed {
                self.poll()?;
                // b. tooltips
                let mut came = false;
                for k in 0..8 {
                    let x0 = 12 + 37 * k;
                    if self.mouse_in(x0, 20, x0 + 30, 46) && self.sb(k, IN_CUR_SECTOR) != 0 {
                        self.call(TIP_MERC, &[k + 1]);
                        came = true;
                    }
                }
                if !came {
                    self.call(TIP_ITEM, &[self.tip_item()]);
                }
                // c. the blink
                if self.call(PD_ELAPSED, &[]) as i32 > 200 {
                    if self.mouse_y() > 145 {
                        self.call(MOUSE_OFF, &[]);
                    }
                    blink = 1 - blink;
                    self.call(PD_BLINK, &[blink]);
                    self.call(PD_TIMER_START, &[]);
                    if self.mouse_y() > 145 {
                        self.call(MOUSE_SHOW, &[]);
                    }
                }
                // d.
                if unsafe { (*self.w).mouse_buttons } & 1 == 0 {
                    continue;
                }
                // e. the faces
                for k in 0..8 {
                    if self.sb(k, IN_CUR_SECTOR) == 0 {
                        break;
                    }
                    let x0 = 13 + 37 * k;
                    if !self.mouse_in(x0, 20, x0 + 27, 44) {
                        continue;
                    }
                    if self.sb(k, PROFILE_ID) >= 60 {
                        self.call(MOUSE_OFF, &[]);
                        self.call(PD_MESSAGE, &[2]);
                        self.call(MOUSE_SHOW, &[]);
                        continue;
                    }
                    let family = (self.sb(k, HP) as i8 <= 0) as i32;
                    if self.call(PD_ASK, &[k, family]) as u8 == 0 {
                        self.call(QUOTE_END, &[0x25]);
                        self.call(MOUSE_SHOW, &[]);
                        continue;
                    }
                    self.dismiss(k, x0);
                    dismissed = true;
                    break;
                }
                // f. the funds box
                if self.mouse_in(279, 166, 304, 181) {
                    self.call(PD_MESSAGE, &[0]);
                }
            }
        }
        // the end
        self.call(PD_END, &[]);
        if self.balance() >= 0 {
            for k in 0..8 {
                if self.sb(k, IN_CUR_SECTOR) == 0 {
                    break;
                }
                if self.sb(k, PROFILE_ID) < 60 {
                    self.call(PD_FACE_FRAME, &[k]);
                }
            }
            self.call(PD_PANEL_SHOW, &[]);
        }
        Ok(0)
    }

    fn dismiss(&self, k: i32, x0: i32) {
        let p = self.sb(k, PROFILE_ID);
        let alive = self.sb(k, HP) as i8 > 0;
        // Fan fix F63 (#70 row 65): 1.15m PayDayDialog? 0x453C1..0x45456 swaps the two cases: the unpaid live merc
        // leaves a debt (profile +0x3E = salary, terminations + 1); a dead merc's family gets CorpseGrudge? 0x46548
        // (as a refused corpse export) and Merc leaves runs with status 0.
        let fan = crate::fanfix::on(63);
        if alive {
            unsafe { (*self.w).voice_test = 1 };
            self.call(PD_QUOTE, &[p as i32]);
            if !self.opt_step_text() {
                self.call(QUOTE_END, &[0x25]);
                self.call(MOUSE_SHOW, &[]);
            }
            self.call(PD_BALLOON, &[k, x0]);
        }
        if alive == fan {
            let s2 = get16(unsafe { (*self.w).soldiers }, SOLDIERS_LEN, k as i64 * REC as i64 + SALARY2 as i64);
            put16(unsafe { (*self.w).profiles }, PROFILES_LEN, p as i64 * PROFILE + 0x3E, s2);
            self.acc_inc(TERMINATIONS);
        }
        let status = if fan && !alive {
            super::corpse::corpse_grudge(self.w, self.h, k);
            0
        } else {
            1
        };
        merc_leaves(self.w, self.h, k, status);
        self.sb_put(k, IN_CUR_SECTOR, 0);
        self.acc_inc(NON_PAYMENTS);
        self.sb_put(k, PROFILE_ID, 99);
        if self.opt_step_text() {
            self.call(QUOTE_END, &[0x25]);
        }
        self.call(MOUSE_SHOW, &[]);
        self.call(TIP_ITEM, &[self.tip_item()]);
        team_compact(self.w, self.h, 0);
        apply_day_ledger(self.w);
        self.call(ISLAND_MAP, &[]);
        self.call(RUNS_OFF_END, &[]);
    }
}

pub(super) fn payday_dialog(w: *mut Ja1World, h: *const Ja1Host) -> i32 {
    // SPEC GAP: a null world, host or soldier buffer does nothing (0).
    let Some(a) = Ai::new(w, h, 0) else { return 0 };
    let pd = Pd { w, h, a };
    match pd.dialog() {
        Ok(v) => v,
        Err(Stop) => 1,
    }
}

#[no_mangle]
pub extern "C" fn ja1_payday_dialog(w: *mut Ja1World, h: *const Ja1Host) -> i32 {
    std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| payday_dialog(w, h))).unwrap_or(0)
}
