//! Day accounts (docs/spec/strategic.md "Workable trees", "Day accounts", "Worker pool and natives who quit"; C
//! interface in docs/spec/port_api.md "Strategic entry points"): `ja1_calc_ledger_all`, `ja1_apply_day_ledger`,
//! `ja1_day_end_accounts`, `ja1_recalc_territory`, `ja1_worker_pool`, `ja1_natives_quit`. A child module of `ai`; the host ops 261
//! natives_quit_msg and 262 map_redraw go through `Ai::call`. All sums are i32 with wrap-around.
use super::damage::{get16, get8, put16, put8};
use super::home::connected_home;
use super::{op, Ai, Ja1Host, Ja1World, REC};

const SECTOR_STATE_LEN: usize = 80 * 9;
const ACCOUNTS_LEN: usize = 0x98;
const PLANTS_LEN: usize = 4 * 12;
const ORDER_LEN: usize = 61;

// g_* offsets in `accounts` (strategic.md Day accounts).
const TEAM_COUNT: i64 = 0x08;
const TREE_PRICE: i64 = 0x10;
const TREES_WORKABLE: i64 = 0x14;
const TREES_OWNED: i64 = 0x18;
const TREES_TAPPED: i64 = 0x1C;
pub(super) const DAY_INCOME: i64 = 0x20;
pub(super) const BALANCE: i64 = 0x24;
const BALANCE_AFTER_DAY: i64 = 0x28;
pub(super) const DAY_NET: i64 = 0x2C;
const TEAM_HALF_SALARIES: i64 = 0x30;
const TEAM_SALARIES: i64 = 0x34;
const TEAM_PAY: i64 = 0x38;
const WORKER_POOL: i64 = 0x3C;
const TAPPERS: i64 = 0x40;
const GUARDS: i64 = 0x44;
const WORKERS: i64 = 0x48;
const IDLE_POOL: i64 = 0x4C;
const MERC_DEATHS_TOTAL: i64 = 0x00;
const NATIVE_DEATHS_TOTAL: i64 = 0x50;
const ENEMY_DEATHS_TOTAL: i64 = 0x6C;
const NATIVE_WAGE: i64 = 0x54;
const TAPPERS_COST: i64 = 0x58;
const GUARDS_COST: i64 = 0x5C;
const NATIVES_COST: i64 = 0x60;
const DAY_EXPENSES: i64 = 0x64;
const SECTORS_OWNED: i64 = 0x74;
const SECTORS_GAINED: i64 = 0x78;
const SECTORS_LOST: i64 = 0x7C;
const PROCESS_CAP: i64 = 0x84;
const PLANTS_OWNED: i64 = 0x88;
const LOST_EXTRA: i64 = 0x91; // u8 ("byte at 0x22A031")
const POOL_MINUS: i64 = 0x92; // u16 ("16-bit value at 0x22A032")
const POOL_PLUS: i64 = 0x94; // u16 ("16-bit value at 0x22A034")

// sectorState byte offsets.
const FLAGS: usize = 0;
const TAPPERS_B: usize = 3;
const GUARDS_B: usize = 4;
const TREES: usize = 6;
const WORKABLE: usize = 7;
const UNHEALTHY: usize = 8;

// soldier record offsets (defs/types.toml).
const SALARY: usize = 0x110;
const SALARY2: usize = 0x112;
const DUTY: usize = 0x17E;
const IN_CUR_SECTOR: usize = 0x182;
const FATIGUE: usize = 0x10C;
const WOUNDS: usize = 0x185;
const ENERGY: usize = 0x18A;
const B1B1: usize = 0x1B1;

fn acc(w: *mut Ja1World) -> *mut u8 {
    unsafe { (*w).accounts }
}
pub(super) fn rd(w: *mut Ja1World, off: i64) -> i32 {
    let p = acc(w);
    (get16(p, ACCOUNTS_LEN, off) as u32 | (get16(p, ACCOUNTS_LEN, off + 2) as u32) << 16) as i32
}
pub(super) fn wr(w: *mut Ja1World, off: i64, v: i32) {
    let p = acc(w);
    put16(p, ACCOUNTS_LEN, off, v as u16);
    put16(p, ACCOUNTS_LEN, off + 2, (v as u32 >> 16) as u16);
}
pub(super) fn add(w: *mut Ja1World, off: i64, v: i32) {
    wr(w, off, rd(w, off).wrapping_add(v));
}

/// Byte i of sector_state entry s (0 outside the table).
fn sec(w: *mut Ja1World, s: u8, i: usize) -> u8 {
    get8(unsafe { (*w).sector_state }, SECTOR_STATE_LEN, s as i64 * 9 + i as i64)
}
fn sec_put(w: *mut Ja1World, s: u8, i: usize, v: u8) {
    put8(unsafe { (*w).sector_state }, SECTOR_STATE_LEN, s as i64 * 9 + i as i64, v)
}
fn owned(w: *mut Ja1World, s: u8) -> bool {
    sec(w, s, FLAGS) & 1 != 0
}
fn connected(w: *mut Ja1World, s: u8) -> bool {
    connected_home(w, s as i32) != 0
}

/// Workable trees of sector s (writes `workable`, returns it).
pub(super) fn workable(w: *mut Ja1World, s: u8) -> u8 {
    let v = if owned(w, s) && connected(w, s) {
        let b0 = sec(w, s, FLAGS);
        let si = s as i32;
        // (side is the map edge, neighbour, barrier bit): north, east, south, west.
        let sides = [(si < 10, si - 10, 0x02u8), (si % 10 == 9, si + 1, 0x04), (si >= 50, si + 10, 0x08), (si % 10 == 0, si - 1, 0x10)];
        let mut security: u32 = 0;
        for (edge, nb, bit) in sides {
            if edge || owned(w, nb as u8) || b0 & bit != 0 {
                security += 25;
            }
        }
        let trees = sec(w, s, TREES);
        // SPEC?: "All arithmetic is on bytes": byte 6 − byte 8 wraps as an unsigned byte and the lower of the two
        // is taken unsigned.
        let a = trees.wrapping_sub(sec(w, s, UNHEALTHY));
        let b = (trees as u32 * security / 100) as u8;
        a.min(b)
    } else {
        0
    };
    sec_put(w, s, WORKABLE, v);
    v
}

/// Plants (`calc_process_cap`).
pub(super) fn calc_process_cap(w: *mut Ja1World) {
    wr(w, PLANTS_OWNED, 0);
    wr(w, PROCESS_CAP, 0);
    for i in 0..4i64 {
        let p = unsafe { (*w).plants };
        let c = get8(p, PLANTS_LEN, i * 12 + 4);
        if owned(w, c) {
            add(w, PLANTS_OWNED, 1);
            let p = unsafe { (*w).plants };
            if connected(w, c) && get8(p, PLANTS_LEN, i * 12 + 8) == 2 {
                let cap = (get16(p, PLANTS_LEN, i * 12) as u32 | (get16(p, PLANTS_LEN, i * 12 + 2) as u32) << 16) as i32;
                add(w, PROCESS_CAP, cap);
            }
        }
    }
}

/// Territory (`recalc_territory`).
pub(super) fn recalc_territory(w: *mut Ja1World) {
    for off in [SECTORS_OWNED, TAPPERS, GUARDS, TREES_OWNED, TREES_WORKABLE, TREES_TAPPED] {
        wr(w, off, 0);
    }
    for s in 0..60u8 {
        if !owned(w, s) {
            sec_put(w, s, WORKABLE, 0);
            // F53 (1.15m RecalcTerritory 0x50EF6 jumps to 0x50FA9): a sector not owned keeps its tappers and guards.
            if !crate::fanfix::on(53) {
                sec_put(w, s, GUARDS_B, 0);
                sec_put(w, s, TAPPERS_B, 0);
            }
            continue;
        }
        add(w, SECTORS_OWNED, 1);
        add(w, TAPPERS, sec(w, s, TAPPERS_B) as i32);
        add(w, GUARDS, sec(w, s, GUARDS_B) as i32);
        add(w, TREES_OWNED, sec(w, s, TREES) as i32);
        if !connected(w, s) {
            sec_put(w, s, WORKABLE, 0);
        } else {
            let wk = workable(w, s);
            add(w, TREES_WORKABLE, wk as i32);
            // SPEC?: the lower of the two is taken as unsigned bytes.
            let t = sec(w, s, TAPPERS_B) / 3;
            let mut free = sec(w, s, TREES).wrapping_sub(sec(w, s, UNHEALTHY));
            // F26 (1.15m RecalcTerritory 0x50F75 + helper 0x62356): free is first capped by the workable count, a
            // signed byte compare, so trees drawn as not workable bring no income.
            if crate::fanfix::on(26) && free as i8 > wk as i8 {
                free = wk;
            }
            add(w, TREES_TAPPED, t.min(free) as i32);
        }
    }
    if rd(w, PROCESS_CAP) < rd(w, TREES_TAPPED) {
        wr(w, TREES_TAPPED, rd(w, PROCESS_CAP));
    }
    wr(w, WORKERS, rd(w, TAPPERS).wrapping_add(rd(w, GUARDS)));
    let idle = rd(w, WORKER_POOL).wrapping_sub(rd(w, WORKERS));
    wr(w, IDLE_POOL, idle.max(0));
}

/// Natives cost (`calc_natives_cost`).
fn calc_natives_cost(w: *mut Ja1World) {
    let wage = rd(w, NATIVE_WAGE);
    let t = wage.wrapping_mul(rd(w, TAPPERS));
    let g = wage.wrapping_mul(rd(w, GUARDS));
    wr(w, TAPPERS_COST, t);
    wr(w, GUARDS_COST, g);
    wr(w, NATIVES_COST, t.wrapping_add(g));
}

/// Team pay (`calc_team_pay`).
fn calc_team_pay(w: *mut Ja1World) {
    let sp = unsafe { (*w).soldiers };
    let len = 8 * REC;
    let (mut count, mut sal, mut halves, mut pay, mut on_duty) = (0i32, 0i32, 0i32, 0i32, 0i32);
    for k in 0..8usize {
        let b = (k * REC) as i64;
        if get8(sp, len, b + IN_CUR_SECTOR as i64) == 0 {
            break;
        }
        let salary = get16(sp, len, b + SALARY as i64) as i32;
        let half = (salary + 1) / 2;
        let duty = get8(sp, len, b + DUTY as i64);
        let charge = if duty == 1 || duty == 4 { half } else { salary };
        put16(sp, len, b + SALARY2 as i64, charge as u16);
        count += 1;
        sal = sal.wrapping_add(salary);
        halves = halves.wrapping_add(half);
        pay = pay.wrapping_add(charge);
        if duty == 0 {
            on_duty += 1;
        }
    }
    wr(w, TEAM_COUNT, count);
    wr(w, TEAM_SALARIES, sal);
    wr(w, TEAM_HALF_SALARIES, halves);
    wr(w, TEAM_PAY, pay);
    // g_teamOnDuty is the scalar world field, not accounts + 0x0C.
    unsafe { (*w).team_on_duty = on_duty };
}

/// Ledger (`calc_ledger`).
fn calc_ledger(w: *mut Ja1World) {
    let income = rd(w, TREE_PRICE).wrapping_mul(rd(w, TREES_TAPPED));
    let expenses = rd(w, NATIVES_COST).wrapping_add(rd(w, TEAM_PAY));
    wr(w, DAY_INCOME, income);
    wr(w, DAY_EXPENSES, expenses);
    wr(w, DAY_NET, income.wrapping_sub(expenses));
}

/// All accounts (`calc_ledger_all`).
pub(super) fn calc_ledger_all(w: *mut Ja1World) {
    calc_process_cap(w);
    recalc_territory(w);
    calc_natives_cost(w);
    calc_team_pay(w);
    calc_ledger(w);
    wr(w, BALANCE_AFTER_DAY, rd(w, BALANCE).wrapping_add(rd(w, DAY_NET)));
}

/// Pay the day (`apply_day_ledger`).
pub(super) fn apply_day_ledger(w: *mut Ja1World) {
    calc_team_pay(w);
    calc_ledger(w);
    let prev = unsafe { (*w).prev_balance };
    wr(w, BALANCE, prev.wrapping_add(rd(w, DAY_NET)));
}

/// Sectors-gained bonus (`gained_bonus`).
fn gained_bonus(g: i32) -> i32 {
    let mut sum = 0i32;
    for i in (0..=140).step_by(10) {
        if i < g {
            sum = sum.wrapping_add(g.wrapping_sub(i).min(10).wrapping_mul(15 - i / 10));
        }
    }
    sum
}

/// Worker pool (`worker_pool`).
pub(super) fn worker_pool(w: *mut Ja1World, h: *const Ja1Host) {
    let k = unsafe { (*w).difficulty } as i8;
    // SPEC GAP (strategic.md): k outside 0..2 uses k = 0.
    let k = if (0..=2).contains(&k) { k as usize } else { 0 };
    let base: u16 = [20, 15, 10][k];
    let loss: u16 = [15, 20, 25][k];
    let wv = (rd(w, NATIVE_WAGE) as u16).wrapping_sub(35) as i16;
    let l = (get8(acc(w), ACCOUNTS_LEN, LOST_EXTRA) as u16).wrapping_add(rd(w, SECTORS_LOST) as u16) as i16;
    let gained = rd(w, SECTORS_GAINED);
    let q = (wv as i32).wrapping_mul((l as i32).wrapping_add(gained)).wrapping_div(5);
    let bonus = gained_bonus(gained);
    let v = base
        .wrapping_add(wv as u16)
        .wrapping_add(bonus as u16)
        .wrapping_sub(loss.wrapping_mul(l as u16))
        .wrapping_add(q as u16)
        .wrapping_add(get16(acc(w), ACCOUNTS_LEN, POOL_PLUS))
        .wrapping_sub(get16(acc(w), ACCOUNTS_LEN, POOL_MINUS))
        .wrapping_sub(rd(w, NATIVE_DEATHS_TOTAL) as u16);
    let v = (v as i16 as i32).clamp(0, 500);
    wr(w, WORKER_POOL, v);
    recalc_territory(w);
    let idle = rd(w, WORKER_POOL).wrapping_sub(rd(w, WORKERS));
    wr(w, IDLE_POOL, idle);
    if idle < 0 {
        natives_quit(w, h, idle.wrapping_neg() as u8);
        wr(w, IDLE_POOL, 0);
        calc_ledger_all(w);
        if let Some(a) = Ai::new(w, h, 0) {
            a.call(op::MAP_REDRAW, &[]);
        }
    }
}

/// Natives quit (`natives_quit`).
pub(super) fn natives_quit(w: *mut Ja1World, h: *const Ja1Host, n: u8) {
    let mut n = n;
    let (mut g, mut t) = (0u8, 0u8);
    let order = |i: usize| get8(unsafe { (*w).sector_order }, ORDER_LEN, i as i64);
    // 1.
    if n != 0 {
        for i in 0..ORDER_LEN {
            let s = order(i);
            if !connected(w, s) {
                continue;
            }
            while sec(w, s, GUARDS_B) != 0 && n != 0 {
                sec_put(w, s, GUARDS_B, sec(w, s, GUARDS_B) - 1);
                n -= 1;
                g = g.wrapping_add(1);
            }
        }
    }
    // 2.
    if g != 0 {
        if let Some(a) = Ai::new(w, h, 0) {
            a.call(op::NATIVES_QUIT_MSG, &[g as i32, if g == 1 { 0 } else { 1 }]);
        }
    }
    // 3.
    if n != 0 {
        for i in 0..ORDER_LEN {
            let s = order(i);
            if !connected(w, s) {
                continue;
            }
            while sec(w, s, TAPPERS_B) != 0 && n != 0 {
                sec_put(w, s, TAPPERS_B, sec(w, s, TAPPERS_B) - 1);
                n -= 1;
                t = t.wrapping_add(1);
            }
            let tp = sec(w, s, TAPPERS_B);
            let r = tp % 3;
            if r != 0 {
                sec_put(w, s, TAPPERS_B, tp - r);
                t = t.wrapping_add(r);
                n = if n <= r { 0 } else { n - r };
            }
        }
    }
    // 4. The count is G, not T (original bug; strategic.md). Port fix P03 (#71, #2 row 13): T, and the singular
    // from T.
    if t != 0 {
        if let Some(a) = Ai::new(w, h, 0) {
            let c = if crate::fanfix::port_on(3) { t } else { g };
            a.call(op::NATIVES_QUIT_MSG, &[c as i32, if c <= 1 { 2 } else { 3 }]);
        }
    }
}

/// The day-end accounts (`day_end_accounts`, strategic.md "The day-end accounts").
fn day_end_accounts(w: *mut Ja1World) {
    // 1. Death totals.
    let (m, a, n, e) = unsafe { ((*w).dead_mercs, (*w).dead_allies, (*w).dead_npc, (*w).enemy_deaths) };
    add(w, MERC_DEATHS_TOTAL, m);
    add(w, NATIVE_DEATHS_TOTAL, a.wrapping_add(n));
    add(w, ENEMY_DEATHS_TOTAL, e);
    // 2.
    calc_process_cap(w);
    recalc_territory(w);
    apply_day_ledger(w);
    // 3. Rest of the team (all 8 records).
    let sp = unsafe { (*w).soldiers };
    let len = 8 * REC;
    for k in 0..8usize {
        let b = (k * REC) as i64;
        put8(sp, len, b + WOUNDS as i64, 0);
        let v = get8(sp, len, b + B1B1 as i64);
        if v != 0 {
            put8(sp, len, b + B1B1 as i64, v - 1);
        }
        put8(sp, len, b + ENERGY as i64, 100);
        put16(sp, len, b + FATIGUE as i64, 0);
    }
    // 4. Tappers on lost trees.
    for s in 0..60u8 {
        let t = sec(w, s, TAPPERS_B);
        if !owned(w, s) || !connected(w, s) || t == 0 {
            continue;
        }
        let a = sec(w, s, WORKABLE).wrapping_mul(3);
        if a < t {
            let e = (t - a) as i32;
            sec_put(w, s, TAPPERS_B, a);
            add(w, TAPPERS, e.wrapping_neg());
            add(w, WORKERS, e.wrapping_neg());
        }
    }
}

// SPEC GAP: a null world does nothing. A null host or soldier buffer skips the host ops 261 / 262 (Ai::new).
fn run(w: *mut Ja1World, f: impl FnOnce()) {
    if w.is_null() {
        return;
    }
    let _ = std::panic::catch_unwind(std::panic::AssertUnwindSafe(f));
}

#[no_mangle]
pub extern "C" fn ja1_calc_ledger_all(w: *mut Ja1World, _h: *const Ja1Host) {
    run(w, || calc_ledger_all(w));
}
#[no_mangle]
pub extern "C" fn ja1_apply_day_ledger(w: *mut Ja1World, _h: *const Ja1Host) {
    run(w, || apply_day_ledger(w));
}
#[no_mangle]
pub extern "C" fn ja1_day_end_accounts(w: *mut Ja1World, _h: *const Ja1Host) {
    run(w, || day_end_accounts(w));
}
#[no_mangle]
pub extern "C" fn ja1_recalc_territory(w: *mut Ja1World, _h: *const Ja1Host) {
    run(w, || recalc_territory(w));
}
#[no_mangle]
pub extern "C" fn ja1_worker_pool(w: *mut Ja1World, h: *const Ja1Host) {
    run(w, || worker_pool(w, h));
}
#[no_mangle]
pub extern "C" fn ja1_natives_quit(w: *mut Ja1World, h: *const Ja1Host, n: i32) {
    run(w, || natives_quit(w, h, n as u8));
}

#[cfg(test)]
mod tests {
    use super::*;

    struct T {
        w: Box<Ja1World>,
        acc: Vec<u8>,
        ss: Vec<u8>,
        sol: Vec<u8>,
        order: Vec<u8>,
    }
    fn world() -> T {
        let mut t = T {
            w: Box::new(unsafe { std::mem::zeroed() }),
            acc: vec![0; ACCOUNTS_LEN],
            ss: vec![0; SECTOR_STATE_LEN],
            sol: vec![0; 48 * REC],
            order: (0..61).map(|i| if i < 60 { 59 - i } else { 59 }).collect(),
        };
        t.w.accounts = t.acc.as_mut_ptr();
        t.w.sector_state = t.ss.as_mut_ptr();
        t.w.soldiers = t.sol.as_mut_ptr();
        t.w.sector_order = t.order.as_mut_ptr();
        t
    }
    fn wp(t: &mut T) -> *mut Ja1World {
        &mut *t.w
    }

    #[test]
    fn bonus() {
        assert_eq!(gained_bonus(0), 0);
        assert_eq!(gained_bonus(3), 45);
        assert_eq!(gained_bonus(12), 150 + 28);
        assert_eq!(gained_bonus(200), (1..=15).sum::<i32>() * 10);
    }

    #[test]
    fn territory_and_workable() {
        let mut t = world();
        let w = wp(&mut t);
        // home 59 and 58 owned; 58 has 100 trees, 10 unhealthy, 9 tappers.
        t.ss[59 * 9] = 1;
        t.ss[58 * 9] = 1;
        t.ss[58 * 9 + 6] = 100;
        t.ss[58 * 9 + 8] = 10;
        t.ss[58 * 9 + 3] = 9;
        t.ss[3 * 9 + 3] = 5; // unowned: cleared
        t.acc[PROCESS_CAP as usize] = 100;
        t.acc[WORKER_POOL as usize] = 20;
        recalc_territory(w);
        // 58: south edge + east owned = 50 -> min(90, 50) = 50.
        assert_eq!(t.ss[58 * 9 + 7], 50);
        assert_eq!(t.ss[3 * 9 + 3], 0);
        assert_eq!(rd(w, SECTORS_OWNED), 2);
        assert_eq!(rd(w, TREES_WORKABLE), 50);
        assert_eq!(rd(w, TREES_TAPPED), 3);
        assert_eq!(rd(w, IDLE_POOL), 11);
    }

    #[test]
    fn team_pay_and_on_duty() {
        let mut t = world();
        let w = wp(&mut t);
        for (k, (sal, duty)) in [(101u16, 0u8), (50, 1), (40, 2)].iter().enumerate() {
            let b = k * REC;
            t.sol[b + IN_CUR_SECTOR] = 1;
            t.sol[b + SALARY..b + SALARY + 2].copy_from_slice(&sal.to_le_bytes());
            t.sol[b + DUTY] = *duty;
        }
        t.acc[0x0C] = 0x77;
        calc_team_pay(w);
        assert_eq!(rd(w, TEAM_COUNT), 3);
        assert_eq!(rd(w, TEAM_PAY), 101 + 25 + 40);
        assert_eq!(rd(w, TEAM_HALF_SALARIES), 51 + 25 + 20);
        assert_eq!(t.w.team_on_duty, 1);
        assert_eq!(t.acc[0x0C], 0x77);
        assert_eq!(u16::from_le_bytes([t.sol[REC + SALARY2], t.sol[REC + SALARY2 + 1]]), 25);
    }

    #[test]
    fn quit_takes_guards_then_tappers_in_threes() {
        let mut t = world();
        let w = wp(&mut t);
        t.ss[59 * 9] = 1;
        t.ss[59 * 9 + 4] = 2;
        t.ss[59 * 9 + 3] = 7;
        natives_quit(w, std::ptr::null(), 3);
        assert_eq!(t.ss[59 * 9 + 4], 0);
        // one tapper by count, then 6 mod 3 = 0.
        assert_eq!(t.ss[59 * 9 + 3], 6);
    }
}
