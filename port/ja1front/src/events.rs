//! Fight event log (#25 item 5; line format in docs/events.md): with `JA1_EVENTS=1` the front-end prints one
//! `ev ...` line on stderr per fight event. The log only reads the world: it compares the soldier records and the
//! sector table with the copy of the last poll (each host call, each random number, each session end). It never
//! writes the world and never draws a random number, so a run gives the same world with and without the variable.
//! The day clock lines (day, clock, battle, gained / lost, quit, wage, move; #25 comment 11052) come from the same
//! comparison and from the host ops 261, 332 and 335 that the library sends (`host_op`).
use crate::host::Ja1World;

const SLOTS: usize = 48;
const REC: usize = 0x200;
const SECTORS: usize = 80;
const OFF_MAP: u16 = 3136;

#[derive(Clone, Copy)]
struct Snap {
    tile: u16,
    hp_max: u8,
    wounds: u8,
    hp: u8,
    in_play: u8,
    shot: u8,
    target: u16,
    ap: u8,
    gun: u8,
    rounds: u8,
    sees: [u8; 40],
    wet: bool,
    /// Task tile (+0x134): the door effect clears it after the door opens, so the line reads the poll before.
    task: u16,
    /// The anim word (u16 at +0x1E8 + 2 x +0x1F3; cursor.md "Weapon cursor").
    pose: u16,
    /// The 12 inventory slots as (type, qty) pairs (#11186: a type's total rising = a pickup, falling = a loss).
    items: [u8; 24],
}

/// A shot from Launch (+0x1BF = 3) until its result: the target's health drops (the wound comes after Shot end),
/// or the shooter shoots again, the turn or the fight ends (a miss).
#[derive(Clone, Copy)]
struct Shot {
    by: usize,
    target: u16,
    victim: Option<usize>,
    hp: u8,
    ap: u8,
}

#[derive(Clone, Default)]
pub struct EvLog {
    pub on: bool,
    snap: Option<Vec<Snap>>,
    enemies: Vec<u8>,
    combat: u8,
    turn: i32,
    side: Option<bool>,
    shots: Vec<Shot>,
    hit_by: Vec<Option<usize>>,
    walk_from: Vec<Option<u16>>,
    dead: Vec<bool>,
    ap_mark: Vec<u8>,
    /// The day clock: the sector table at the last poll (9 bytes per sector), day, native wage, last printed clock.
    st: Vec<u8>,
    day: Option<i32>,
    wage: Option<i32>,
    clock: Option<(u8, u8)>,
    /// Enemies taken out of a sector at this clock minute (sector, count, day, minute): the next increases are moves.
    took: Vec<(usize, u8, i32, i32)>,
    /// A sector battle (op 332 (0) .. (8)): the sector table and the mines at its start; its sector (op 335).
    battle: Option<(Vec<u8>, Vec<u8>, Option<usize>)>,
    /// Guards who quit this morning (op 261 kinds 0 / 1) and the tappers of the table at that moment.
    quit: Option<(i32, u32, u64)>,
    polls: u64,
    st_ptr: usize,
    /// The owned bit of each sector as the last gained / lost line (or the first poll) gave it: a second world
    /// buffer that takes over a change gives no second line.
    owned: Vec<u8>,
    /// The first world buffer of the run: the day clock runs in it. The day end binds a second buffer that it fills
    /// from a save; its changes give no day clock lines.
    first_st: usize,
    /// The move codes and their sector at the last poll.
    codes: Vec<u8>,
    code_sector: Option<u8>,
}

/// The total number of items of each type in the 12 (type, qty) slots; type 0 is Nothing.
fn item_totals(items: &[u8; 24]) -> [u32; 256] {
    let mut t = [0u32; 256];
    for i in 0..12 {
        let (ty, q) = (items[2 * i] as usize, items[2 * i + 1] as u32);
        if ty != 0 {
            t[ty] += q.max(1);
        }
    }
    t
}

/// Where the event lines go: stderr with `JA1_EVENTS=1`, and events.log of the recorder (record.rs) with
/// `JA1_RECORD_DIR`.
static TO_STDERR: std::sync::atomic::AtomicBool = std::sync::atomic::AtomicBool::new(false);

macro_rules! ev_out {
    ($($a:tt)*) => {{
        let line = format!($($a)*);
        if TO_STDERR.load(std::sync::atomic::Ordering::Relaxed) {
            eprintln!("{}", line);
        }
        crate::record::event(&line);
    }};
}

/// "merc 2", "native 12" or "enemy 41".
/// The posture value of an anim word: the game's own table g_animPosture 0x9300D (256 bytes, Ja1World posture), the
/// only proved meaning of which is "6 and up can block a shot at another soldier" (ProjectileHitTest, aim.rs). Values in
/// the table: 2 (anims 0..3, 0x29, 0x2F, 0x30, 0x41..0x43), 3 (0x3A..0x3C, 0x40), 4 (5, 0xF), 5 (0x11), 0/1 (0x44..0x4D),
/// 6 the rest. No names (stand / crouch / lie): nothing proves them, and the game has no prone mode (the maintainer, Insert #12054).
/// An anim word of 256 and up reads 0, as in aim.rs.
fn posture(w: &Ja1World, anim: u16) -> u8 {
    if w.posture.is_null() || anim >= 256 {
        return 0;
    }
    unsafe { *w.posture.add(anim as usize) }
}

fn who(k: usize) -> String {
    match k {
        0..=7 => format!("merc {}", k),
        8..=39 => format!("native {}", k),
        _ => format!("enemy {}", k),
    }
}

impl EvLog {
    pub fn new() -> EvLog {
        let err = std::env::var("JA1_EVENTS").is_ok_and(|v| v == "1");
        TO_STDERR.store(err, std::sync::atomic::Ordering::Relaxed);
        let on = err || crate::record::on();
        ja1logic::trace::set_on(on);
        EvLog { on, ..Default::default() }
    }

    /// Compares the world `w` with the last poll and prints the events between them.
    ///
    /// # Safety
    /// `w` is null or a bound library world whose buffers have the sizes of world.rs.
    pub unsafe fn poll(&mut self, w: *const Ja1World) {
        if !self.on || w.is_null() || (*w).soldiers.is_null() || (*w).sector_state.is_null() {
            return;
        }
        let w = &*w;
        let sol = std::slice::from_raw_parts(w.soldiers as *const u8, SLOTS * REC);
        let st = std::slice::from_raw_parts(w.sector_state as *const u8, SECTORS * 9);
        let codes = if w.move_codes.is_null() { None } else { Some(std::slice::from_raw_parts(w.move_codes as *const u8, 2 * 3136)) };
        let water = |t: u16| {
            codes.is_some_and(|c| t < OFF_MAP && matches!(u16::from_le_bytes([c[2 * t as usize], c[2 * t as usize + 1]]), 5000 | 3600))
        };
        let cur: Vec<Snap> = (0..SLOTS)
            .map(|k| {
                let r = &sol[k * REC..(k + 1) * REC];
                let mut sees = [0; 40];
                sees.copy_from_slice(&r[0x13A..0x13A + 40]);
                let tile = u16::from_le_bytes([r[0x114], r[0x115]]);
                Snap {
                    tile,
                    hp: r[0x183],
                    hp_max: r[0x184],
                    wounds: r[0x185],
                    in_play: r[0x138],
                    shot: r[0x1BF],
                    target: u16::from_le_bytes([r[0x1E6], r[0x1E7]]),
                    ap: r[0x1A5],
                    gun: r[0],
                    rounds: r[1],
                    sees,
                    wet: water(tile),
                    task: u16::from_le_bytes([r[0x134], r[0x135]]),
                    pose: {
                        let a = 0x1E8 + 2 * r[0x1F3] as usize;
                        r.get(a..a + 2).map_or(0, |b| u16::from_le_bytes([b[0], b[1]]))
                    },
                    items: {
                        let mut it = [0u8; 24];
                        for i in 0..12 {
                            it[2 * i] = r[i * 12];
                            it[2 * i + 1] = r[i * 12 + 1];
                        }
                        it
                    },
                }
            })
            .collect();
        let enemies: Vec<u8> = (0..SECTORS).map(|s| st[s * 9 + 1]).collect();
        self.polls += 1;
        // Another world buffer (the day end and the morning bind a second copy): a new start, no lines.
        if self.st_ptr != w.sector_state as usize {
            self.st_ptr = w.sector_state as usize;
            self.snap = None;
        }
        if self.quit.is_some_and(|q| self.polls > q.2 + 1) {
            self.flush_quit();
        }
        let Some(prev) = self.snap.take() else {
            if self.owned.is_empty() {
                self.owned = (0..60).map(|s| st[s * 9] & 1).collect();
            }
            self.st = st.to_vec();
            self.day = self.day.or(Some(w.day));     // (a day that changes across a new buffer still gets its line)
            self.took.clear();
            self.ap_mark = cur.iter().map(|s| s.ap).collect();
            self.dead = cur.iter().map(|c| c.in_play == 0 || c.hp == 0).collect();
            self.snap = Some(cur);
            if self.enemies.is_empty() {
                self.enemies = enemies;
            }
            self.combat = w.combat;
            self.hit_by = vec![None; SLOTS];
            self.walk_from = vec![None; SLOTS];
            return;
        };
        // Fight start and end.
        if w.combat != self.combat {
            self.close_all(&cur);
            if w.combat != 0 {
                ev_out!("ev combat on sector {}", w.cur_sector);
            } else {
                ev_out!("ev combat off sector {}", w.cur_sector);
            }
            self.combat = w.combat;
            self.side = None;
        }
        self.interrupts_doors_stances(w, &prev, &cur, codes);
        // Turns: the side of the selected soldier.
        if w.combat != 0 {
            let side = match w.selected as usize {
                0..=7 => Some(true),
                40..=47 => Some(false),
                _ => None,
            };
            if side.is_some() && side != self.side {
                self.close_all(&cur);
                if side == Some(true) || self.side.is_none() {
                    self.turn += 1;
                }
                ev_out!("ev turn {} side {}", self.turn, if side == Some(true) { "merc" } else { "enemy" });
                self.side = side;
                for (m, s) in self.ap_mark.iter_mut().zip(cur.iter()) {
                    *m = s.ap;
                }
            }
        }
        // Shots: a new Launch closes the shooter's open shot.
        for k in 0..SLOTS {
            let (p, c) = (&prev[k], &cur[k]);
            if p.shot != 3 && c.shot == 3 {
                if let Some(i) = self.shots.iter().position(|s| s.by == k) {
                    let s = self.shots.remove(i);
                    self.result(&s, None, &cur);
                }
                let victim = (0..SLOTS).find(|&j| j != k && cur[j].in_play != 0 && cur[j].tile == c.target);
                let hp = victim.map_or(0, |v| prev[v].hp);
                self.shots.push(Shot { by: k, target: c.target, victim, hp, ap: self.ap_mark[k] });
            }
        }
        // Health: one line per change, so a reader keeps health, its total and the unbandaged wounds of each soldier without sums.
        for j in 0..SLOTS {
            let (p, c) = (&prev[j], &cur[j]);
            if (c.hp, c.hp_max, c.wounds) != (p.hp, p.hp_max, p.wounds) {
                ev_out!("ev health {} {} {} {}", who(j), c.hp, c.hp_max, c.wounds);
            }
        }
        // Wounds: the open shot at this soldier, else the newest open shot, else no shot (blasts, bleeding, blows).
        for j in 0..SLOTS {
            let (p, c) = (&prev[j], &cur[j]);
            if c.hp >= p.hp {
                continue;
            }
            if let Some(i) = self.shots.iter().position(|s| s.victim == Some(j)) {
                let s = self.shots.remove(i);
                self.hit_by[j] = Some(s.by);
                self.result(&s, Some(c.hp), &cur);
            } else if let Some(s) = self.shots.last() {
                self.hit_by[j] = Some(s.by);
                ev_out!("ev hit {} by {} hp {}->{}", who(j), who(s.by), p.hp, c.hp);
            } else if c.hp == 0 {
                // (#11186 item 2) a wound to 0 with no shot: the tile, so a death far away is found in the log.
                ev_out!("ev hurt {} hp {}->0 tile {}", who(j), p.hp, c.tile);
            } else {
                ev_out!("ev hurt {} hp {}->{}", who(j), p.hp, c.hp);
            }
        }
        for k in 0..SLOTS {
            let (p, c) = (&prev[k], &cur[k]);
            // Sightings: merc k of an enemy, enemy k of a merc.
            if c.in_play != 0 && c.tile < OFF_MAP {
                let range = if k < 8 { 40..48 } else if k >= 40 { 0..8 } else { 0..0 };
                for j in range {
                    let i = if k < 8 { j - 0x28 } else { j };
                    if p.sees[i] != 1 && c.sees[i] == 1 {
                        ev_out!("ev sees {} {}", who(k), who(j));
                    } else if p.sees[i] == 1 && c.sees[i] != 1 {
                        ev_out!("ev loses {} {}", who(k), who(j));
                    }
                }
            }
            // A new turn's AP: the base of the next shot line.
            if c.ap > p.ap {
                self.ap_mark[k] = c.ap;
            }
            // Rounds: the gun in the main hand is empty.
            if p.gun != 0 && c.gun == p.gun && p.rounds > 0 && c.rounds == 0 {
                ev_out!("ev ammo {} out item {}", who(k), c.gun);
            }
            if p.gun != 0 && c.gun == p.gun && p.rounds == 0 && c.rounds > 0 {
                ev_out!("ev reload {} rounds {}", who(k), c.rounds);
            }
            // Walks: from the first step to the end of the turn.
            if p.tile != c.tile && p.tile < OFF_MAP && c.tile < OFF_MAP && self.walk_from[k].is_none() {
                self.walk_from[k] = Some(p.tile);
            }
            // (one line per death: the in-play byte and the health can drop to 0 in different polls)
            if c.in_play != 0 && c.hp > 0 {
                self.dead[k] = false;
            }
            // (#11186 item 3) a type's total in a merc's record rising = an item arrived (ground or stash).
            if k < 8 && c.in_play != 0 && c.items != p.items {
                let (pt, ct) = (item_totals(&p.items), item_totals(&c.items));
                for ty in 1..256 {
                    if ct[ty] > pt[ty] {
                        ev_out!("ev pickup {} item {} tile {}", who(k), ty, c.tile);
                    }
                }
            }
            if ((p.in_play != 0 && c.in_play == 0) || (p.hp > 0 && c.hp == 0)) && !self.dead[k] {
                self.dead[k] = true;
                match self.hit_by[k] {
                    Some(a) => ev_out!("ev kill {} by {} tile {}", who(k), who(a), c.tile),
                    None => ev_out!("ev kill {} tile {}", who(k), c.tile),
                }
                // (#11186 item 3) the items the dead soldier's record held at the poll before it died.
                for i in 0..12 {
                    let ty = p.items[2 * i];
                    if ty != 0 {
                        ev_out!("ev drop {} item {} tile {}", who(k), ty, p.tile);
                    }
                }
                self.hit_by[k] = None;
            }
        }
        self.day_clock(w, st, &enemies);
        self.snap = Some(cur);
    }

    /// `ev interrupt`, `ev door` and `ev stance` (docs/events.md). All three read the world only.
    fn interrupts_doors_stances(&mut self, w: &Ja1World, prev: &[Snap], cur: &[Snap], codes: Option<&[u8]>) {
        // Interrupts: each contest that a soldier won since the last poll (a won contest can come and go between two polls).
        for (a, sel) in ja1logic::trace::take_grants() {
            let a = a as usize;
            if a < SLOTS {
                ev_out!("ev interrupt {} during {} tile {}", who(a), who(sel as usize), cur[a].tile);
            }
        }
        // Doors: move code 1500 (closed) -> 25400 (open) at a tile (combat.md "Doors"); a new sector changes many tiles, so it gives no line.
        if let Some(c) = codes {
            let same = self.code_sector == Some(w.cur_sector) && self.codes.len() == c.len();
            if same && self.codes != c {
                let opened: Vec<usize> = (0..c.len() / 2)
                    .filter(|&t| {
                        let (a, b) = (&self.codes[2 * t..2 * t + 2], &c[2 * t..2 * t + 2]);
                        u16::from_le_bytes([a[0], a[1]]) == 1500 && u16::from_le_bytes([b[0], b[1]]) == 25400
                    })
                    .collect();
                if opened.len() <= 4 {
                    for t in opened {
                        let beside = |k: usize| {
                            let (a, b) = (cur[k].tile as i32, t as i32);
                            (a / 56 - b / 56).abs() <= 2 && (a % 56 - b % 56).abs() <= 2
                        };
                        let sel = w.selected as usize;
                        let by = (0..SLOTS).find(|&k| prev[k].task == t as u16).or((sel < SLOTS && beside(sel)).then_some(sel));
                        match by {
                            Some(k) => ev_out!("ev door {} tile {} open", who(k), t),
                            None => ev_out!("ev door tile {} open", t),
                        }
                    }
                }
            }
            if !same || self.codes != c {
                self.codes = c.to_vec();
                self.code_sector = Some(w.cur_sector);
            }
        }
        // Stances: a change of a living soldier on the map, in a fight.
        if w.combat != 0 {
            for k in (0..8).chain(40..SLOTS) {
                let (p, c) = (&prev[k], &cur[k]);
                if posture(w, p.pose) != posture(w, c.pose) && c.in_play != 0 && c.hp > 0 && c.tile < OFF_MAP {
                    ev_out!("ev stance {} posture {}", who(k), posture(w, c.pose));
                }
            }
        }
    }

    /// `ev clock H:MM` before the first day clock line of a new clock minute.
    fn stamp(&mut self, w: &Ja1World) {
        if self.clock != Some((w.clock_hour, w.clock_minute)) {
            self.clock = Some((w.clock_hour, w.clock_minute));
            ev_out!("ev clock {}:{:02}", w.clock_hour, w.clock_minute);
        }
    }

    /// The day clock lines between two polls: day, wage, gained / lost, enemy moves and waves.
    fn day_clock(&mut self, w: &Ja1World, st: &[u8], enemies: &[u8]) {
        // SAFETY: `accounts` is null or the world's Day accounts (more than 0x58 bytes).
        let wage = if w.accounts.is_null() { None } else { Some(unsafe { (w.accounts.add(0x54) as *const i32).read_unaligned() }) };
        if self.day.is_some_and(|d| d != w.day) {
            ev_out!("ev day {}", w.day);
            self.clock = None;
            self.took.clear();
        }
        self.day = Some(w.day);
        let now = (w.day, w.day_minutes);
        if self.first_st == 0 {
            self.first_st = w.sector_state as usize;
        }
        if self.first_st != w.sector_state as usize {
            self.wage = wage;
            self.st = st.to_vec();
            self.took.clear();
            return;
        }
        if self.wage.is_some() && wage.is_some() && wage != self.wage {
            self.stamp(w);
            ev_out!("ev wage {}", wage.unwrap());
        }
        self.wage = wage;
        let prev = std::mem::replace(&mut self.st, st.to_vec());
        if prev.len() == st.len() {
            for s in 0..60 {
                let (a, b) = (prev[s * 9] & 1, st[s * 9] & 1);
                if a != b && self.owned[s] != b {
                    self.owned[s] = b;
                    self.stamp(w);
                    ev_out!("ev {} sector {}", if b != 0 { "gained" } else { "lost" }, s);
                }
            }
        }
        // Enemy moves (op 311): a take and the puts of the same clock minute; an increase with no take is a wave.
        // Losses in a fight or a sector battle are no takes.
        let calm = w.combat == 0 && self.battle.is_none();
        self.took.retain(|t| (t.2, t.3) == now);
        // (records 0..60: the island and Santino's sector 60; the other records are no sectors.) A count that
        // changed in this buffer, against the count of the last line (a buffer filled with a copy gives none).
        let mut ups = Vec::new();
        for s in 0..61 {
            let (a, b) = (self.enemies[s], enemies[s]);
            if prev.len() != st.len() || prev[s * 9 + 1] == b || a == b {
                continue;
            }
            self.enemies[s] = b;
            if b < a && calm {
                self.took.push((s, a - b, now.0, now.1));
            } else if b > a {
                ups.push((s, a, b));
            }
        }
        for (s, a, b) in ups {
            {
                self.stamp(w);
                match self.took.iter_mut().find(|t| t.1 > 0) {
                    Some(t) => {
                        let n = (b - a).min(t.1);
                        t.1 -= n;
                        ev_out!("ev move src {} -> {} enemies {}", t.0, s, n);
                    }
                    None => ev_out!("ev wave sector {} enemies {}", s, b),
                }
            }
        }
    }

    /// The host ops of the day clock that the log reads (called before the poll of the same call): 261 natives_quit_msg,
    /// 332 battle_ui (0 = a sector battle starts, 8 = it ends) and 335 battle_msg (its sector).
    ///
    /// # Safety
    /// As in `poll`; `args` holds `nargs` values.
    pub unsafe fn host_op(&mut self, w: *const Ja1World, op: i32, args: *const i32, nargs: i32) {
        if !self.on || w.is_null() || (*w).sector_state.is_null() || nargs < 2 {
            return;
        }
        let w = &*w;
        let st = std::slice::from_raw_parts(w.sector_state as *const u8, SECTORS * 9);
        let (a0, a1) = (*args, *args.add(1));
        let taps = |st: &[u8]| (0..60).map(|s| st[s * 9 + 3] as u32).sum::<u32>();
        let mines = |s: usize| if w.sectors.is_null() { 0 } else { *w.sectors.add(s * 0xB1 + 0xAF) };
        match op {
            261 if a1 <= 1 => {
                self.flush_quit();
                self.quit = Some((a0, taps(st), self.polls));
            }
            261 => {
                let (g, before) = self.quit.take().map(|q| (q.0, q.1)).unwrap_or((0, if self.st.len() == st.len() { taps(&self.st) } else { taps(st) }));
                self.stamp(w);
                ev_out!("ev quit guards {} tappers {}", g, before.saturating_sub(taps(st)));
            }
            332 if a0 == 0 => {
                self.battle = Some((st.to_vec(), (0..60).map(mines).collect(), None));
            }
            335 if a0 != 0 => {
                if let Some(b) = self.battle.as_mut() {
                    let s = (a1 - 1).clamp(0, 59) as usize;
                    b.2 = Some(s);
                    let t = &b.0[s * 9..s * 9 + 9];
                    let line = format!("ev battle sector {} guards {} tappers {} enemies {} mines {}", s, t[4], t[3], t[1], b.1[s]);
                    self.stamp(w);
                    ev_out!("{}", line);
                }
            }
            332 if a0 == 8 => {
                if let Some((_, _, Some(s))) = self.battle.take() {
                    let t = &st[s * 9..s * 9 + 9];
                    ev_out!("ev battle end sector {} guards {} tappers {} enemies {} owned {}", s, t[4], t[3], t[1], t[0] & 1);
                }
            }
            _ => {}
        }
    }

    /// Guards who quit with no tapper message after them.
    fn flush_quit(&mut self) {
        if let Some((g, _, _)) = self.quit.take() {
            ev_out!("ev quit guards {} tappers 0", g);
        }
    }

    /// The line of shot `s`: a hit (the target's health now `hp`) or a miss; the shooter's AP before and now.
    fn result(&mut self, s: &Shot, hp: Option<u8>, cur: &[Snap]) {
        let mut line = format!("ev shot {} -> ", who(s.by));
        match (s.victim, hp) {
            (Some(v), Some(h)) => line += &format!("{} hit {} hp {}->{}", who(v), s.hp.saturating_sub(h), s.hp, h),
            (Some(v), None) => line += &format!("{} miss", who(v)),
            (None, _) => line += &format!("tile {} miss", s.target),
        }
        ev_out!("{} ap {}->{}", line, s.ap, cur[s.by].ap);
        self.ap_mark[s.by] = cur[s.by].ap;
    }

    /// The end of a turn or a fight: the open shots are misses; the walk lines of the turn.
    fn close_all(&mut self, cur: &[Snap]) {
        for s in std::mem::take(&mut self.shots) {
            self.result(&s, None, cur);
        }
        for k in 0..SLOTS {
            if let Some(f) = self.walk_from[k].take() {
                let t = cur[k].tile;
                if t != f && t < OFF_MAP {
                    ev_out!("ev walk {} {} -> {}{}", who(k), f, t, if cur[k].wet { " water" } else { "" });
                }
            }
        }
    }

    /// The open shots and walks at the end of a run.
    pub fn flush(&mut self) {
        self.flush_quit();
        if let Some(cur) = self.snap.clone() {
            self.close_all(&cur);
        }
    }
}
