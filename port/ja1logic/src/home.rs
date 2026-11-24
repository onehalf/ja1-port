//! Joined to home (docs/spec/soldier_update.md "Joined to home"; C interface in docs/spec/port_api.md):
//! `ja1_connected_home`, library op 43. A child module of `ai`. No host calls, no random numbers.
use super::damage::get8;
use super::Ja1World;

const SECTOR_STATE_LEN: usize = 80 * 9;
const STEP: [i32; 4] = [1, 10, -1, -10];
const TRY: [u8; 4] = [0x04, 0x08, 0x10, 0x02];
const BACK: [u8; 4] = [0x10, 0x02, 0x04, 0x08];

/// Byte 0 of sector_state entry n (0 outside 0..79).
fn b0(w: *const Ja1World, n: i32) -> u8 {
    if !(0..80).contains(&n) {
        return 0;
    }
    get8(unsafe { (*w).sector_state }, SECTOR_STATE_LEN, n as i64 * 9)
}

pub(super) fn connected_home(w: *const Ja1World, sector: i32) -> i32 {
    let c = sector as u8;
    if c == 59 {
        return 1;
    }
    if c >= 60 {
        return 0;
    }
    let mut a = [0u8; 180];
    for n in 0..60 {
        a[120 + n] = b0(w, n as i32);
    }
    let start = c as usize;
    let mut s = start;
    let mut d = 0usize;
    let mut top = 1usize;
    a[0] = c;
    if a[120 + s] & 1 == 0 {
        return 0;
    }
    for _ in 0..1_000_000u32 {
        let mut moved = false;
        while !moved {
            if a[120 + s] & TRY[d] == 0 {
                a[120 + s] |= TRY[d];
                moved = true;
            }
            if a[120 + s] >= 0x1E {
                if a[60 + s] != 0 {
                    top -= 1;
                    if top == 0 && a[60 + start] != 0 {
                        return 0;
                    } else if top == 0 {
                        s = start;
                        top = 1;
                    } else {
                        s = a[top] as usize;
                    }
                } else {
                    a[60 + s] = 1;
                }
            }
            if !moved {
                d += 1;
            }
            if d > 3 {
                d = 0;
            }
        }
        let n = (s as i32 + STEP[d]) as u8;
        let blocked = (s % 10 == 0 && d == 2)
            || (s % 10 == 9 && d == 0)
            || n >= 60
            || a[120 + n as usize] & 1 == 0;
        if !blocked {
            s = n as usize;
            if a[60 + s] == 0 {
                a[120 + s] |= BACK[d];
                let si = s as i32;
                let cnt = [si + 1, si + 10, si - 1, si - 10].iter().filter(|&&k| b0(w, k) & 1 != 0).count();
                if cnt > 2 {
                    if top >= 180 {
                        return 0;
                    }
                    a[top] = s as u8;
                    top += 1;
                    d = 0;
                }
            }
        }
        if s == 59 {
            return 1;
        }
    }
    0
}

#[no_mangle]
pub extern "C" fn ja1_connected_home(w: *mut Ja1World, _h: *const super::Ja1Host, sector: i32) -> i32 {
    std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| connected_home(w, sector))).unwrap_or(0)
}
