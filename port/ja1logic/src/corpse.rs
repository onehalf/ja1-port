//! Corpse export (docs/spec/strategic.md "Corpse export"; C interface in docs/spec/port_api.md "Corpse export entry
//! point"): `ja1_corpse_export`. A child module of `ai`; host ops 292..294 and the host random go through `Ai`.
//! Merc leaves, Close up the team and Store an item are those of `pay`.
use super::damage::{get8, put8};
use super::ledger::{add, rd, BALANCE, DAY_NET};
use super::pay::{merc_leaves, stash_copy, team_compact};
use super::{op, Ai, Ja1Host, Ja1World, REC};

const SOLDIERS_LEN: usize = 48 * REC;
const PROFILES_LEN: usize = 64 * 0x5A;
const ITEM: usize = 12;
const NICKNAME: usize = 0x16E;
const PROFILE_ID: usize = 0x179;
const SLOT: usize = 0x17A;
const IN_CUR_SECTOR: usize = 0x182;
const HP: usize = 0x183;
/// Guard for the random redraw loop (a host that never returns a free index).
const DRAW_CAP: u32 = 1_000_000;

struct Ctx {
    w: *mut Ja1World,
    h: *const Ja1Host,
    ai: Ai,
}

impl Ctx {
    fn s(&self, k: i32, off: usize) -> u8 {
        get8(unsafe { (*self.w).soldiers }, SOLDIERS_LEN, k as i64 * REC as i64 + off as i64)
    }
    fn s_put(&self, k: i32, off: usize, v: u8) {
        put8(unsafe { (*self.w).soldiers }, SOLDIERS_LEN, k as i64 * REC as i64 + off as i64, v)
    }
    fn prof(&self, p: i32, off: usize) -> u8 {
        get8(unsafe { (*self.w).profiles }, PROFILES_LEN, p as i64 * 0x5A + off as i64)
    }
    fn prof_put(&self, p: i32, off: usize, v: u8) {
        put8(unsafe { (*self.w).profiles }, PROFILES_LEN, p as i64 * 0x5A + off as i64, v)
    }
    /// Dead = signed hp < 1.
    fn dead(&self, k: i32) -> bool {
        (self.s(k, HP) as i8) < 1
    }

    fn grudge(&self, k: i32) {
        let pid = self.s(k, PROFILE_ID);
        let c = (self.ai.random(2).wrapping_add(1)) as u8;
        let mut draws = 0u32;
        for _ in 0..c {
            let u = loop {
                if draws >= DRAW_CAP {
                    return;
                }
                draws += 1;
                let u = self.ai.random(60) as u8;
                if u != pid {
                    break u as i32;
                }
            };
            if u >= 64 {
                continue;
            }
            if self.prof(u, 0x3C) != pid && self.prof(u, 0x3B) == 99 && self.prof(u, 0x4D) == 99 && self.prof(u, 0x2F) != 3 {
                let present = (0..8).any(|j| self.s(j, IN_CUR_SECTOR) != 0 && self.s(j, PROFILE_ID) as i32 == u);
                self.prof_put(u, if present { 0x4D } else { 0x3B }, pid);
            }
        }
    }

    fn native_leaves(&self, k: i32) {
        for i in 0..12usize {
            let b = k as i64 * REC as i64 + (i * ITEM) as i64;
            let g = |j: i64| get8(unsafe { (*self.w).soldiers }, SOLDIERS_LEN, b + j);
            let t = g(0);
            if t == 0 {
                continue;
            }
            if self.listed(t) || self.listed(g(3)) {
                let mut c = [0u8; ITEM];
                for (j, x) in c.iter_mut().enumerate() {
                    *x = g(j as i64);
                }
                stash_copy(self.w, self.h, c);
            }
        }
        self.ai.call(op::FACE_CLEAR, &[self.s(k, SLOT) as i32]);
        self.s_put(k, NICKNAME, 0);
        self.s_put(k, IN_CUR_SECTOR, 0);
        self.s_put(k, PROFILE_ID, 99);
    }

    fn listed(&self, t: u8) -> bool {
        let l = unsafe { (*self.w).listed };
        t != 0 && (0..32i64).any(|i| get8(l, 32, i) == t)
    }

    fn export(&self) {
        // 1.
        let mut found = false;
        for k in 0..8 {
            if self.s(k, IN_CUR_SECTOR) == 0 {
                break;
            }
            if self.dead(k) && self.s(k, PROFILE_ID) < 60 {
                found = true;
                break;
            }
        }
        // 2.
        if found {
            // SPEC GAP: a difficulty above 2 costs 1000.
            let cost = match unsafe { (*self.w).difficulty } {
                0 => 600,
                1 => 800,
                _ => 1000,
            };
            let can_pay = cost <= rd_balance(self.w);
            let answer = self.ai.call(op::CORPSE_ASK, &[cost, can_pay as i32]);
            if can_pay && answer != 0 {
                add(self.w, BALANCE, -cost);
                add(self.w, DAY_NET, -cost);
                self.ai.call(op::CORPSE_PAID, &[]);
            } else {
                for k in 0..8 {
                    if self.dead(k) && self.s(k, IN_CUR_SECTOR) != 0 && self.s(k, PROFILE_ID) < 60 {
                        self.grudge(k);
                    }
                }
            }
        }
        // 3.
        let mut k = 0;
        while k < 8 {
            if self.dead(k) && self.s(k, IN_CUR_SECTOR) != 0 {
                if self.s(k, PROFILE_ID) < 60 {
                    merc_leaves(self.w, self.h, k, 0);
                } else {
                    self.native_leaves(k);
                }
                team_compact(self.w, self.h, 0);
            } else {
                k += 1;
            }
        }
        self.ai.call(op::CORPSE_END, &[]);
    }
}

fn rd_balance(w: *mut Ja1World) -> i32 {
    rd(w, BALANCE)
}

/// Native leaves (k) for other modules (FIRE: strategic.md "Firing a merc").
pub(super) fn native_leaves(w: *mut Ja1World, h: *const Ja1Host, k: i32) {
    if let Some(ai) = Ai::new(w, h, 0) {
        Ctx { w, h, ai }.native_leaves(k);
    }
}

// SPEC GAP: a null world, host, soldier, profile or accounts buffer does nothing.
#[no_mangle]
pub extern "C" fn ja1_corpse_export(w: *mut Ja1World, h: *const Ja1Host) {
    if w.is_null() {
        return;
    }
    unsafe {
        if (*w).soldiers.is_null() || (*w).profiles.is_null() || (*w).accounts.is_null() {
            return;
        }
    }
    if let Some(ai) = Ai::new(w, h, 0) {
        let c = Ctx { w, h, ai };
        let _ = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| c.export()));
    }
}
