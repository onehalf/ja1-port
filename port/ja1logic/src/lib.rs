//! ja1logic: clean-room port. Slice 1 (here): move codes, step AP, path search. Slice 2a: `ai` (enemy AI decisions).
//! Slice 2b: `ai::search` (the search procedures, library ops of port_api.md).
pub mod ai;
pub mod sector;
pub mod trace;
use std::panic::catch_unwind;
use std::slice;

const W: i32 = 56;
const N: usize = 3136;
const DOOR: u16 = 25300; // 0x62D4
const OPEN_DOOR: u16 = 25400; // 0x6338
const WALL: u16 = 25500; // 0x639C

/// Terrain move code of one tile from its layers.
pub fn move_code(ground: u8, object: u8, roof: u8, previous: u16, gc: &[u8; 256], oc: &[u8; 256]) -> u16 {
    let code = if object != 0 {
        match oc[object as usize] {
            1 => 2000,
            2 | 3 => 25400,
            4 | 5 => {
                if matches!(object, 0x21 | 0x23 | 0x24 | 0x26) { 25300 } else { 25500 }
            }
            _ => {
                if matches!(object, 0x3D | 0x3E | 0x43 | 0x45 | 0x5C | 0x5D | 0x62 | 0x64) {
                    1500
                } else if roof == 0x73 {
                    1201
                } else {
                    25500
                }
            }
        }
    } else {
        match gc[ground as usize] {
            1 | 4 | 5 => 1200,
            2 | 3 => 1000,
            6 | 7 => 1600,
            8 => if ground < 0x80 { 3000 } else { 3600 },
            9 => if ground > 0x96 { 5000 } else { 3600 },
            // class 0 keeps the previous code.
            // SPEC GAP: classes 10..255 are not listed; treated like class 0 (keep previous).
            _ => previous,
        }
    };
    // A tile that had the door code never gets a lower code.
    // SPEC GAP: applies to both branches (object and ground); higher codes (25400, 25500) may replace it.
    if previous == DOOR && code < DOOR { DOOR } else { code }
}

// Neighbour step, direction 1 = north .. 8 = north-west. Off the map -> stays on the tile.
fn nb(t: usize, d: u8) -> usize {
    let (x, y) = ((t as i32) % W, (t as i32) / W);
    let (dx, dy) = match d {
        1 => (0, -1), 2 => (1, -1), 3 => (1, 0), 4 => (1, 1),
        5 => (0, 1), 6 => (-1, 1), 7 => (-1, 0), 8 => (-1, -1),
        _ => (0, 0),
    };
    let (nx, ny) = (x + dx, y + dy);
    if nx < 0 || ny < 0 || nx >= W || ny >= W { t } else { (ny * W + nx) as usize }
}

fn door_marks(obj: u8) -> &'static [u8] {
    match obj {
        0x1B => &[7, 8, 6],
        0x1D => &[3, 2, 4],
        0x1E => &[1, 8, 2],
        0x20 => &[5, 6, 4],
        0x3B | 0x5A | 0x3D | 0x3E | 0x5C | 0x5D => &[1, 8, 2, 5, 6],
        0x41 | 0x60 | 0x43 | 0x45 | 0x62 | 0x64 => &[7, 8, 6, 3, 2],
        _ => &[],
    }
}

pub fn rebuild(tiles: &[u8], gc: &[u8; 256], oc: &[u8; 256], out: &mut [u16]) {
    for c in out.iter_mut() { *c = 0; }
    for t in 0..N {
        let b = &tiles[t * 8..t * 8 + 8];
        out[t] = move_code(b[0], b[1], b[3], 0, gc, oc);
    }
    // SPEC GAP: the door pass is one pass in tile order; a tile first sets its own 25400 (open doors),
    // then marks its neighbours, before the next tile is looked at.
    for t in 0..N {
        let obj = tiles[t * 8 + 1];
        if matches!(obj, 0x3B | 0x5A | 0x41 | 0x60) {
            out[t] = OPEN_DOOR;
        }
        for &d in door_marks(obj) {
            let n = nb(t, d);
            // SPEC GAP: a neighbour already 25300/25400 is overwritten with 25300; a step off the map marks the tile itself.
            if !matches!(out[n], 25500 | 1201 | 1500) {
                out[n] = DOOR;
            }
        }
    }
}

pub fn step_ap(code: u16, sneak: u8, b177: u8, alt: u8) -> i32 {
    if code == WALL { return 100; }
    // SPEC GAP: any nonzero sneak/alt/byte177 counts as set.
    let base: i32 = (if sneak != 0 { 2 } else if b177 != 0 { 3 } else { 0 }) + (if alt != 0 { 1 } else { 0 });
    let extra = match code {
        1000 | 1500 | 25300 | 25400 => 2,
        1200 => 3,
        1201 | 1600 => 4,
        2000 => 5,
        3000 => 6,
        5000 => 7,
        3600 => 8,
        _ => 0,
    };
    (base + extra) as i8 as i32
}

#[derive(Clone, Copy)]
struct Cand { tile: u16, g: u32, f: u32, rec: i32 }

pub struct PathIn<'a> {
    pub codes: &'a [u16], pub ground: &'a [u8], pub occ: &'a [u8],
    pub b177: u8, pub bb9: u8, pub slot: u8, pub combat: u8,
}

/// Returns the directions of the full path (not cut), or None ("no path").
pub fn find_path(start: usize, dest: usize, p: &PathIn) -> Option<Vec<u8>> {
    use std::collections::VecDeque;
    // SPEC GAP: a start or destination outside the map is "no path" (the text only covers tiles on the map).
    if start >= N || dest >= N { return None; }
    let (dx_, dy_) = ((dest as i32) % W, (dest as i32) / W);
    let dist = |t: usize| -> (u32, u32) {
        let (x, y) = ((t as i32) % W, (t as i32) / W);
        ((x - dx_).unsigned_abs(), (y - dy_).unsigned_abs())
    };
    let est = |t: usize| -> u32 { let (a, b) = dist(t); a.max(b) + (a.min(b) * 14) / 10 };
    let manh = |t: usize| -> u32 { let (a, b) = dist(t); a + b };
    const POOL: usize = 510;
    const MAX_RECORDS: usize = 4094;
    let mut pool: Vec<Cand> = Vec::with_capacity(POOL);
    let mut open: Vec<usize> = Vec::new(); // pool indices, front = first
    let mut finished: VecDeque<usize> = VecDeque::new();
    let mut recs: Vec<(u8, i32)> = Vec::new(); // (direction, previous record); the start has none
    let mut best = vec![u32::MAX; N];
    best[start] = 0;
    pool.push(Cand { tile: start as u16, g: 0, f: est(start), rec: -1 });
    open.push(0);
    let both_deep = matches!(p.codes[start], 3600 | 5000) && matches!(p.codes[dest], 3600 | 5000);
    let offs: [(i32, i32); 8] = [(0, -1), (1, -1), (1, 0), (1, 1), (0, 1), (-1, 1), (-1, 0), (-1, -1)];
    loop {
        let ci = open.remove(0);
        finished.push_back(ci);
        let mut cur = pool[ci];
        let ct = cur.tile as usize;
        if cur.g <= best[ct] {
            let (cx, cy) = ((ct as i32) % W, (ct as i32) / W);
            let from = p.codes[ct];
            for (d, (ox, oy)) in offs.iter().enumerate() {
                let (nx, ny) = (cx + ox, cy + oy);
                if nx < 0 || ny < 0 || nx >= W || ny >= W { continue; }
                let n = (ny * W + nx) as usize;
                let is_dest = n == dest;
                let mut code = p.codes[n];
                // 1. occupied
                if p.occ[n] < 0x40 && !is_dest {
                    if p.combat != 0 { continue; }
                    code = 20000;
                } else if p.b177 != 0 && code < 3000 {
                    // 2.
                    code = 1000;
                }
                // 3.
                if code == 1500 && !(is_dest || (from == DOOR && p.bb9 != 0)) { continue; }
                // 4.
                if code == WALL { continue; }
                if code == 1201 && p.slot >= 8 { continue; }
                // 5.
                if from == OPEN_DOOR && !matches!(code, OPEN_DOOR | DOOR) { continue; }
                if code == OPEN_DOOR && !matches!(from, OPEN_DOOR | DOOR) { continue; }
                // 6.
                let mut cost: u32 = match code {
                    OPEN_DOOR => 1000,
                    DOOR => if p.ground[n] < 0x6C { 1000 } else { 6000 },
                    c => c as u32,
                };
                // 7.
                if is_dest { cost = 0; }
                // 8.
                if both_deep && matches!(cost, 3600 | 5000) { cost = 500; }
                // 9.
                if d % 2 == 1 { cost += 1; }
                // SPEC GAP: overflow of the cost sum is not described; saturating u32 addition.
                let g = cur.g.saturating_add(cost);
                if g >= best[n] { continue; }
                if recs.len() >= MAX_RECORDS { return None; }
                let idx = if pool.len() < POOL {
                    pool.push(Cand { tile: 0, g: 0, f: 0, rec: -1 });
                    pool.len() - 1
                } else {
                    match finished.pop_front() { Some(i) => i, None => return None }
                };
                recs.push((d as u8, cur.rec));
                let f = g.saturating_add(est(n));
                pool[idx] = Cand { tile: n as u16, g, f, rec: (recs.len() - 1) as i32 };
                best[n] = g;
                let m = manh(n);
                // The reused candidate may be `ci` itself: tile and cost stay, the step record becomes the new one.
                if idx == ci { cur.rec = (recs.len() - 1) as i32; }
                let pos = open.iter().position(|&o| {
                    let c = &pool[o];
                    c.f > f || (c.f == f && manh(c.tile as usize) >= m)
                }).unwrap_or(open.len());
                open.insert(pos, idx);
            }
        }
        match open.first() {
            None => return if start == dest { Some(vec![]) } else { None },
            Some(&o) if pool[o].tile as usize == dest => {
                let mut dirs = vec![];
                let mut r = pool[o].rec;
                while r >= 0 {
                    dirs.push(recs[r as usize].0);
                    r = recs[r as usize].1;
                }
                dirs.reverse();
                return Some(dirs);
            }
            _ => {}
        }
    }
}

#[no_mangle]
pub extern "C" fn ja1_move_code(ground: u8, object: u8, roof: u8, previous: u16, ground_class: *const u8, object_class: *const u8) -> u16 {
    if ground_class.is_null() || object_class.is_null() { return previous; }
    catch_unwind(|| {
        let gc = unsafe { &*(ground_class as *const [u8; 256]) };
        let oc = unsafe { &*(object_class as *const [u8; 256]) };
        move_code(ground, object, roof, previous, gc, oc)
    }).unwrap_or(previous)
}

#[no_mangle]
pub extern "C" fn ja1_rebuild_move_codes(tiles: *const u8, ground_class: *const u8, object_class: *const u8, move_codes: *mut u16) {
    if tiles.is_null() || ground_class.is_null() || object_class.is_null() || move_codes.is_null() { return; }
    let _ = catch_unwind(|| {
        let t = unsafe { slice::from_raw_parts(tiles, N * 8) };
        let gc = unsafe { &*(ground_class as *const [u8; 256]) };
        let oc = unsafe { &*(object_class as *const [u8; 256]) };
        let out = unsafe { slice::from_raw_parts_mut(move_codes, N) };
        rebuild(t, gc, oc, out);
    });
}

#[no_mangle]
pub extern "C" fn ja1_step_ap(move_code: u16, sneak: u8, byte177: u8, alt: u8) -> i32 {
    step_ap(move_code, sneak, byte177, alt)
}

#[no_mangle]
pub extern "C" fn ja1_find_path(start: i32, dest: i32, move_codes: *const u16, ground: *const u8, occupant: *const u8,
    byte177: u8, byte_b9: u8, slot: u8, combat: u8, dirs_out: *mut u8, count_out: *mut u16) -> i32 {
    // "no path": dirs_out and *count_out stay untouched.
    if move_codes.is_null() || ground.is_null() || occupant.is_null() || start < 0 || dest < 0 { return 0; }
    let r = catch_unwind(|| {
        let p = PathIn {
            codes: unsafe { slice::from_raw_parts(move_codes, N) },
            ground: unsafe { slice::from_raw_parts(ground, N) },
            occ: unsafe { slice::from_raw_parts(occupant, N) },
            b177: byte177, bb9: byte_b9, slot, combat,
        };
        find_path(start as usize, dest as usize, &p)
    });
    match r {
        Ok(Some(d)) => {
            let n = d.len().min(80);
            // SPEC GAP: bytes of dirs_out beyond the kept steps are left unchanged (not zeroed).
            if !dirs_out.is_null() { unsafe { slice::from_raw_parts_mut(dirs_out, 80)[..n].copy_from_slice(&d[..n]); } }
            if !count_out.is_null() { unsafe { *count_out = n as u16; } }
            1
        }
        _ => 0,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    /// Port rule: a null move-code, ground or occupant table is "no path" (0), and the outputs stay untouched.
    #[test]
    fn guard_find_path_null_tables() {
        let codes = vec![1000u16; N];
        let ground = vec![0u8; N];
        let occ = vec![0u8; N];
        let mut dirs = [7u8; 80];
        let mut count = 9u16;
        for (c, g, o) in [(std::ptr::null(), ground.as_ptr(), occ.as_ptr()), (codes.as_ptr(), std::ptr::null(), occ.as_ptr()),
                          (codes.as_ptr(), ground.as_ptr(), std::ptr::null())] {
            assert_eq!(ja1_find_path(100, 102, c, g, o, 0, 0, 0, 0, dirs.as_mut_ptr(), &mut count), 0);
        }
        assert_eq!((dirs, count), ([7u8; 80], 9));
        assert_ne!(ja1_find_path(100, 102, codes.as_ptr(), ground.as_ptr(), occ.as_ptr(), 0, 0, 0, 0, dirs.as_mut_ptr(), &mut count), 0);
    }
    #[test]
    fn wall_costs_100() {
        assert_eq!(step_ap(25500, 1, 1, 1), 100);
        assert_eq!(step_ap(1200, 0, 0, 0), 3);
        assert_eq!(step_ap(1200, 1, 0, 1), 6);
    }
    #[test]
    fn codes() {
        let mut gc = [0u8; 256]; gc[5] = 2; let mut oc = [0u8; 256]; oc[9] = 1;
        assert_eq!(move_code(5, 0, 0, 0, &gc, &oc), 1000);
        assert_eq!(move_code(5, 9, 0, 0, &gc, &oc), 2000);
        assert_eq!(move_code(0, 0, 0, 777, &gc, &oc), 777);
        assert_eq!(move_code(5, 0, 0, 25300, &gc, &oc), 25300);
    }
    #[test]
    fn open_field_path() {
        let codes = vec![1200u16; N]; let ground = vec![0u8; N]; let occ = vec![64u8; N];
        let p = PathIn { codes: &codes, ground: &ground, occ: &occ, b177: 0, bb9: 0, slot: 0, combat: 0 };
        let d = find_path(0, 3, &p).unwrap();
        assert_eq!(d, vec![2, 2, 2]);
        let d = find_path(0, 114, &p).unwrap();
        assert_eq!(d, vec![3, 3]);
    }
    #[test]
    fn start_equals_dest_found_zero_steps() {
        let codes = vec![1200u16; N]; let ground = vec![0u8; N]; let occ = vec![64u8; N];
        let p = PathIn { codes: &codes, ground: &ground, occ: &occ, b177: 0, bb9: 0, slot: 0, combat: 0 };
        assert_eq!(find_path(5, 5, &p), Some(vec![]));
    }
    #[test]
    fn wall_blocks() {
        let mut codes = vec![1200u16; N]; let ground = vec![0u8; N]; let occ = vec![64u8; N];
        for y in 0..56 { codes[y * 56 + 1] = 25500; }
        let p = PathIn { codes: &codes, ground: &ground, occ: &occ, b177: 0, bb9: 0, slot: 0, combat: 0 };
        assert!(find_path(0, 3, &p).is_none());
    }
}
