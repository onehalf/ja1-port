//! Morning pay (docs/spec/strategic.md "Morning pay"; C interface in docs/spec/port_api.md "Strategic entry
//! points"): `ja1_cash_count`, `ja1_collect_cash`, `ja1_stash_item`, `ja1_merc_leaves`, `ja1_team_compact`,
//! `ja1_pay_mercs`. A child module of `ai`; the host ops 263..269 go through `Ai::call`. "Pay the day" is
//! `ledger::apply_day_ledger`. Bytes are unsigned unless stated; sums are i32 with wrap-around.
use super::damage::{get16, get8, put16, put8};
use super::ledger::{add, apply_day_ledger, BALANCE, DAY_INCOME, DAY_NET};
use super::{op, Ai, Ja1Host, Ja1World, REC};

const SOLDIERS_LEN: usize = 48 * REC;
const ITEM: usize = 12;
const ITEM_TYPES_LEN: usize = 136 * 12;
const LISTED_LEN: usize = 32;
const STASH_LEN: usize = 136;
const PROFILE: usize = 0x5A;
const PROFILES_LEN: usize = 64 * PROFILE;
const PROGRESS_ROW: usize = 0x12;
const PROGRESS_LEN: usize = 8 * PROGRESS_ROW;
const GREED_LEN: usize = 256;
const ACCOUNTS_LEN: usize = 0xA8;
const RESIGNATIONS: i64 = 0xA0;
const MONEY: u8 = 0x6F;

// soldier record offsets (defs/types.toml "soldier").
const SALARY: usize = 0x110;
const SALARY2: usize = 0x112;
const NICKNAME: usize = 0x16E;
const PROFILE_ID: usize = 0x179;
const SLOT: usize = 0x17A;
const DUTY: usize = 0x17E;
const IN_CUR_SECTOR: usize = 0x182;
const HP: usize = 0x183;
const HP_MAX: usize = 0x184;
const WOUNDS: usize = 0x185;
const ENERGY: usize = 0x18A;
const AGILITY: usize = 0x18B;
const DEXTERITY: usize = 0x18C;
const WISDOM: usize = 0x18D;
const MEDICAL: usize = 0x18E;
const EXPLOSIVES: usize = 0x18F;
const MECHANICAL: usize = 0x190;
const MARKSMANSHIP: usize = 0x191;
const LEVEL: usize = 0x192;
const B19C: usize = 0x19C;
const B19D: usize = 0x19D;

// profile offsets (defs/types.toml "profile").
const P_SALARY: usize = 0x28;
const P_TEAM_STATUS: usize = 0x4B;
const P_B3A: usize = 0x3A;
const P_B3C: usize = 0x3C;
const P_KIT: usize = 0x50;

/// (soldier byte, profile byte) pairs Merc leaves copies.
const STATS: [(usize, usize); 9] = [
    (HP_MAX, 0x2A),
    (AGILITY, 0x2C),
    (MARKSMANSHIP, 0x2D),
    (WISDOM, 0x2E),
    (MEDICAL, 0x40),
    (MECHANICAL, 0x41),
    (DEXTERITY, 0x42),
    (EXPLOSIVES, 0x43),
    (LEVEL, 0x44),
];

fn sol(w: *mut Ja1World) -> *mut u8 {
    unsafe { (*w).soldiers }
}
/// Byte `off` of soldier `slot` (any slot; outside the 48 records reads 0 and is not written).
fn sb(w: *mut Ja1World, slot: i32, off: usize) -> u8 {
    get8(sol(w), SOLDIERS_LEN, slot as i64 * REC as i64 + off as i64)
}
fn sb_put(w: *mut Ja1World, slot: i32, off: usize, v: u8) {
    put8(sol(w), SOLDIERS_LEN, slot as i64 * REC as i64 + off as i64, v)
}
fn s16(w: *mut Ja1World, slot: i32, off: usize) -> u16 {
    get16(sol(w), SOLDIERS_LEN, slot as i64 * REC as i64 + off as i64)
}
fn s16_put(w: *mut Ja1World, slot: i32, off: usize, v: u16) {
    put16(sol(w), SOLDIERS_LEN, slot as i64 * REC as i64 + off as i64, v)
}
fn place(slot: i32, i: usize) -> i64 {
    slot as i64 * REC as i64 + (i * ITEM) as i64
}

/// One host call (no call when the host or the soldier buffer is null: result 0).
fn hcall(w: *mut Ja1World, h: *const Ja1Host, opc: i32, args: &[i32]) -> u32 {
    match Ai::new(w, h, 0) {
        Some(a) => a.call(opc, args),
        None => 0,
    }
}

fn is_listed(w: *mut Ja1World, t: u8) -> bool {
    let l = unsafe { (*w).listed };
    t != 0 && (0..LISTED_LEN as i64).any(|i| get8(l, LISTED_LEN, i) == t)
}

/// An item entry: in the soldier buffer (bytes past the 12 read the following soldier bytes as they lie), or a
/// 12-byte copy (bytes past the 12 read 0).
enum Item {
    At(i64),
    Copy([u8; ITEM]),
}
impl Item {
    fn get(&self, w: *mut Ja1World, i: i64) -> u8 {
        match self {
            Item::At(b) => get8(sol(w), SOLDIERS_LEN, b + i),
            Item::Copy(c) => if (0..ITEM as i64).contains(&i) { c[i as usize] } else { 0 },
        }
    }
    fn clear(&mut self, w: *mut Ja1World, n: usize) {
        match self {
            Item::At(b) => (0..n as i64).for_each(|i| put8(sol(w), SOLDIERS_LEN, *b + i, 0)),
            Item::Copy(c) => c[..n].iter_mut().for_each(|x| *x = 0),
        }
    }
}

/// Money in an item (`cash_value`) at soldier-buffer offset b.
fn cash_value(w: *mut Ja1World, b: i64) -> i32 {
    let g = |i: i64| get8(sol(w), SOLDIERS_LEN, b + i);
    if g(0) != MONEY {
        return 0;
    }
    let n = g(1) as i8 as i32;
    let mut v = 10i32.wrapping_mul(g(2) as i8 as i32);
    for j in 1..n {
        v = v.wrapping_add(10 * (g(7 + j as i64) as i8 as i32));
    }
    v
}

/// Cash carried (`cash_count`).
pub(super) fn cash_count(w: *mut Ja1World, slot: i32, drop: u8) -> i32 {
    if sb(w, slot, IN_CUR_SECTOR) == 0 {
        return 0;
    }
    let mut total = 0i32;
    for i in [0usize, 7, 8, 9, 10, 11, 1] {
        let b = place(slot, i);
        let v = cash_value(w, b);
        total = total.wrapping_add(v);
        if v != 0 && drop != 0 {
            Item::At(b).clear(w, ITEM);
        }
    }
    total
}

/// Collect the cash (`collect_cash`).
pub(super) fn collect_cash(w: *mut Ja1World) {
    let mut total = 0i32;
    for k in 0..8 {
        total = total.wrapping_add(cash_count(w, k, 1));
    }
    if total > 0 {
        add(w, DAY_INCOME, total);
        add(w, BALANCE, total);
        add(w, DAY_NET, total);
    }
}

/// Store an item (`stash_item`): true when it passed its first tests (type not 0, room below 100).
fn stash_item(w: *mut Ja1World, h: *const Ja1Host, e: &mut Item) -> bool {
    let t = e.get(w, 0);
    if t == 0 {
        return false;
    }
    let c = get8(unsafe { (*w).item_types }, ITEM_TYPES_LEN, t as i64 * 12 + 6);
    let k: i32 = if c < 2 { 1 } else { e.get(w, 1) as i8 as i32 };
    if k + get8(unsafe { (*w).stash_counts }, STASH_LEN, t as i64) as i32 >= 100 {
        return false;
    }
    let mut recs: Vec<[u8; 3]> = Vec::new();
    if c < 2 {
        recs.push([t, e.get(w, 1), e.get(w, 2)]);
        if e.get(w, 3) != 0 {
            recs.push([e.get(w, 3), e.get(w, 4), e.get(w, 5)]);
        }
    } else {
        for j in 0..k.max(0) as i64 {
            recs.push([t, 1, if j == 0 { e.get(w, 2) } else { e.get(w, 7 + j) }]);
        }
    }
    let mut r = 0u8;
    for rec in recs {
        r = hcall(w, h, op::STASH_PUT, &[rec[0] as i32, rec[1] as i32, rec[2] as i32]) as u8;
        if r != 0 {
            let sc = unsafe { (*w).stash_counts };
            let n = get8(sc, STASH_LEN, rec[0] as i64);
            put8(sc, STASH_LEN, rec[0] as i64, n.wrapping_add(1));
        }
    }
    if r != 0 {
        e.clear(w, 6);
    }
    true
}

/// Store an item held in a 12-byte buffer (the cursor of the assignment screen); see `stash_item`.
pub(super) fn stash_buf(w: *mut Ja1World, h: *const Ja1Host, b: &mut [u8; ITEM]) -> bool {
    let mut it = Item::Copy(*b);
    let r = stash_item(w, h, &mut it);
    if let Item::Copy(c) = it {
        *b = c;
    }
    r
}

/// Store an item with a copy of 12 bytes (Native leaves, strategic.md Corpse export).
pub(super) fn stash_copy(w: *mut Ja1World, h: *const Ja1Host, c: [u8; ITEM]) {
    let _ = stash_item(w, h, &mut Item::Copy(c));
}

/// Merc leaves (`merc_leaves`).
pub(super) fn merc_leaves(w: *mut Ja1World, h: *const Ja1Host, slot: i32, status: u8) {
    let pb = sb(w, slot, PROFILE_ID) as i64 * PROFILE as i64;
    let pp = |w: *mut Ja1World, off: usize, v: u8| put8(unsafe { (*w).profiles }, PROFILES_LEN, pb + off as i64, v);
    let pg = |w: *mut Ja1World, off: usize| get8(unsafe { (*w).profiles }, PROFILES_LEN, pb + off as i64);
    pp(w, P_TEAM_STATUS, status);
    put16(unsafe { (*w).profiles }, PROFILES_LEN, pb + P_SALARY as i64, s16(w, slot, SALARY));
    for (s, p) in STATS {
        pp(w, p, sb(w, slot, s));
    }
    for i in 0..12usize {
        let b = place(slot, i);
        let t = get8(sol(w), SOLDIERS_LEN, b);
        let kit = if t == 0 || t == MONEY {
            0
        } else if i >= 10 || is_listed(w, t) || is_listed(w, get8(sol(w), SOLDIERS_LEN, b + 3)) {
            let mut c = [0u8; ITEM];
            for (j, x) in c.iter_mut().enumerate() {
                *x = get8(sol(w), SOLDIERS_LEN, b + j as i64);
            }
            let _ = stash_item(w, h, &mut Item::Copy(c));
            0
        } else {
            t
        };
        if i < 10 {
            pp(w, P_KIT + i, kit);
        }
    }
    let b3c = pg(w, P_B3C);
    if b3c != 99 && b3c == pg(w, P_B3A) {
        pp(w, P_B3A, 99);
    }
    hcall(w, h, op::FACE_CLEAR, &[sb(w, slot, SLOT) as i32]);
    sb_put(w, slot, NICKNAME, 0);
    sb_put(w, slot, IN_CUR_SECTOR, 0);
    sb_put(w, slot, PROFILE_ID, 99);
    sb_put(w, slot, B19D, 0);
}

/// Team shift (`team_shift`).
fn team_shift(w: *mut Ja1World, h: *const Ja1Host, to: i32, from: i32) {
    let sp = unsafe { (*w).stat_progress };
    for i in 0..PROGRESS_ROW as i64 {
        let v = get8(sp, PROGRESS_LEN, from as i64 * PROGRESS_ROW as i64 + i);
        put8(sp, PROGRESS_LEN, to as i64 * PROGRESS_ROW as i64 + i, v);
    }
    for i in 0..PROGRESS_ROW as i64 {
        put8(sp, PROGRESS_LEN, from as i64 * PROGRESS_ROW as i64 + i, 0);
    }
    for i in 0..REC {
        let v = sb(w, from, i);
        sb_put(w, to, i, v);
    }
    sb_put(w, to, SLOT, to as u8);
    hcall(w, h, op::FACE_MOVE, &[to & 0xFF, from & 0xFF]);
    s16_put(w, from, SALARY, 0);
    s16_put(w, from, SALARY2, 0);
    sb_put(w, from, DUTY, 0);
    sb_put(w, from, PROFILE_ID, 99);
    sb_put(w, from, NICKNAME, 0);
    for off in [IN_CUR_SECTOR, ENERGY, HP_MAX, WOUNDS, AGILITY, MARKSMANSHIP, HP, WISDOM] {
        sb_put(w, from, off, 0);
    }
    for i in 0..12 {
        Item::At(place(from, i)).clear(w, 6);
    }
    sb_put(w, from, SLOT, 99);
    sb_put(w, from, B19D, 0);
    hcall(w, h, op::FACE_CLEAR, &[from & 0xFF]);
}

/// Close up the team (`team_compact`).
pub(super) fn team_compact(w: *mut Ja1World, h: *const Ja1Host, redraw: u8) {
    for j in 0..7 {
        if sb(w, j, IN_CUR_SECTOR) == 0 {
            team_shift(w, h, j, j + 1);
        }
    }
    if redraw != 0 {
        hcall(w, h, op::TEAM_REDRAW, &[]);
    }
}

/// Pay the mercs (`pay_mercs`).
pub(super) fn pay_mercs(w: *mut Ja1World, h: *const Ja1Host) {
    let mut k = 0i32;
    while k < 8 && sb(w, k, IN_CUR_SECTOR) != 0 {
        let s19c = sb(w, k, B19C);
        let runs = sb(w, k, HP) != 0 && s19c != 1 && s19c != 7 && {
            let greed = get8(unsafe { (*w).greed }, GREED_LEN, sb(w, k, PROFILE_ID) as i64) as i32;
            cash_count(w, k, 0) >= greed.wrapping_mul(s16(w, k, SALARY) as i32)
        };
        if !runs {
            k += 1;
            continue;
        }
        unsafe { (*w).voice_test = 1 };
        hcall(w, h, op::RUNS_OFF, &[k, sb(w, k, SLOT) as i32, sb(w, k, PROFILE_ID) as i32]);
        merc_leaves(w, h, k, 0x0E);
        sb_put(w, k, PROFILE_ID, 99);
        sb_put(w, k, IN_CUR_SECTOR, 0);
        let acc = unsafe { (*w).accounts };
        put8(acc, ACCOUNTS_LEN, RESIGNATIONS, get8(acc, ACCOUNTS_LEN, RESIGNATIONS).wrapping_add(1));
        team_compact(w, h, 0);
        hcall(w, h, op::ISLAND_MAP, &[]);
        apply_day_ledger(w);
        hcall(w, h, op::RUNS_OFF_END, &[]);
    }
}

// SPEC GAP: a null world does nothing (0). A null host or soldier buffer skips the host calls (results 0).
fn run<T: Default>(w: *mut Ja1World, f: impl FnOnce() -> T) -> T {
    if w.is_null() {
        return T::default();
    }
    std::panic::catch_unwind(std::panic::AssertUnwindSafe(f)).unwrap_or_default()
}

#[no_mangle]
pub extern "C" fn ja1_cash_count(w: *mut Ja1World, _h: *const Ja1Host, slot: i32, drop: i32) -> i32 {
    run(w, || cash_count(w, slot, drop as u8))
}
#[no_mangle]
pub extern "C" fn ja1_collect_cash(w: *mut Ja1World, _h: *const Ja1Host) {
    run(w, || collect_cash(w))
}
#[no_mangle]
pub extern "C" fn ja1_stash_item(w: *mut Ja1World, h: *const Ja1Host, slot: i32, place_i: i32) {
    run(w, || { let _ = stash_item(w, h, &mut Item::At(place(slot, 0) + place_i as i64 * ITEM as i64)); })
}
#[no_mangle]
pub extern "C" fn ja1_merc_leaves(w: *mut Ja1World, h: *const Ja1Host, slot: i32, status: i32) {
    run(w, || merc_leaves(w, h, slot, status as u8))
}
#[no_mangle]
pub extern "C" fn ja1_team_compact(w: *mut Ja1World, h: *const Ja1Host, redraw: i32) {
    run(w, || team_compact(w, h, redraw as u8))
}
#[no_mangle]
pub extern "C" fn ja1_pay_mercs(w: *mut Ja1World, h: *const Ja1Host) {
    run(w, || pay_mercs(w, h))
}

#[cfg(test)]
mod tests {
    use super::*;

    struct T {
        w: Box<Ja1World>,
        acc: Vec<u8>,
        sol: Vec<u8>,
        prof: Vec<u8>,
        stash: Vec<u8>,
        prog: Vec<u8>,
        greed: Vec<u8>,
        it: Vec<u8>,
        listed: Vec<u8>,
    }
    fn world() -> T {
        let mut t = T {
            w: Box::new(unsafe { std::mem::zeroed() }),
            acc: vec![0; ACCOUNTS_LEN],
            sol: vec![0; SOLDIERS_LEN],
            prof: vec![0; PROFILES_LEN],
            stash: vec![0; STASH_LEN],
            prog: vec![0; PROGRESS_LEN],
            greed: vec![0; GREED_LEN],
            it: vec![0; ITEM_TYPES_LEN],
            listed: vec![0; LISTED_LEN],
        };
        t.w.accounts = t.acc.as_mut_ptr();
        t.w.soldiers = t.sol.as_mut_ptr();
        t.w.profiles = t.prof.as_mut_ptr();
        t.w.stash_counts = t.stash.as_mut_ptr();
        t.w.stat_progress = t.prog.as_mut_ptr();
        t.w.greed = t.greed.as_ptr();
        t.w.item_types = t.it.as_ptr();
        t.w.listed = t.listed.as_ptr();
        t
    }

    #[test]
    fn cash_reads_stack_and_drops() {
        let mut t = world();
        let w: *mut Ja1World = &mut *t.w;
        t.sol[IN_CUR_SECTOR] = 1;
        // place 0: money, 3 units: 5, 7, 2.
        t.sol[0..10].copy_from_slice(&[MONEY, 3, 5, 0, 0, 0, 0, 0, 7, 2]);
        // place 1: money, 1 unit of -1.
        t.sol[12..15].copy_from_slice(&[MONEY, 1, 0xFF]);
        assert_eq!(cash_count(w, 0, 0), 140 - 10);
        assert_eq!(t.sol[0], MONEY);
        assert_eq!(cash_count(w, 0, 1), 130);
        assert!(t.sol[0..24].iter().all(|&b| b == 0));
        assert_eq!(cash_count(w, 1, 1), 0);
    }

    #[test]
    fn compact_shifts_down() {
        let mut t = world();
        let w: *mut Ja1World = &mut *t.w;
        t.sol[REC + IN_CUR_SECTOR] = 1;
        t.sol[REC + HP] = 40;
        t.sol[REC + 0x20] = 0x55;
        t.prog[PROGRESS_ROW] = 9;
        team_compact(w, std::ptr::null(), 0);
        assert_eq!(t.sol[IN_CUR_SECTOR], 1);
        assert_eq!(t.sol[HP], 40);
        assert_eq!(t.sol[0x20], 0x55);
        assert_eq!(t.sol[SLOT], 0);
        assert_eq!(t.prog[0], 9);
        assert_eq!(t.prog[PROGRESS_ROW], 0);
        assert_eq!(t.sol[7 * REC + SLOT], 99);
        assert_eq!(t.sol[7 * REC + PROFILE_ID], 99);
    }

    #[test]
    fn leaves_copies_stats_and_kit() {
        let mut t = world();
        let w: *mut Ja1World = &mut *t.w;
        t.sol[PROFILE_ID] = 3;
        t.sol[HP_MAX] = 77;
        t.sol[LEVEL] = 4;
        t.sol[SALARY] = 0x34;
        t.sol[SALARY + 1] = 0x12;
        t.sol[0] = 5; // kit
        t.sol[12] = MONEY;
        t.sol[24] = 9; // listed -> store (no host: not stored), kit 0
        t.listed[0] = 9;
        t.prof[3 * PROFILE + P_B3A] = 7;
        t.prof[3 * PROFILE + P_B3C] = 7;
        merc_leaves(w, std::ptr::null(), 0, 0x0E);
        let p = &t.prof[3 * PROFILE..4 * PROFILE];
        assert_eq!(p[P_TEAM_STATUS], 0x0E);
        assert_eq!(p[0x2A], 77);
        assert_eq!(p[0x44], 4);
        assert_eq!(&p[P_SALARY..P_SALARY + 2], &[0x34, 0x12]);
        assert_eq!(&p[P_KIT..P_KIT + 3], &[5, 0, 0]);
        assert_eq!(p[P_B3A], 99);
        assert_eq!(t.sol[24], 9);
        assert_eq!(t.sol[PROFILE_ID], 99);
    }
}
