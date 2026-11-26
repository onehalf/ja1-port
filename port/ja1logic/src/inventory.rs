//! The inventory screen (docs/spec/inventory.md; C interface in docs/spec/port_api.md "Inventory screen entry
//! point"): `ja1_inventory_screen`. A child module of `ai`: every op goes through `Ai::call` (the host_ops rule;
//! spend_ap 31 and reserve_ap 58 are library ops). World data is read again after every host call; the only
//! value kept across calls is the screen's armour value (a local of the loop, not world data).
//! Every poll ("poll", "wait up") is op 221 inv_poll; when it returns 1 the screen returns 1 at once.
use super::{op, Ai, Ja1Host, Ja1World, ITEM_TYPES, NO_SLOT, REC, SLOTS, TILES};

const INV_POLL: i32 = 221;
const KEY_PRESSED: i32 = 222;
const GET_KEY: i32 = 223;
const MOUSE_SHOW: i32 = 224;
const CURSOR_PIC: i32 = 225;
const CURSOR_PIC2: i32 = 226;
const INV_CLICK: i32 = 227;
const INV_BEGIN: i32 = 228;
const INV_END: i32 = 229;
const ITEM_VALUE: i32 = 230;
const ITEM_INFO: i32 = 231;
const INV_BODY: i32 = 232;
const TIP_ITEM: i32 = 233;
const TIP_MERC: i32 = 234;
const DROP_ANSWER: i32 = 235;
const TEXT_TOGGLE: i32 = 236;
const INV_EXTRA: i32 = 237;
const DRAW_SLOT: i32 = 238;
const VESTMAP: i32 = 239;
const REST: i32 = 241;
const INV_SELECT: i32 = 242;
const INV_LEAVE_MERC: i32 = 243;
const INV_MOVED: i32 = 244;
const INV_VEST_BEGIN: i32 = 245;
const INV_VEST_END: i32 = 246;
const MINE_SPOTTED: i32 = 247;
const DISARM: i32 = 248;
const PICK_UP: i32 = 249;
const INV_PUT_ITEM: i32 = 250;
const MERGE_PROMPT: i32 = 251;
const MERGE_QUOTE: i32 = 252;
const MERGE_YES_NO: i32 = 253;

// inv_extra sounds / texts (port_api.md op 237)
const SND_EMPTY: i32 = 1;
const SND_HMM: i32 = 2;
const SND_HUH: i32 = 3;
const SND_NOROOM: i32 = 4;
const TXT_NONE: i32 = 0;
const TXT_EMPTY: i32 = 1;
const TXT_DRANK: i32 = 2;
const TXT_OK: i32 = 3;
const TXT_NOROOM: i32 = 4;
// message texts (op 125)
const MSG_PUT_DOWN: i32 = 7;
const MSG_ONE_DRINK: i32 = 8;
const MSG_ONE_KIT: i32 = 9;
const MSG_VEST_NOT_EMPTY: i32 = 10;

// soldier record offsets (defs/types.toml struct.soldier; inventory.md)
const S_ITEMS: usize = 0x000;
const S_VEST: usize = 0x048; // entry 6 type
const S_B4: usize = 0x0B4;
const S_D0: usize = 0x0D0;
const S_TILE: usize = 0x114;
const S_130: usize = 0x130;
const S_SLOT: usize = 0x17A;
const S_PROFILE: usize = 0x179;
const S_HP: usize = 0x183;
const S_ENERGY: usize = 0x18A;
const S_DEX: usize = 0x18C;
const S_EXPLOSIVES: usize = 0x18F;
const S_MECHANICAL: usize = 0x190;
const S_LEVEL: usize = 0x192;
const S_AP_LEFT: usize = 0x1A5;
const S_CAMO: usize = 0x1B1;
const S_OUT: usize = 0x1BB;
const S_SKILL_BONUS: usize = 0x1D0;
const S_1D5: usize = 0x1D5;
const S_AP_RESERVE: usize = 0x1E2;

const SECTOR_ROWS: usize = 60;
const SECTOR_ROW: usize = 0xB1;
const WEAPONS: usize = 256;
const MERGE_ROWS: usize = 82;

const MONEY: u8 = 0x6F;

/// op 221 said stop: the screen returns 1 at once.
struct Stop;
type R<T> = Result<T, Stop>;

/// How one poll of the loop ends.
#[derive(PartialEq)]
enum Flow {
    Next,
    Leave,
}

/// An item entry: the cursor, the swap item, or entry `e` of soldier `s` (bytes at +12 e of the record).
#[derive(Clone, Copy)]
enum Loc {
    Cursor,
    Swap,
    Sol(usize, usize),
}

struct Inv {
    w: *mut Ja1World,
    h: *const Ja1Host,
    a: Ai,
}

fn in_rect(x: i32, y: i32, x1: i32, y1: i32, x2: i32, y2: i32) -> bool {
    x1 <= x && x <= x2 && y1 <= y && y <= y2
}

fn rd(p: *const u8, len: usize, i: usize) -> u8 {
    if p.is_null() || i >= len {
        0
    } else {
        unsafe { p.add(i).read() }
    }
}
fn wr(p: *mut u8, len: usize, i: usize, v: u8) {
    if !p.is_null() && i < len {
        unsafe { p.add(i).write(v) }
    }
}

impl Inv {
    // ---- host ----
    fn call(&self, opc: i32, args: &[i32]) -> u32 {
        self.a.call(opc, args)
    }
    fn random(&self, n: i32) -> i32 {
        self.a.random(n)
    }
    fn wd(&self) -> &mut Ja1World {
        unsafe { &mut *self.w }
    }
    /// One poll (op 221); then shift / ctrl from the keyboard flags.
    fn poll(&self) -> R<()> {
        self.poll_raw()?;
        let w = self.wd();
        w.key_shift = (w.key_flags & 3 != 0) as u8;
        w.key_ctrl = (w.key_flags & 0x104 != 0) as u8;
        Ok(())
    }
    /// One poll (op 221) that leaves shift / ctrl alone (the polls inside a wait).
    fn poll_raw(&self) -> R<()> {
        if self.call(INV_POLL, &[]) == 1 {
            return Err(Stop);
        }
        Ok(())
    }
    /// Poll at least once, then again while the mouse button `bit` is down.
    fn wait_btn(&self, bit: u8) -> R<()> {
        loop {
            self.poll_raw()?;
            if self.wd().mouse_buttons & bit == 0 {
                return Ok(());
            }
        }
    }
    fn wait_up(&self) -> R<()> {
        self.wait_btn(1)
    }
    fn mouse(&self) -> (i32, i32) {
        let w = self.wd();
        (w.mouse_x, w.mouse_y)
    }

    // ---- soldier records ----
    fn sb(&self, s: usize, off: usize) -> u8 {
        if s >= SLOTS || off >= REC {
            return 0;
        }
        rd(self.wd().soldiers, SLOTS * REC, s * REC + off)
    }
    fn ss(&self, s: usize, off: usize) -> i32 {
        self.sb(s, off) as i8 as i32
    }
    fn sw(&self, s: usize, off: usize, v: u8) {
        if s >= SLOTS || off >= REC {
            return;
        }
        wr(self.wd().soldiers, SLOTS * REC, s * REC + off, v)
    }
    fn tile_of(&self, s: usize) -> i32 {
        (self.sb(s, S_TILE) as u16 | (self.sb(s, S_TILE + 1) as u16) << 8) as i16 as i32
    }

    // ---- item entries ----
    fn gb(&self, l: Loc, i: usize) -> u8 {
        match l {
            Loc::Cursor => rd(self.wd().inv_cursor, 12, i),
            Loc::Swap => rd(self.wd().inv_swap, 12, i),
            Loc::Sol(s, e) => self.sb(s, S_ITEMS + 12 * e + i),
        }
    }
    fn pb(&self, l: Loc, i: usize, v: u8) {
        match l {
            Loc::Cursor => wr(self.wd().inv_cursor, 12, i, v),
            Loc::Swap => wr(self.wd().inv_swap, 12, i, v),
            Loc::Sol(s, e) => self.sw(s, S_ITEMS + 12 * e + i, v),
        }
    }
    /// Like pb, but the byte index may be negative (before the entry, inside the soldier record).
    fn pb_signed(&self, l: Loc, i: i32, v: u8) {
        match l {
            Loc::Sol(s, e) => {
                let o = (S_ITEMS + 12 * e) as i32 + i;
                if o >= 0 {
                    self.sw(s, o as usize, v);
                }
            }
            _ => {
                if i >= 0 {
                    self.pb(l, i as usize, v);
                }
            }
        }
    }
    fn typ(&self, l: Loc) -> u8 {
        self.gb(l, 0)
    }
    fn count(&self, l: Loc) -> i32 {
        self.gb(l, 1) as i8 as i32
    }
    fn cond(&self, l: Loc) -> i32 {
        self.gb(l, 2) as i8 as i32
    }
    fn get12(&self, l: Loc) -> [u8; 12] {
        let mut b = [0u8; 12];
        for (i, v) in b.iter_mut().enumerate() {
            *v = self.gb(l, i);
        }
        b
    }
    fn put12(&self, l: Loc, b: &[u8; 12]) {
        for (i, v) in b.iter().enumerate() {
            self.pb(l, i, *v);
        }
    }
    fn copy(&self, dst: Loc, src: Loc) {
        let b = self.get12(src);
        self.put12(dst, &b);
    }
    fn clear(&self, l: Loc) {
        self.put12(l, &[0u8; 12]);
    }
    /// Exchange: cursor = entry, entry = swap.
    fn exchange(&self, e: Loc) {
        self.copy(Loc::Cursor, e);
        self.copy(e, Loc::Swap);
    }

    // ---- tables ----
    fn item_type(&self, t: u8, i: usize) -> u8 {
        rd(self.wd().item_types, ITEM_TYPES * 12, t as usize * 12 + i)
    }
    fn max_stack(&self, t: u8) -> u8 {
        self.item_type(t, 6)
    }
    fn stacks(&self, t: u8) -> bool {
        self.max_stack(t) > 1
    }
    fn clip(&self, t: u8) -> u8 {
        self.item_type(t, 5)
    }
    fn ammo_of(&self, t: u8) -> u8 {
        rd(self.wd().weapons, WEAPONS * 6, 6 * t as usize + 1)
    }
    fn give(&self, i: usize) -> u8 {
        rd(self.wd().inv_give, 2, i)
    }
    fn set_give(&self, i: usize, v: u8) {
        wr(self.wd().inv_give, 2, i, v)
    }
    fn set_picked(&self, s: usize, v: u8) {
        wr(self.wd().inv_picked, 8, s, v)
    }
    fn picked(&self, s: usize) -> u8 {
        rd(self.wd().inv_picked, 8, s)
    }
    fn rect(&self, p: *const u8, k: usize) -> (i32, i32) {
        let g = |i: usize| (rd(p, 64, i) as u16 | (rd(p, 64, i + 1) as u16) << 8) as i16 as i32;
        (g(4 * k), g(4 * k + 2))
    }
    fn tile_ground(&self, t: i32) -> u8 {
        if !(0..TILES as i32).contains(&t) {
            return 0;
        }
        rd(self.wd().tiles, TILES * 8, t as usize * 8 + 6)
    }
    /// Byte i of ground entry g; entries g of 200 or more read past the 200 entries (`shared::ground_byte`).
    fn ground(&self, g: u8, i: usize) -> u8 {
        super::shared::ground_byte(self.w, g as usize * 12 + i)
    }
    fn set_ground(&self, g: u8, i: usize, v: u8) {
        super::shared::ground_byte_put(self.w, g as usize * 12 + i, v)
    }
    fn mine_count_down(&self) {
        let c = self.wd().cur_sector as usize;
        if c >= SECTOR_ROWS {
            return;
        }
        let p = self.wd().sectors;
        let i = c * SECTOR_ROW + 0xAF;
        let v = rd(p, SECTOR_ROWS * SECTOR_ROW, i);
        wr(p, SECTOR_ROWS * SECTOR_ROW, i, v.wrapping_sub(1));
    }

    // ---- front-end ----
    fn show(&self) {
        self.call(MOUSE_SHOW, &[]);
    }
    fn hide(&self) {
        self.call(op::MOUSE_OFF, &[]);
    }
    fn click(&self) {
        self.call(INV_CLICK, &[]);
    }
    fn cursor_pic(&self) {
        let t = self.typ(Loc::Cursor) as i32;
        self.call(CURSOR_PIC, &[t]);
    }
    fn cursor_pic2(&self) {
        let t = self.typ(Loc::Cursor) as i32;
        self.call(CURSOR_PIC2, &[t]);
    }
    fn message(&self, t: i32) {
        self.call(op::MESSAGE, &[t]);
    }
    fn extra(&self, snd: i32, txt: i32) {
        self.call(INV_EXTRA, &[snd, txt]);
    }
    fn body(&self, s: usize) -> i32 {
        self.call(INV_BODY, &[s as i32]) as i32
    }
    fn draw_slot(&self, s: usize, p: usize) {
        self.call(DRAW_SLOT, &[s as i32, p as i32]);
    }
    fn slot_redraw(&self, s: usize) {
        self.call(op::PORTRAIT_REFRESH, &[s as i32, 1]);
    }
    fn spend_ap(&self, s: usize, ap: i32, fatigue: i32) {
        self.call(op::SPEND_AP, &[s as i32, ap, fatigue]);
    }
    fn stat_practice(&self, s: usize, stat: i32, amount: i32) {
        self.call(op::STAT_PRACTICE, &[s as i32, stat, amount, 0]);
    }
    fn put_item(&self, tile: i32, l: Loc) {
        let b = self.get12(l);
        let d = |k: usize| u32::from_le_bytes([b[4 * k], b[4 * k + 1], b[4 * k + 2], b[4 * k + 3]]) as i32;
        self.call(INV_PUT_ITEM, &[tile, d(0), d(1), d(2), 2, 0, 0, 0]);
    }
    fn item_info(&self, s: usize, t: u8, count: i32, v: i32, info: i32) -> u32 {
        let prof = self.sb(s, S_PROFILE) as i32;
        self.call(ITEM_INFO, &[prof, t as i32, count, v, 1, 1, info])
    }
    fn armour_rating(&self, s: usize) -> i32 {
        self.a.armour(s)
    }

    // ---- rules ----
    /// Not enough AP (flag 99: the check applies to any merc): true = refused (the merc says so).
    fn refused(&self, s: usize, c: i32) -> bool {
        if self.wd().combat != 0 && self.ss(s, S_AP_LEFT) + self.ss(s, S_AP_RESERVE) < c {
            self.call(op::MERC_TEXT, &[s as i32, 1, 1200]);
            return true;
        }
        false
    }
    fn missing(&self, s: usize, k: usize) -> bool {
        let need = match k {
            7 | 8 => 0x32,
            9 => 0x33,
            10 => 0x34,
            11 => 0x35,
            _ => return false,
        };
        self.sb(s, S_SLOT) < 8 && self.sb(s, S_VEST) < need
    }
    /// The skill bonus (+0x1D0) of S.
    fn skill_bonus(&self, s: usize) {
        self.sw(s, S_SKILL_BONUS, (self.ss(s, S_LEVEL) > 3) as u8);
        let ear = Loc::Sol(s, 4);
        if self.typ(ear) == 0x2B {
            let v = self.ss(s, S_SKILL_BONUS) + 5 - (100 - self.cond(ear)) / 20;
            self.sw(s, S_SKILL_BONUS, v as u8);
        }
    }
    /// Gave an item (slot n): GaveItem? 0x53958.
    fn gave(&self, n: usize) {
        let b = self.give(1);
        if b == NO_SLOT {
            return;
        }
        let a = self.give(0);
        if n as u8 != a && n as u8 != b {
            for k in [b as usize, n] {
                if self.ss(k, S_HP) >= 15 {
                    self.spend_ap(k, 6, 20);
                    self.set_picked(k, 1);
                    self.slot_redraw(k);
                }
            }
            let b = self.give(1);
            self.set_give(0, b);
            self.set_give(1, NO_SLOT);
        } else {
            if n as u8 == a {
                self.set_give(0, NO_SLOT);
            }
            self.set_give(1, NO_SLOT);
        }
    }
    /// Stack join (dst, src).
    fn stack_join(&self, dst: Loc, src: Loc) {
        let n = self.count(src);
        for i in 0..n.max(0) as usize {
            let c = if i == 0 { self.gb(src, 2) } else { self.gb(src, 7 + i) };
            let dc = self.count(dst);
            if i == 0 && dc == 0 {
                self.pb(dst, 2, c);
            } else {
                self.pb_signed(dst, 7 + dc, c);
            }
            self.pb(dst, 1, (dc + 1) as u8);
        }
    }
    /// inventory.md "No room" (entry, cursor): the entry is full = inv_extra("noroom.8", "No Room"); else units go from
    /// the end of the cursor's stack to the end of the entry's stack until the entry is full.
    fn no_room(&self, dst: Loc, src: Loc) {
        let n = self.max_stack(self.typ(dst)) as i32 - self.count(dst);
        if n < 1 {
            self.extra(SND_NOROOM, TXT_NOROOM);
            return;
        }
        for _ in 0..n {
            let sc = self.count(src);
            let si = 6 + sc;
            let b = if (0..12).contains(&si) { self.gb(src, si as usize) } else { 0 };
            if (0..12).contains(&si) {
                self.pb(src, si as usize, 0);
            }
            self.pb(src, 1, (sc - 1) as u8);
            let dc = self.count(dst) + 1;
            self.pb(dst, 1, dc as u8);
            // a soldier entry: offsets outside the 12 bytes reach the next entries (as the original)
            self.pb_signed(dst, 6 + dc, b);
        }
    }
    /// Take one (entry).
    fn take_one(&self, e: Loc) {
        let c = self.count(e);
        if c <= 0 {
            return;
        }
        if !self.stacks(self.typ(e)) || c == 1 {
            self.clear(e);
            return;
        }
        self.pb(e, 2, self.gb(e, 8));
        for k in 1..(c - 1).max(1) as usize {
            self.pb(e, 7 + k, self.gb(e, 8 + k));
        }
        self.pb(e, (6 + c) as usize, 0);
        self.pb(e, 1, (c - 1) as u8);
    }
    /// Unstack (ctrl): true when it acted.
    fn unstack(&self, e: Loc) -> bool {
        let c = self.count(e);
        if self.max_stack(self.typ(e)) == 1 || c == 99 || c < 2 {
            return false;
        }
        let first = self.gb(e, 2);
        self.take_one(e);
        let c = self.count(e) + 1;
        self.pb(e, 1, c as u8);
        self.pb(e, (6 + c).max(0) as usize, first);
        true
    }

    /// May it go (X = the swap item onto place p of S): 1 go on, 0 not allowed, -1 cancelled.
    fn may_go(&self, s: usize, p: usize) -> i32 {
        self.may_go_x(s, p, Loc::Swap)
    }
    /// May it go with item X (the bottom strip asks with the cursor item itself).
    fn may_go_x(&self, s: usize, p: usize, x: Loc) -> i32 {
        let t = self.typ(x);
        if t == 0 {
            return 1;
        }
        if (0x31..=0x35).contains(&t) {
            return match p {
                6 => 1,
                0 | 1 => {
                    let o = self.wd().vest_owner;
                    if o >= NO_SLOT {
                        return 1;
                    }
                    if (7..12).all(|k| self.typ(Loc::Sol(o as usize, k)) == 0) {
                        1
                    } else {
                        self.message(MSG_VEST_NOT_EMPTY);
                        0
                    }
                }
                _ => 0,
            };
        }
        let r = |ok: bool| ok as i32;
        match p {
            2 => r((0x23..=0x26).contains(&t)),
            3 => r((0x27..=0x28).contains(&t) && !(t == 0x27 && self.count(x) >= 2)),
            4 => r((0x2A..=0x2B).contains(&t)),
            5 => r((0x2D..=0x30).contains(&t)),
            6 => 0,
            12 => self.may_merge(s),
            _ => 1,
        }
    }
    /// May it go, p = 12: onto the item in entry 0.
    fn may_merge(&self, s: usize) -> i32 {
        let x = Loc::Swap;
        let hl = Loc::Sol(s, 0);
        let xt = self.typ(x);
        let ht = self.typ(hl);
        if self.gb(x, 3) != 0
            || (self.stacks(xt) && self.count(x) >= 2)
            || (self.stacks(ht) && self.count(hl) >= 2)
        {
            return 0;
        }
        let tab = self.wd().attach;
        let row = (0..MERGE_ROWS).find(|&k| rd(tab, MERGE_ROWS * 4, 4 * k) == xt && rd(tab, MERGE_ROWS * 4, 4 * k + 1) == ht);
        let Some(k) = row else { return 0 };
        let result = rd(tab, MERGE_ROWS * 4, 4 * k + 2);
        let kind = rd(tab, MERGE_ROWS * 4, 4 * k + 3);
        if result == 0 {
            return 1;
        }
        if self.call(MERGE_PROMPT, &[xt as i32, ht as i32]) != 1 {
            return -1;
        }
        let r = self.merge(s, kind);
        if r < 2 {
            return r;
        }
        self.pb(hl, 0, result);
        if self.clip(result) == 99 {
            self.pb(hl, 1, 99);
        }
        r
    }
    /// The merge by kind (X = swap, H = entry 0 of S).
    fn merge(&self, s: usize, kind: u8) -> i32 {
        let x = Loc::Swap;
        let hl = Loc::Sol(s, 0);
        match kind {
            0 => {
                self.stat_practice(s, 4, 2);
                3
            }
            1 => {
                let skill = self.ss(s, S_MECHANICAL) - (100 - self.ss(s, S_DEX)) / 2;
                let roll = self.random(100);
                if roll < skill {
                    self.stat_practice(s, 7, 10);
                    return 2;
                }
                let u = self.random(3);
                let d = roll - skill;
                let lose = |l: Loc, n: i32| self.pb(l, 2, (self.cond(l) - n) as u8);
                match u {
                    0 => {
                        lose(hl, d / 2);
                        lose(x, d / 2);
                    }
                    1 => lose(hl, d),
                    2 => lose(x, d),
                    _ => {}
                }
                for l in [x, hl] {
                    if self.cond(l) <= 0 {
                        self.pb(l, 0, 0);
                        self.pb(l, 1, 0);
                        self.pb(l, 2, 0);
                    }
                }
                self.hide();
                self.call(CURSOR_PIC, &[0]);
                self.call(INV_VEST_BEGIN, &[0, 0x34, 0x25]);
                self.show();
                self.wd().voice_test = 1;
                let prof = self.sb(s, S_PROFILE) as i32;
                self.call(MERGE_QUOTE, &[prof]);
                self.call(MERGE_YES_NO, &[s as i32]);
                self.call(INV_VEST_END, &[0x25]);
                self.stat_practice(s, 7, 5);
                1
            }
            2 => {
                let _ = self.random(100);
                self.stat_practice(s, 7, 10);
                2
            }
            3 => {
                let skill = self.ss(s, S_EXPLOSIVES);
                let roll = self.random(100);
                self.stat_practice(s, 6, 10);
                let h = self.cond(hl);
                let xc = self.cond(x);
                let v = (((h + xc) / 2 + h * xc / 100) / 2 + skill - roll).clamp(10, 100);
                self.pb(hl, 2, v as u8);
                2
            }
            4 => {
                let h = self.cond(hl);
                let xc = self.cond(x);
                if h + xc <= 100 {
                    self.pb(hl, 2, (h + xc) as u8);
                    self.pb(x, 2, 0);
                } else {
                    let rest = h + xc - 100;
                    self.pb(x, 2, rest as u8);
                    self.pb(hl, 2, 100);
                    let ht = self.typ(hl);
                    self.pb(hl, 3, ht);
                    self.pb(hl, 4, if self.stacks(ht) { 1 } else { 99 });
                    self.pb(hl, 5, rest as u8);
                }
                4
            }
            _ => 0,
        }
    }

    // ---- the screen ----
    fn run(&self, slot: usize, redraw: bool) -> R<i32> {
        let mut s = slot;
        let mut redraw = redraw;
        // Start.
        self.set_give(0, NO_SLOT);
        self.set_give(1, NO_SLOT);
        for k in 0..8 {
            self.set_picked(k, 0);
        }
        self.show();
        let mut armour = self.armour_rating(s);
        for i in 0..6 {
            self.pb(Loc::Swap, i, 0);
        }
        self.call(INV_BEGIN, &[0]);
        loop {
            if self.frame(&mut s, &mut redraw, &mut armour)? == Flow::Leave {
                break;
            }
        }
        // After the loop.
        self.skill_bonus(s);
        self.call(INV_LEAVE_MERC, &[s as i32]);
        self.call(INV_END, &[]);
        Ok(0)
    }

    /// One pass of the loop (steps 1..5).
    fn frame(&self, s: &mut usize, redraw: &mut bool, armour: &mut i32) -> R<Flow> {
        // 1.
        if *redraw {
            let c = Loc::Cursor;
            let v = if self.typ(c) == MONEY {
                self.call(ITEM_VALUE, &[-1, 0]) as u16 as i32
            } else {
                self.cond(c)
            };
            if self.item_info(*s, self.typ(c), self.count(c), v, 0) != 0 {
                *armour = self.body(*s);
            }
            *redraw = false;
        }
        // 2.
        self.poll()?;
        // 3.
        if self.call(KEY_PRESSED, &[]) != 0 {
            let mut k = self.call(GET_KEY, &[]) as u8;
            if k == 0 {
                self.call(GET_KEY, &[]);
            } else {
                if k.is_ascii_lowercase() {
                    k -= 0x20;
                }
                if k == 0x1B {
                    if self.typ(Loc::Cursor) == 0 {
                        return Ok(Flow::Leave);
                    }
                    self.message(MSG_PUT_DOWN);
                } else if k == b'T' {
                    self.call(TEXT_TOGGLE, &[]);
                }
            }
        }
        let btn = self.wd().mouse_buttons;
        // 4.
        if btn & 1 == 0 {
            if btn & 2 != 0 {
                return self.right_click(*s, armour);
            }
            let (x, y) = self.mouse();
            if in_rect(x, y, 72, 31, 100, 55) {
                let n = self.wd().selected as i32 + 1;
                self.call(TIP_MERC, &[n]);
            } else {
                let t = self.wd().tip_item as i32;
                self.call(TIP_ITEM, &[t]);
            }
            return Ok(Flow::Next);
        }
        // 5.
        let (x, y) = self.mouse();
        if in_rect(x, y, 108, 62, 156, 86) && !self.refused(*s, 5) {
            self.drink(*s);
            self.wait_up()?;
            return Ok(Flow::Next);
        }
        let (x, y) = self.mouse();
        if in_rect(x, y, 112, 82, 150, 118) && !self.refused(*s, 10) {
            self.camouflage(*s, armour);
            self.wait_up()?;
            return Ok(Flow::Next);
        }
        let (x, y) = self.mouse();
        if in_rect(x, y, 70, 28, 102, 56) {
            self.click();
            if self.typ(Loc::Cursor) == 0 {
                self.wait_up()?;
                self.wd().tip_item = 0;
                self.hide();
                self.wd().face_left = 1;
                return Ok(Flow::Leave);
            }
            self.message(MSG_PUT_DOWN);
            self.wait_up()?;
            return Ok(Flow::Next);
        }
        let (x, _) = self.mouse();
        if !(48..=271).contains(&x) {
            return self.merc_slot_click(s, armour);
        }
        self.item_click(*s, armour)
    }

    /// Step 4, right button down.
    fn right_click(&self, s: usize, armour: &mut i32) -> R<Flow> {
        let (x, y) = self.mouse();
        let rects = self.wd().item_rects;
        let hit = (0..13).find(|&k| {
            let (rx, ry) = self.rect(rects, k);
            in_rect(x, y, rx, ry, rx + 41, ry + 15)
        });
        if let Some(p) = hit {
            self.click();
            let e0 = Loc::Sol(s, 0);
            let (t, n, v) = if p == 12 {
                let t = self.gb(e0, 3);
                let c = self.gb(e0, 5) as i8 as i32;
                (t, self.gb(e0, 4) as i8 as i32, if t == MONEY { c * 10 } else { c })
            } else {
                let e = Loc::Sol(s, p);
                let v = self.call(ITEM_VALUE, &[s as i32, p as i32]) as u16 as i32;
                let v = if v == 0 { self.cond(e) } else { v };
                (self.typ(e), self.count(e), v)
            };
            if self.item_info(s, t, n, v, 1) == 1 {
                *armour = self.body(s);
            }
            self.wait_btn(2)?;
        }
        self.wait_btn(2)?;
        let w = self.wd();
        if w.mouse_y.wrapping_add(w.panel_dy) > 181 {
            if self.typ(Loc::Cursor) != 0 {
                self.wd().cursor_drop = 1;
                let a = self.call(DROP_ANSWER, &[1, 190, 318, 197]) as i32;
                self.wd().drop_answer = a;
            }
            return Ok(Flow::Leave);
        }
        Ok(Flow::Next)
    }

    fn drink(&self, s: usize) {
        self.click();
        let c = Loc::Cursor;
        let t = self.typ(c);
        if t != 0x41 && t != 0x4F {
            return;
        }
        if t == 0x41 && self.count(c) != 1 {
            self.message(MSG_ONE_DRINK);
        } else {
            self.click();
            let cv = self.cond(c);
            let n = if cv > 20 {
                20
            } else if t == 0x4F {
                cv
            } else {
                cv - 1
            };
            if n <= 1 {
                self.extra(SND_EMPTY, TXT_EMPTY);
            } else {
                self.spend_ap(s, 5, 0);
                self.slot_redraw(s);
                let amount = (100 - self.ss(s, S_ENERGY)) * n;
                self.call(REST, &[s as i32, amount, 1]);
                self.call(op::RESERVE_AP, &[s as i32]);
                self.extra(SND_HMM, TXT_DRANK);
                let v = self.cond(c) - n;
                self.pb(c, 2, v as u8);
            }
            if self.cond(c) <= 0 {
                if t == 0x4F {
                    self.call(op::DELAY, &[1000]);
                }
                self.clear(c);
                self.extra(SND_EMPTY, TXT_EMPTY);
            }
        }
        self.cursor_pic2();
    }

    fn camouflage(&self, s: usize, armour: &mut i32) {
        self.click();
        let c = Loc::Cursor;
        if self.typ(c) != 0x47 {
            return;
        }
        if self.count(c) != 1 {
            self.message(MSG_ONE_KIT);
        } else {
            self.click();
            self.sw(s, S_CAMO, self.sb(s, S_CAMO).wrapping_add(3));
            self.spend_ap(s, 10, 0);
            self.slot_redraw(s);
            *armour = self.body(s);
            self.extra(SND_HMM, TXT_OK);
            let v = self.cond(c) - 20;
            self.pb(c, 2, v as u8);
            if self.cond(c) <= 0 {
                self.clear(c);
                self.extra(SND_EMPTY, TXT_EMPTY);
            }
        }
        self.cursor_pic2();
    }

    fn merc_slot_click(&self, s: &mut usize, armour: &mut i32) -> R<Flow> {
        let (x, y) = self.mouse();
        let rects = self.wd().slot_rects;
        let hit = (0..8).find(|&k| {
            let (rx, ry) = self.rect(rects, k);
            in_rect(x, y, rx, ry, rx + 47, ry + 41)
        });
        let Some(k) = hit else {
            self.wait_up()?;
            return Ok(Flow::Next);
        };
        let cur = *s;
        if self.sb(k, S_OUT) != 0 || k == cur || (self.ss(k, S_HP) < 15 && self.ss(cur, S_HP) < 15) {
            if k != cur {
                self.wait_up()?;
                return Ok(Flow::Next);
            }
            if self.typ(Loc::Cursor) != 0 {
                self.message(MSG_PUT_DOWN);
                self.wait_up()?;
                return Ok(Flow::Next);
            }
            self.wait_up()?;
            return Ok(Flow::Leave);
        }
        if self.typ(Loc::Cursor) != 0 && k as u8 != self.give(0) {
            if self.ss(cur, S_HP) >= 15 && self.refused(cur, 6) {
                self.wait_up()?;
                return Ok(Flow::Next);
            }
            if self.ss(k, S_HP) >= 15 && self.refused(k, 6) {
                self.wait_up()?;
                return Ok(Flow::Next);
            }
        }
        if self.call(INV_SELECT, &[k as i32, 0, 0]) == 0 {
            self.wait_up()?;
            return Ok(Flow::Next);
        }
        if self.typ(Loc::Cursor) != 0 && self.give(1) == NO_SLOT {
            self.set_give(1, cur as u8);
        }
        self.skill_bonus(cur);
        self.call(INV_LEAVE_MERC, &[cur as i32]);
        *s = k;
        *armour = self.body(k);
        self.wait_up()?;
        Ok(Flow::Next)
    }

    fn item_click(&self, s: usize, armour: &mut i32) -> R<Flow> {
        let (x, y) = self.mouse();
        let rects = self.wd().item_rects;
        let mut hit = (0..14).find(|&k| {
            let (rx, ry) = self.rect(rects, k);
            in_rect(x, y, rx - 4, ry - 4, rx + 45, ry + 19)
        });
        if let Some(p) = hit {
            if self.wd().key_ctrl != 0 {
                // Port fix P02 (#71, #2 row 10): rects 12 / 13 (attachment, ground) hold no item entry; the original
                // unstacks soldier bytes +0x90 / +0x9C there. With P02 the click is still used, nothing moves.
                if !(crate::fanfix::port_on(2) && p >= 12) && self.unstack(Loc::Sol(s, p)) {
                    self.hide();
                    self.draw_slot(s, p);
                    self.show();
                }
                hit = None;
            }
        }
        let Some(p) = hit else {
            self.wait_up()?;
            return Ok(Flow::Next);
        };
        if p == 13 {
            return if self.typ(Loc::Cursor) == 0 { self.pick_up(s, armour) } else { self.put_down(s, armour) };
        }
        let c = Loc::Cursor;
        // Reload.
        if p < 12 {
            let e = Loc::Sol(s, p);
            let gt = self.typ(e);
            if (1..=14).contains(&gt) && self.count(e) == 0 && self.count(c) == 1 && self.typ(c) == self.ammo_of(gt) {
                self.pb(e, 1, self.clip(gt));
                if self.ss(s, S_1D5) > 0 {
                    self.stat_practice(s, 3, 1);
                }
                self.call(op::SFX, &[if gt < 8 { 15 } else { 14 }, 0x2B11, 50, 1]);
                self.clear(c);
                self.hide();
                self.draw_slot(s, p);
                if p == 0 {
                    self.draw_slot(s, 12);
                }
                self.cursor_pic();
                self.show();
                if self.wd().opt_step_text != 0 {
                    self.call(op::MERC_TEXT, &[s as i32, 2, 800]);
                }
                self.wait_up()?;
                return Ok(Flow::Next);
            }
        }
        self.copy(Loc::Swap, c);
        let ans = self.may_go(s, p);
        if ans == -1 {
            self.wait_up()?;
            return Ok(Flow::Next);
        }
        if ans == 0 {
            self.extra(SND_HUH, TXT_NONE);
            self.wait_up()?;
            return Ok(Flow::Next);
        }
        for (i, b) in 2000i32.to_le_bytes().iter().enumerate() {
            self.sw(s, S_B4 + i, *b);
        }
        self.sw(s, S_D0, 0x41);
        self.sw(s, S_D0 + 1, 0x0C);
        self.sw(s, S_130, 1);
        let mut skip = false;
        let ct = self.typ(c);
        if p == 6 && (0x32..=0x35).contains(&ct) {
            let o = self.wd().vest_owner;
            if o < NO_SLOT && o as usize != s {
                for k in 7..12 {
                    let a = Loc::Sol(s, k);
                    let b = Loc::Sol(o as usize, k);
                    let ab = self.get12(a);
                    self.copy(a, b);
                    self.put12(b, &ab);
                }
            } else {
                if o < NO_SLOT {
                    self.wd().vest_owner = NO_SLOT;
                }
                if self.sb(s, S_VEST) > self.typ(c) {
                    self.call(INV_VEST_BEGIN, &[0, 0x34, 0x25]);
                    self.copy(c, Loc::Sol(s, 6));
                    self.copy(Loc::Sol(s, 6), Loc::Swap);
                    for k in 7..12 {
                        if self.missing(s, k) && self.typ(Loc::Sol(s, k)) != 0 {
                            // Original 0x538FC drops occupied pockets from a vest that no longer fits.
                            let t = self.tile_of(s);
                            self.put_item(t, Loc::Sol(s, k));
                            self.clear(Loc::Sol(s, k));
                        }
                    }
                    self.call(INV_VEST_END, &[0x25]);
                    self.gave(s);
                    skip = true;
                }
            }
        } else if p == 6 {
            self.wd().vest_owner = s as u8;
        }
        if p < 12 && !skip && !self.missing(s, p) {
            self.pocket_move(s, p);
        }
        if p == 12 && self.typ(Loc::Sol(s, 0)) != 0 {
            self.gave(s);
            let e0 = Loc::Sol(s, 0);
            self.pb(c, 0, self.gb(e0, 3));
            self.pb(c, 1, self.gb(e0, 4));
            self.pb(c, 2, self.gb(e0, 5));
            for i in [3, 4, 5, 8, 9, 10, 11] {
                self.pb(c, i, 0);
            }
            if ans == 1 {
                for i in 0..3 {
                    self.pb(e0, 3 + i, self.gb(Loc::Swap, i));
                }
            } else {
                self.extra(SND_HMM, TXT_NONE);
                if ans == 4 {
                    for i in 3..6 {
                        self.pb(e0, i, 0);
                    }
                } else {
                    for i in 0..3 {
                        self.pb(c, i, 0);
                    }
                }
            }
        }
        self.clear(Loc::Swap);
        // Redraw.
        if p == 6 {
            self.hide();
            self.draw_slot(s, 6);
            self.cursor_pic();
            self.show();
            self.call(VESTMAP, &[s as i32]);
            self.click();
        } else {
            self.hide();
            if !self.missing(s, p) {
                self.draw_slot(s, p);
                self.click();
            }
            if p == 0 || p == 12 {
                self.draw_slot(s, 0);
                self.draw_slot(s, 12);
            }
            self.cursor_pic();
            self.show();
        }
        self.call(INV_MOVED, &[s as i32, 1, 1]);
        if self.armour_rating(s) != *armour {
            *armour = self.body(s);
        }
        self.wait_up()?;
        Ok(Flow::Next)
    }

    /// Pocket move (p = 0..11).
    fn pocket_move(&self, s: usize, p: usize) {
        let c = Loc::Cursor;
        let e = Loc::Sol(s, p);
        let shift = self.wd().key_shift != 0;
        let et = self.typ(e);
        if self.typ(c) == et && self.stacks(et) && p != 3 {
            self.click();
            if shift {
                self.exchange(e);
                self.gave(s);
            } else if (self.max_stack(et) as i32) < self.count(e) + self.count(c) || p == 3 {
                if (2..=6).contains(&p) {
                    self.extra(SND_NOROOM, TXT_NOROOM);
                } else {
                    self.no_room(e, c);
                }
            } else {
                self.stack_join(e, c);
                self.clear(c);
                self.gave(s);
            }
        } else if self.count(e) <= 1 || !self.stacks(et) {
            if p == 3 && self.typ(c) == 0x27 && self.count(c) >= 2 {
                self.extra(SND_NOROOM, TXT_NOROOM);
            } else {
                self.exchange(e);
                self.gave(s);
            }
        } else if self.typ(c) == 0 {
            if shift {
                self.exchange(e);
            } else {
                self.copy(c, e);
                self.pb(c, 1, 1);
                for i in 8..12 {
                    self.pb(c, i, 0);
                }
                self.take_one(e);
            }
        } else {
            self.exchange(e);
            self.gave(s);
        }
    }

    /// Put down (p = 13, an item on the cursor).
    fn put_down(&self, s: usize, armour: &mut i32) -> R<Flow> {
        self.gave(s);
        self.set_picked(s, 1);
        self.click();
        let c = Loc::Cursor;
        if (0x32..=0x35).contains(&self.typ(c)) {
            let o = self.wd().vest_owner;
            if o < NO_SLOT {
                for k in 7..12 {
                    let t = self.tile_of(s);
                    self.put_item(t, Loc::Sol(o as usize, k));
                    self.clear(Loc::Sol(o as usize, k));
                }
            }
            self.wd().vest_owner = NO_SLOT;
        }
        let t = self.tile_of(s);
        self.put_item(t, c);
        self.clear(c);
        *armour = self.body(s);
        self.cursor_pic2();
        self.wait_up()?;
        Ok(Flow::Next)
    }

    /// Pick up (p = 13, empty cursor).
    fn pick_up(&self, s: usize, armour: &mut i32) -> R<Flow> {
        let tile = self.tile_of(s);
        let mut g = self.tile_ground(tile);
        if g != 0xFF && self.ground(g, 6) == 0xFF && (self.ground(g, 2) as i8) < 0 {
            g = 0xFF;
        }
        if self.picked(s) == 0 {
            if g == 0xFF || self.refused(s, 4) {
                self.wait_up()?;
                return Ok(Flow::Next);
            }
            self.spend_ap(s, 4, 0);
            self.set_picked(s, 1);
        }
        let spotted = self.call(MINE_SPOTTED, &[s as i32, tile, g as i32]) as u8 as i8;
        let mine = spotted != 0;
        // F56 (found by DarzaR): 1.15m UseInventoryItem? 0x54A4D jle: a negative MineSpotted? answer counts no mine down.
        let down = mine && (spotted > 0 || !crate::fanfix::on(56));
        let r = self.call(DISARM, &[s as i32, tile, g as i32, 0, 0]) as i32;
        if r > 0 {
            self.body(s);
            self.call(PICK_UP, &[s as i32, tile, 0]);
            if down {
                self.mine_count_down();
            }
            self.cursor_pic();
            *armour = self.body(s);
            self.wait_up()?;
            return Ok(Flow::Next);
        }
        if self.tile_ground(tile) >= 200 {
            self.mine_count_down();
            self.call(op::FOCUS, &[s as i32, 1]);
            return Ok(Flow::Leave);
        }
        *armour = self.body(s);
        if mine {
            let g = self.tile_ground(tile);
            let v = self.ground(g, 2) as i8;
            self.set_ground(g, 2, v.wrapping_neg() as u8);
        }
        self.wait_up()?;
        Ok(Flow::Next)
    }
}

/// inventory.md: the screen loop for the merc in `slot`. 0 = the player left the screen, 1 = op 221 said stop.
pub(super) fn inventory_screen(w: *mut Ja1World, h: *const Ja1Host, slot: i32, redraw: i32) -> i32 {
    // Any soldier 0..47 (cursor.md "Debug keys (level 4)": Shift + left click opens it also for an enemy).
    // SPEC GAP: a slot outside 0..47 or a null world / host / soldier buffer does nothing (0).
    let Some(a) = Ai::new(w, h, slot) else { return 0 };
    let inv = Inv { w, h, a };
    let _ = inv.h;
    match inv.run(slot as usize, redraw != 0) {
        Ok(v) => v,
        Err(Stop) => 1,
    }
}

// ---- the bottom strip (panel.md "Clicks on the bottom strip (item boxes)") ----
const INV_OPEN: i32 = 349;
const MOUSE_TO: i32 = 350;
const WAIT_UP: i32 = 351;
const WEAPON_CURSOR: i32 = 352;
/// Left edges of the 7 boxes and the item entry each box shows.
const STRIP_EDGES: [i32; 7] = [0, 49, 94, 139, 184, 229, 272];
const STRIP_ENTRY: [usize; 7] = [0, 7, 8, 9, 10, 11, 1];

impl Inv {
    /// The strip click for S (steps 1..8); `mode` = the cursor mode.
    fn strip_click(&self, s: usize, mode: i32) {
        if (self.sb(s, S_HP) as i8) < 15 || self.sb(s, 0x138) == 0 {
            return;
        }
        let h0 = self.sb(s, 0);
        let mut changed = false;
        let x = self.wd().mouse_x;
        let k = (0..7).rev().find(|&k| x >= STRIP_EDGES[k] - 1).map_or(-1, |k| k as i32);
        let entry = |k: i32| if k < 0 { 0 } else { STRIP_ENTRY[k as usize] };
        let buttons = self.wd().mouse_buttons;
        if buttons & 1 != 0 {
            let p = entry(k);
            let e = Loc::Sol(s, p);
            if self.may_go_x(s, p, Loc::Cursor) != 0 && !self.missing(s, p) {
                if self.wd().key_ctrl != 0 {
                    changed = self.unstack(e);
                } else {
                    let c = self.get12(Loc::Cursor);
                    self.copy(Loc::Cursor, e);
                    self.put12(e, &c);
                    changed = true;
                }
            }
            if self.typ(Loc::Cursor) != 0 && self.wd().cursor_drop == 0 {
                self.wd().drop_answer = self.call(DROP_ANSWER, &[1, 190, 318, 197]) as i32;
                let mx = self.wd().mouse_x;
                self.wd().drop_answer = self.call(MOUSE_TO, &[mx, 191, 318]) as i32;
                self.wd().cursor_drop = 1;
                self.wd().strip_box = k as u8;
            }
            self.hide();
            self.cursor_pic();
            self.call(INV_MOVED, &[s as i32, 0, 1]);
            self.show();
            self.call(WAIT_UP, &[1]);
        } else if buttons & 2 != 0 {
            changed = true;
            let mut open = false;
            if k == 0 {
                if self.typ(Loc::Sol(s, 0)) == 0 {
                    open = true;
                } else {
                    let free = (1..7).rev().map(|j| STRIP_ENTRY[j]).find(|&e| {
                        self.typ(Loc::Sol(s, e)) == 0 && !self.missing(s, e)
                    });
                    match free {
                        Some(e) => {
                            self.copy(Loc::Sol(s, e), Loc::Sol(s, 0));
                            self.clear(Loc::Sol(s, 0));
                        }
                        None => open = true,
                    }
                }
            } else if k == 6 && self.typ(Loc::Sol(s, 1)) != 0 {
                let b = self.get12(Loc::Sol(s, 1));
                self.copy(Loc::Sol(s, 1), Loc::Sol(s, 0));
                self.put12(Loc::Sol(s, 0), &b);
            } else {
                open = true;
            }
            if open {
                self.call(INV_OPEN, &[s as i32]);
            } else {
                self.call(INV_MOVED, &[s as i32, 0, 1]);
                self.call(WAIT_UP, &[2]);
            }
        }
        if self.typ(Loc::Cursor) == 0 && self.wd().cursor_drop != 0 {
            self.wd().drop_answer = self.call(DROP_ANSWER, &[0, 0, 319, 199]) as i32;
            self.wd().cursor_drop = 0;
        }
        if mode == 2 && self.wd().cursor_drop == 0 {
            self.call(WEAPON_CURSOR, &[s as i32]);
        }
        let h = self.sb(s, 0);
        if h != h0 {
            if self.sb(s, 0x177) != 0 && h != 0x4D {
                self.sw(s, 0x177, 0);
            }
            // sector_start.md Clear target
            self.sw(s, 0x1D8, 0x40);
            self.sw(s, 0x1C2, 0x40);
            self.sw(s, 0x1BE, 0);
            for off in [0xD0, 0xDC] {
                self.sw(s, off, 0x41);
                self.sw(s, off + 1, 0x0C);
            }
            self.call(op::ANIM_STEP, &[s as i32]);
        }
        if changed {
            self.call(op::RESERVE_AP, &[s as i32]);
            let slot = self.sb(s, S_SLOT) as i32;
            self.call(op::PORTRAIT_REFRESH, &[slot, 1]);
        }
    }
}

/// panel.md "Clicks on the bottom strip (item boxes)": the strip click for the merc in `slot` (mode = cursor mode).
pub(super) fn strip_click(w: *mut Ja1World, h: *const Ja1Host, slot: i32, mode: i32) {
    // SPEC GAP: a slot outside 0..47 or a null world / host does nothing.
    if !(0..SLOTS as i32).contains(&slot) {
        return;
    }
    let Some(a) = Ai::new(w, h, slot) else { return };
    Inv { w, h, a }.strip_click(slot as usize, mode);
}

#[no_mangle]
pub extern "C" fn ja1_strip_click(w: *mut Ja1World, h: *const Ja1Host, slot: i32, mode: i32) {
    std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| strip_click(w, h, slot, mode))).unwrap_or(())
}

#[no_mangle]
pub extern "C" fn ja1_inventory_screen(w: *mut Ja1World, h: *const Ja1Host, slot: i32, redraw: i32) -> i32 {
    std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| inventory_screen(w, h, slot, redraw))).unwrap_or(0)
}

// ---- the assignment screen (strategic.md "Item moves on the item panel") ----
const ASSIGN_PANEL: i32 = 321;
const DISCARD_ASK: i32 = 322;
const STASH_TAKE_REC: i32 = 323;
const STORE_PLACE: i32 = 324;
/// Op 324 answer bit: store view 0 with no current record.
const NO_RECORD: u32 = 1 << 26;
const STORE_AGAIN: i32 = 325;
const CAMO_KIT: u8 = 0x47;

impl Inv {
    fn is_pocket_vest(t: u8) -> bool {
        (0x32..=0x35).contains(&t)
    }
    /// Store an item with the 12 bytes at `l` (strategic.md "Store an item"): true when it passed its first tests.
    fn stash(&self, l: Loc) -> bool {
        let mut b = self.get12(l);
        let r = super::pay::stash_buf(self.w, self.h, &mut b);
        self.put12(l, &b);
        r
    }
    fn count_of(&self, t: u8) -> u8 {
        rd(self.wd().stash_counts, ITEM_TYPES, t as usize)
    }

    /// One right-button press of the assignment screen (strategic.md "Right button on the item panel").
    fn assign_right(&self, s: usize) -> R<()> {
        let (x, y) = self.mouse();
        let rects = self.wd().item_rects;
        let prof = self.sb(s, S_PROFILE) as i32;
        let info = |t: i32, n: i32, v: i32, ab: i32| {
            if self.call(ITEM_INFO, &[prof, t, n, v, ab, ab, 1]) != 0 {
                self.call(ASSIGN_PANEL, &[s as i32]);
            }
        };
        let hit = (0..13).find(|&k| {
            let (rx, ry) = self.rect(rects, k);
            in_rect(x, y, rx, ry, rx + 41, ry + 15)
        });
        if let Some(p) = hit {
            self.click();
            let e = Loc::Sol(s, if p == 12 { 0 } else { p });
            let o = if p == 12 { 3 } else { 0 };
            info(self.gb(e, o) as i32, self.gb(e, o + 1) as i8 as i32, self.gb(e, o + 2) as i8 as i32, 1);
            self.wait_btn(2)?;
            return self.wait_btn(2);
        }
        if let Some(j) = (0..4).find(|&j| in_rect(x, y, 29, 59 + 19 * j, 70, 74 + 19 * j)) {
            let v = self.call(STORE_PLACE, &[j, 1]);
            if v & NO_RECORD != 0 {
                // No current record: the original reads machine memory here (strategic.md): a stop point. Port fix
                // P05 (#2 row 19): no info box, nothing else.
                if !crate::fanfix::port_on(5) {
                    self.call(op::DIV_ZERO, &[]);
                }
            } else if v & 1 << 25 != 0 {
                info((v & 0xFF) as i32, (v >> 8) as u8 as i8 as i32, (v >> 16) as u8 as i8 as i32, 1);
            } else if v != 0 {
                info((v & 0xFF) as i32, 99, 20, 0);
            }
        }
        self.wait_btn(2)
    }

    /// One item press of the assignment screen.
    fn assign_press(&self, s: usize, armour: &mut i32) -> R<()> {
        let (x, y) = self.mouse();
        let c = Loc::Cursor;
        // 1. Body.
        if in_rect(x, y, 161, 48, 199, 84) {
            self.click();
            if self.typ(c) == CAMO_KIT {
                self.click();
                self.sw(s, S_CAMO, self.sb(s, S_CAMO).wrapping_add(3));
                self.call(ASSIGN_PANEL, &[s as i32]);
                self.extra(SND_HMM, TXT_NONE);
                let v = self.gb(c, 2).wrapping_sub(20);
                self.pb(c, 2, v);
                if (v as i8) < 1 {
                    self.clear(c);
                    self.extra(SND_EMPTY, TXT_NONE);
                }
                self.cursor_pic();
            }
            return self.wait_up();
        }
        // 2. Item places.
        let rects = self.wd().item_rects;
        let mut hit = (0..14).find(|&k| {
            let (rx, ry) = self.rect(rects, k);
            in_rect(x, y, rx - 4, ry - 4, rx + 45, ry + 19)
        });
        if let Some(p) = hit {
            if self.wd().key_ctrl != 0 {
                // Port fix P02 (#71, #2 row 10): rects 12 / 13 (attachment, ground) hold no item entry; the original
                // unstacks soldier bytes +0x90 / +0x9C there. With P02 the click is still used, nothing moves.
                if !(crate::fanfix::port_on(2) && p >= 12) && self.unstack(Loc::Sol(s, p)) {
                    self.hide();
                    self.draw_slot(s, p);
                    self.show();
                }
                hit = None;
            }
        }
        let mut acted = false;
        match hit {
            // 3. Discard.
            Some(13) => {
                if self.typ(c) != 0 {
                    self.click();
                    if self.call(DISCARD_ASK, &[self.typ(c) as i32]) != 0 {
                        if Self::is_pocket_vest(self.typ(c)) {
                            let o = self.wd().vest_owner;
                            if o < NO_SLOT {
                                for k in 7..12 {
                                    self.clear(Loc::Sol(o as usize, k));
                                }
                                self.wd().vest_owner = NO_SLOT;
                            }
                        }
                        self.clear(c);
                        self.cursor_pic();
                    }
                }
            }
            // 4. Places 0..12.
            Some(p) => acted = self.assign_place(s, p, armour),
            None => {}
        }
        // 5. Store.
        if !acted && in_rect(x, y, 25, 60, 72, 132) {
            if self.typ(c) != 0 {
                self.click();
                let o = self.wd().vest_owner;
                if Self::is_pocket_vest(self.typ(c)) && o < NO_SLOT {
                    for k in 7..12 {
                        self.stash(Loc::Sol(o as usize, k));
                    }
                    self.wd().vest_owner = NO_SLOT;
                }
                if self.stash(c) {
                    self.cursor_pic();
                    self.call(STORE_AGAIN, &[0]);
                }
                return self.wait_up();
            }
            if let Some(j) = (0..4).find(|&j| in_rect(x, y, 29, 59 + 19 * j, 70, 74 + 19 * j)) {
                self.click();
                let v = self.call(STORE_PLACE, &[j]);
                if v & NO_RECORD != 0 {
                    // No current record: the original reads machine memory here (strategic.md): a stop point. Port
                    // fix P05 (#2 row 19): nothing is taken (the empty cursor picture, then wait up).
                    if !crate::fanfix::port_on(5) {
                        self.call(op::DIV_ZERO, &[]);
                    }
                } else if v & 1 << 24 != 0 {
                    let (t, q, cd) = (v as u8, (v >> 8) as u8, (v >> 16) as u8);
                    if v & 1 << 25 != 0 {
                        self.pb(c, 0, t);
                        self.pb(c, 1, q);
                        self.pb(c, 2, cd);
                        if t != 0 {
                            self.take(t, q, cd, true);
                        }
                    } else if t != 0 {
                        self.pb(c, 0, t);
                        self.take(t, 0, 0, false);
                    }
                }
                self.cursor_pic();
            }
        }
        self.wait_up()
    }

    /// Take (strategic.md "Item moves on the item panel"): records out of the store onto the cursor.
    fn take(&self, t: u8, q: u8, cd: u8, exact: bool) {
        let c = Loc::Cursor;
        let n = if !exact && self.wd().key_shift != 0 { self.max_stack(t).min(self.count_of(t)) } else { 1 };
        let mut took = 0u8;
        for i in 0..n as usize {
            let r = self.call(STASH_TAKE_REC, &[t as i32, q as i32, cd as i32, exact as i32]);
            if r == 0 {
                break;
            }
            let (rq, rc) = ((r >> 8) as u8, r as u8);
            if i == 0 {
                self.pb(c, 1, rq);
                self.pb(c, 2, rc);
                for k in [3, 4, 5, 8, 9, 10, 11] {
                    self.pb(c, k, 0);
                }
            } else {
                self.pb(c, 1, self.gb(c, 1).wrapping_add(1));
                self.pb(c, 7 + i, rc);
            }
            took += 1;
        }
        if took == 0 {
            return;
        }
        let sc = self.wd().stash_counts;
        wr(sc, ITEM_TYPES, t as usize, self.count_of(t).wrapping_sub(n));
        self.call(STORE_AGAIN, &[1]);
    }

    /// Step 4 of the item press, place p = 0..12: true when it acted (the store is not tested then).
    fn assign_place(&self, s: usize, p: usize, armour: &mut i32) -> bool {
        let c = Loc::Cursor;
        if p < 12 {
            let e = Loc::Sol(s, p);
            let gt = self.typ(e);
            if self.gb(e, 1) == 0 && (1..=14).contains(&gt) && self.count(c) == 1 && self.typ(c) == self.ammo_of(gt) {
                self.pb(e, 1, self.gb(e, 1).wrapping_add(self.clip(gt)));
                self.call(op::SFX, &[if gt < 8 { 0x19 } else { 0x1A }, 0x2B11, 50, 1]);
                self.clear(c);
                self.hide();
                self.draw_slot(s, p);
                if p == 0 {
                    self.draw_slot(s, 12);
                }
                self.cursor_pic();
                self.show();
                if self.wd().opt_step_text != 0 {
                    self.call(op::MERC_TEXT, &[s as i32, 2, 800]);
                }
                return true;
            }
        }
        self.copy(Loc::Swap, c);
        let a = self.may_go(s, p);
        if a == 0 {
            self.extra(SND_HUH, TXT_NONE);
        }
        if a <= 0 {
            return false;
        }
        let mut skip = false;
        if p == 6 && Self::is_pocket_vest(self.typ(c)) {
            let o = self.wd().vest_owner;
            if o < NO_SLOT && o != self.sb(s, S_SLOT) {
                for k in 7..12 {
                    let a = Loc::Sol(s, k);
                    let b = Loc::Sol(o as usize, k);
                    let ab = self.get12(a);
                    self.copy(a, b);
                    self.put12(b, &ab);
                }
            } else {
                if o < NO_SLOT {
                    self.wd().vest_owner = NO_SLOT;
                }
                if self.typ(c) < self.sb(s, S_VEST) {
                    self.copy(c, Loc::Sol(s, 6));
                    self.copy(Loc::Sol(s, 6), Loc::Swap);
                    for k in 7..12 {
                        if self.missing(s, k) && self.typ(Loc::Sol(s, k)) != 0 {
                            self.stash(Loc::Sol(s, k));
                        }
                    }
                    skip = true;
                    self.call(STORE_AGAIN, &[0]);
                }
            }
        } else if p == 6 {
            self.wd().vest_owner = self.sb(s, S_SLOT);
        }
        if p < 12 && !skip && !self.missing(s, p) {
            self.assign_pocket_move(s, p);
        }
        if p == 12 && self.typ(Loc::Sol(s, 0)) != 0 {
            let e0 = Loc::Sol(s, 0);
            self.pb(c, 0, self.gb(e0, 3));
            self.pb(c, 1, self.gb(e0, 4));
            self.pb(c, 2, self.gb(e0, 5));
            for i in [3, 4, 5, 8, 9, 10, 11] {
                self.pb(c, i, 0);
            }
            if a == 1 {
                for i in 0..3 {
                    self.pb(e0, 3 + i, self.gb(Loc::Swap, i));
                }
            } else {
                self.extra(SND_HMM, TXT_NONE);
                if a == 4 {
                    for i in 3..6 {
                        self.pb(e0, i, 0);
                    }
                } else {
                    for i in 0..3 {
                        self.pb(c, i, 0);
                    }
                }
            }
        }
        self.clear(Loc::Swap);
        if p == 6 {
            self.hide();
            self.draw_slot(s, 6);
            self.cursor_pic();
            self.show();
            self.call(VESTMAP, &[s as i32]);
            self.click();
        } else {
            self.hide();
            if !self.missing(s, p) {
                self.draw_slot(s, p);
                self.click();
            }
            if p == 0 || p == 12 {
                self.draw_slot(s, 0);
                self.draw_slot(s, 12);
            }
            self.cursor_pic();
            self.show();
        }
        let ar = self.armour_rating(s);
        if ar != *armour {
            *armour = ar;
            self.call(ASSIGN_PANEL, &[s as i32]);
        }
        true
    }

    /// Pocket move of the assignment screen (p = 0..11).
    fn assign_pocket_move(&self, s: usize, p: usize) {
        let c = Loc::Cursor;
        let e = Loc::Sol(s, p);
        let shift = self.wd().key_shift != 0;
        let et = self.typ(e);
        if self.typ(c) == et && self.stacks(et) && p != 3 {
            if shift {
                self.exchange(e);
            } else if (self.max_stack(et) as i32) < self.count(e) + self.count(c) {
                if (2..=6).contains(&p) {
                    self.extra(SND_NOROOM, TXT_NOROOM);
                } else {
                    self.no_room(e, c);
                }
            } else {
                self.stack_join(e, c);
                self.clear(c);
            }
        } else if self.count(e) <= 1 || !self.stacks(et) {
            if p == 3 && self.typ(c) == 0x27 && self.count(c) > 1 {
                self.extra(SND_NOROOM, TXT_NOROOM);
            } else {
                self.exchange(e);
            }
        } else if self.typ(c) == 0 && !shift {
            self.copy(c, e);
            self.pb(c, 1, 1);
            for i in 8..12 {
                self.pb(c, i, 0);
            }
            self.take_one(e);
            if self.gb(e, 1) == 0 {
                self.clear(e);
            }
        } else {
            self.exchange(e);
        }
    }
}

/// strategic.md "Item moves on the item panel": one item press of the assignment screen for merc `slot`.
pub(super) fn assign_item_press(w: *mut Ja1World, h: *const Ja1Host, slot: i32, armour: *mut i32) -> i32 {
    // SPEC GAP: a slot outside 0..7 or a null armour pointer does nothing (0).
    if !(0..8).contains(&slot) || armour.is_null() {
        return 0;
    }
    let Some(a) = Ai::new(w, h, slot) else { return 0 };
    let inv = Inv { w, h, a };
    let mut ar = unsafe { *armour };
    let r = match inv.assign_press(slot as usize, &mut ar) {
        Ok(()) => 0,
        Err(Stop) => 1,
    };
    unsafe { *armour = ar };
    r
}

/// strategic.md "Right button on the item panel": one right-button press for merc `slot`.
pub(super) fn assign_right_press(w: *mut Ja1World, h: *const Ja1Host, slot: i32) -> i32 {
    // SPEC GAP: a slot outside 0..7 does nothing (0).
    if !(0..8).contains(&slot) {
        return 0;
    }
    let Some(a) = Ai::new(w, h, slot) else { return 0 };
    let inv = Inv { w, h, a };
    match inv.assign_right(slot as usize) {
        Ok(()) => 0,
        Err(Stop) => 1,
    }
}

#[no_mangle]
pub extern "C" fn ja1_assign_right_press(w: *mut Ja1World, h: *const Ja1Host, slot: i32) -> i32 {
    std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| assign_right_press(w, h, slot))).unwrap_or(0)
}

/// strategic.md "Key E: all items into the store".
pub(super) fn assign_key_e(w: *mut Ja1World, h: *const Ja1Host, slot: i32) {
    store_all(w, h, slot, true)
}

/// All items of team slot `slot` into the store; `redraw` = the item panel again (key E; FIRE passes false).
pub(super) fn store_all(w: *mut Ja1World, h: *const Ja1Host, slot: i32, redraw: bool) {
    // SPEC GAP: a slot outside 0..7 does nothing.
    if !(0..8).contains(&slot) {
        return;
    }
    let Some(a) = Ai::new(w, h, slot) else { return };
    let inv = Inv { w, h, a };
    let s = slot as usize;
    for p in (0..12).rev() {
        let l = Loc::Sol(s, p);
        if inv.typ(l) != 0 {
            // Port fix P06 (#2 row 20): the entry is cleared only when the store took it (Store an
            // item clears its copy then); 1.13b clears it always, so an item of a type with 99+ in the store is lost.
            let mut c = inv.get12(l);
            super::pay::stash_buf(w, h, &mut c);
            if !crate::fanfix::port_on(6) || c[0] == 0 {
                for i in 0..6 {
                    inv.pb(l, i, 0);
                }
            }
        }
    }
    if redraw {
        inv.call(ASSIGN_PANEL, &[slot]);
    }
}

#[no_mangle]
pub extern "C" fn ja1_assign_key_e(w: *mut Ja1World, h: *const Ja1Host, slot: i32) {
    let _ = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| assign_key_e(w, h, slot)));
}

#[no_mangle]
pub extern "C" fn ja1_assign_item_press(w: *mut Ja1World, h: *const Ja1Host, slot: i32, armour: *mut i32) -> i32 {
    std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| assign_item_press(w, h, slot, armour))).unwrap_or(0)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::os::raw::c_void;

    struct Host {
        calls: Vec<(i32, Vec<i32>)>,
        polls: Vec<(i32, i32, u8, u16)>,
        w: *mut Ja1World,
    }
    unsafe extern "C" fn rnd(_c: *mut c_void, _n: i32) -> i32 {
        0
    }
    unsafe extern "C" fn call(c: *mut c_void, opc: i32, a: *const i32, n: i32, _o: *mut u8, _s: i32) -> u32 {
        let h = &mut *(c as *mut Host);
        let args = std::slice::from_raw_parts(a, n as usize).to_vec();
        h.calls.push((opc, args));
        if opc == INV_POLL {
            if h.polls.is_empty() {
                return 1;
            }
            let (x, y, b, f) = h.polls.remove(0);
            (*h.w).mouse_x = x;
            (*h.w).mouse_y = y;
            (*h.w).mouse_buttons = b;
            (*h.w).key_flags = f;
        }
        0
    }

    fn rects() -> Vec<u8> {
        let pts: [(i16, i16); 14] = [
            (108, 73), (203, 89), (161, 8), (108, 31), (200, 28), (108, 52), (257, 18),
            (257, 37), (257, 56), (257, 75), (257, 94), (257, 113), (108, 92), (159, 115),
        ];
        pts.iter().flat_map(|(x, y)| [x.to_le_bytes(), y.to_le_bytes()].concat()).collect()
    }

    #[test]
    fn stop_and_put_down() {
        let mut sold = vec![0u8; SLOTS * REC];
        let items = vec![0u8; ITEM_TYPES * 12];
        let mut cursor = [0u8; 12];
        let mut swap = [0u8; 12];
        let mut picked = [0u8; 8];
        let mut give = [0u8; 2];
        let ir = rects();
        sold[0x114] = 10; // tile 10
        cursor[0] = 0x10;
        cursor[1] = 1;
        let mut w: Ja1World = unsafe { std::mem::zeroed() };
        w.soldiers = sold.as_mut_ptr();
        w.item_types = items.as_ptr();
        w.inv_cursor = cursor.as_mut_ptr();
        w.inv_swap = swap.as_mut_ptr();
        w.inv_picked = picked.as_mut_ptr();
        w.inv_give = give.as_mut_ptr();
        w.item_rects = ir.as_ptr();
        w.vest_owner = NO_SLOT;
        let mut host = Host { calls: vec![], polls: vec![(160, 116, 1, 0), (160, 116, 0, 0)], w: &mut w };
        let h = Ja1Host { ctx: &mut host as *mut Host as *mut c_void, random: Some(rnd), call: Some(call), host_ops: !0 };
        let r = ja1_inventory_screen(&mut w, &h, 0, 0);
        assert_eq!(r, 1);
        let ops: Vec<i32> = host.calls.iter().map(|c| c.0).collect();
        assert_eq!(
            ops,
            vec![MOUSE_SHOW, INV_BEGIN, INV_POLL, KEY_PRESSED, INV_CLICK, INV_PUT_ITEM, INV_BODY, CURSOR_PIC2, INV_POLL,
                 INV_POLL]
        );
        assert_eq!(host.calls[5].1, vec![10, 0x0110, 0, 0, 2, 0, 0, 0]);
        assert_eq!(cursor, [0u8; 12]);
        assert_eq!(picked[0], 1);
        assert_eq!(give, [0x40, 0x40]);
    }
}

#[cfg(test)]
mod guard_tests {
    use super::*;

    #[test]
    fn guard_rd_range() {
        // Port rule (SPEC GAP): an index outside the table or a null table reads 0; the byte past the end is 9.
        let buf = [7u8, 1, 2, 3, 4, 9];
        let p = buf[1..].as_ptr();
        assert_eq!(rd(p, 4, 3), 4);
        assert_eq!(rd(p, 4, 4), 0);
        assert_eq!(rd(std::ptr::null(), 4, 4), 0);
    }

    #[test]
    fn guard_wr_range() {
        // Port rule (SPEC GAP): a write outside the table or to a null table is dropped.
        let mut buf = [0u8; 6];
        let p = unsafe { buf.as_mut_ptr().add(1) };
        wr(p, 4, 3, 5);
        wr(p, 4, 4, 6);
        wr(std::ptr::null_mut(), 4, 4, 6);
        assert_eq!(buf, [0, 0, 0, 0, 5, 0]);
    }
}

#[cfg(test)]
mod guard_world_tests {
    use super::*;

    /// Port rule (SPEC GAP): a soldier byte of slot 48 and up or past the 512-byte record reads 0.
    #[test]
    fn guard_sb_range() {
        let mut sol = vec![0u8; SLOTS * REC];
        sol[REC - 1] = 3;
        sol[REC] = 5;
        sol[(SLOTS - 1) * REC] = 6;
        let mut w: Box<Ja1World> = Box::new(unsafe { std::mem::zeroed() });
        w.soldiers = sol.as_mut_ptr();
        let h = Ja1Host { ctx: std::ptr::null_mut::<std::os::raw::c_void>(), random: None, call: None, host_ops: 0 };
        let wp: *mut Ja1World = &mut *w;
        let inv = Inv { w: wp, h: &h, a: Ai { w: wp, h: &h, s: 0 } };
        assert_eq!(inv.sb(0, REC - 1), 3);
        assert_eq!(inv.sb(0, REC), 0);
        assert_eq!(inv.sb(SLOTS - 1, 0), 6);
        assert_eq!(inv.sb(SLOTS, 0), 0);
    }

    /// Port rule (SPEC GAP): a slot outside 0..7 or a null armour pointer does nothing (0).
    #[test]
    fn guard_assign_item_press_args() {
        let mut sol = vec![0u8; SLOTS * REC];
        let mut w: Box<Ja1World> = Box::new(unsafe { std::mem::zeroed() });
        w.soldiers = sol.as_mut_ptr();
        let h = Ja1Host { ctx: std::ptr::null_mut::<std::os::raw::c_void>(), random: None, call: None, host_ops: 0 };
        assert_eq!(assign_item_press(&mut *w, &h, 0, std::ptr::null_mut()), 0);
        let mut ar = 0i32;
        assert_eq!(assign_item_press(&mut *w, &h, 8, &mut ar), 0);
    }
}
