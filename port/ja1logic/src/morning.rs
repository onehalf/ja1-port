//! Back on duty and profile sync (docs/spec/strategic.md "Back on duty", "Profiles take the team's stats";
//! C interface in docs/spec/port_api.md): `ja1_duty_wake`, `ja1_profile_sync`. No host ops.
use super::damage::{get16, get8, put16, put8};
use super::{Ja1Host, Ja1World, REC};

const SOLDIERS_LEN: usize = 48 * REC;
const PROFILE: usize = 0x5A;
const PROFILES_LEN: usize = 64 * PROFILE;

const SALARY: usize = 0x110;
const SALARY2: usize = 0x112;
const PROFILE_ID: usize = 0x179;
const DUTY: usize = 0x17E;
const IN_CUR_SECTOR: usize = 0x182;
const HP: usize = 0x183;
const HP_MAX: usize = 0x184;
const WOUNDS: usize = 0x185;
const LEVEL: usize = 0x192;

fn base(k: i64) -> i64 {
    k * REC as i64
}

/// (soldier offset, profile offset) of the copied stats.
const COPY: [(usize, usize); 9] = [
    (LEVEL, 0x44),
    (HP_MAX, 0x2A),
    (0x18B, 0x2C),
    (0x18C, 0x42),
    (0x18D, 0x2E),
    (0x18E, 0x40),
    (0x190, 0x41),
    (0x191, 0x2D),
    (0x18F, 0x43),
];

fn duty_wake(w: *mut Ja1World) {
    let sol = unsafe { (*w).soldiers };
    let g = unsafe { (*w).duty7_merc };
    if g < 0x40 && get8(sol, SOLDIERS_LEN, base(g as i64) + DUTY as i64) == 7 && (g as usize) < 48 {
        put8(sol, SOLDIERS_LEN, base(g as i64) + DUTY as i64, 0);
        unsafe { (*w).duty7_merc = 0x40 };
    }
    // Healed: signed hpMax - signed hp equals the signed wounds byte (integers, no wrap).
    let healed = |k: i64| {
        let g = |o: usize| get8(sol, SOLDIERS_LEN, base(k) + o as i64) as i8 as i32;
        g(HP_MAX) - g(HP) == g(WOUNDS)
    };
    let duty = |k: i64| get8(sol, SOLDIERS_LEN, base(k) + DUTY as i64);
    for k in 0..8i64 {
        if duty(k) == 4 && healed(k) {
            let s = get16(sol, SOLDIERS_LEN, base(k) + SALARY as i64);
            put16(sol, SOLDIERS_LEN, base(k) + SALARY2 as i64, s);
            unsafe { (*w).team_on_duty = (*w).team_on_duty.wrapping_add(1) };
            put8(sol, SOLDIERS_LEN, base(k) + DUTY as i64, 0);
        }
    }
    if (0..8i64).any(|k| duty(k) == 4) {
        return;
    }
    for k in 0..8i64 {
        if duty(k) == 3 && healed(k) {
            unsafe { (*w).team_on_duty = (*w).team_on_duty.wrapping_add(1) };
            put8(sol, SOLDIERS_LEN, base(k) + DUTY as i64, 0);
        }
    }
}

fn profile_sync(w: *mut Ja1World) {
    let sol = unsafe { (*w).soldiers };
    let prof = unsafe { (*w).profiles };
    for k in 0..8i64 {
        let b = base(k);
        if get8(sol, SOLDIERS_LEN, b + IN_CUR_SECTOR as i64) == 0 {
            continue;
        }
        let p = get8(sol, SOLDIERS_LEN, b + PROFILE_ID as i64) as i64;
        if p >= 64 {
            continue;
        }
        let pb = p * PROFILE as i64;
        put16(prof, PROFILES_LEN, pb + 0x28, get16(sol, SOLDIERS_LEN, b + SALARY as i64));
        for (so, po) in COPY {
            put8(prof, PROFILES_LEN, pb + po as i64, get8(sol, SOLDIERS_LEN, b + so as i64));
        }
    }
}

#[no_mangle]
pub extern "C" fn ja1_duty_wake(w: *mut Ja1World, _h: *const Ja1Host) {
    if w.is_null() || unsafe { (*w).soldiers.is_null() } {
        return;
    }
    let _ = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| duty_wake(w)));
}

#[no_mangle]
pub extern "C" fn ja1_profile_sync(w: *mut Ja1World, _h: *const Ja1Host) {
    if w.is_null() || unsafe { (*w).soldiers.is_null() || (*w).profiles.is_null() } {
        return;
    }
    let _ = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| profile_sync(w)));
}
