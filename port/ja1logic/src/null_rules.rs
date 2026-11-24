//! Port rule tests (SPEC GAP, no original counterpart): a C entry point with a null world, host or buffer
//! returns at once. It calls no host op and changes no buffer. Ledger rows of the eq1118 check (Insert #12690
//! todo 1) that both screens marked "equivalent in production only": the front-end never passes a null pointer,
//! but the C interface takes any pointer.

#[cfg(test)]
use super::{Ja1Host, Ja1World};
use std::ffi::c_void;
use std::sync::atomic::{AtomicU32, Ordering};

const LEN: usize = 1 << 20;
const FIELDS: &[&str] = &["soldiers", "tiles", "item_types", "anim_table", "sector_state", "mission_done", "knowledge", "fresher", "last_seen_m", "last_seen_e", "calls", "paths", "dir_steps", "t0_class", "t1_class", "remember_m", "remember_e", "story", "plants", "crates", "weapons", "absorb", "anim_rows", "anim_scripts", "anim_free", "anim_sounds", "anim_steps_tile", "anim_turn_while", "supplies", "equip_skill", "attach", "ground_items", "notes", "move_codes", "held", "blast_list", "listed", "t2_class", "enemy_watch", "text_buf", "item_buf", "crate_state", "crate_shown", "fixed_items", "sectors", "scratch", "ap_warned", "splash", "fx_anims", "view_rays", "obj_block", "itq", "bled", "timed_fx", "sector_spoken", "silencer", "shot_sound", "impact_noise", "shot_range", "dmg_text", "posture", "window", "attack_sound", "diff_mod", "marks", "inv_cursor", "inv_swap", "inv_picked", "inv_give", "item_rects", "slot_rects", "accounts", "sector_order", "profiles", "stash_counts", "stat_progress", "greed", "stat_changed", "sector_replay", "guide_queue", "guard_eta", "guard_rec", "item_told", "gas_clouds", "no_break", "walter_said", "walter_next"];

unsafe extern "C" fn rnd(c: *mut c_void, _n: i32) -> i32 {
    (*(c as *const AtomicU32)).fetch_add(1, Ordering::SeqCst);
    0
}
unsafe extern "C" fn call(c: *mut c_void, _o: i32, _a: *const i32, _n: i32, _out: *mut u8, _sz: i32) -> u32 {
    (*(c as *const AtomicU32)).fetch_add(1, Ordering::SeqCst);
    0
}

/// A world whose every buffer is a zeroed 1 MB vector, except the buffers named in `null`.
struct W {
    bufs: Vec<Vec<u8>>,
    w: Box<Ja1World>,
    calls: Box<AtomicU32>,
    h: Ja1Host,
}

impl W {
    fn new(null: &[&str]) -> W {
        for n in null {
            assert!(FIELDS.contains(n), "no buffer {n}");
        }
        let mut bufs: Vec<Vec<u8>> = FIELDS.iter().map(|_| vec![0u8; LEN]).collect();
        let mut w: Box<Ja1World> = Box::new(unsafe { std::mem::zeroed() });
        {
            let mut b = bufs.iter_mut();
            w.soldiers = b.next().unwrap().as_mut_ptr();
            w.tiles = b.next().unwrap().as_mut_ptr();
            w.item_types = b.next().unwrap().as_mut_ptr() as *const u8;
            w.anim_table = b.next().unwrap().as_mut_ptr() as *const u8;
            w.sector_state = b.next().unwrap().as_mut_ptr();
            w.mission_done = b.next().unwrap().as_mut_ptr();
            w.knowledge = b.next().unwrap().as_mut_ptr();
            w.fresher = b.next().unwrap().as_mut_ptr() as *const u8;
            w.last_seen_m = b.next().unwrap().as_mut_ptr();
            w.last_seen_e = b.next().unwrap().as_mut_ptr();
            w.calls = b.next().unwrap().as_mut_ptr();
            w.paths = b.next().unwrap().as_mut_ptr();
            w.dir_steps = b.next().unwrap().as_mut_ptr() as *const u8;
            w.t0_class = b.next().unwrap().as_mut_ptr() as *const u8;
            w.t1_class = b.next().unwrap().as_mut_ptr() as *const u8;
            w.remember_m = b.next().unwrap().as_mut_ptr();
            w.remember_e = b.next().unwrap().as_mut_ptr();
            w.story = b.next().unwrap().as_mut_ptr();
            w.plants = b.next().unwrap().as_mut_ptr();
            w.crates = b.next().unwrap().as_mut_ptr() as *const u8;
            w.weapons = b.next().unwrap().as_mut_ptr() as *const u8;
            w.absorb = b.next().unwrap().as_mut_ptr() as *const u8;
            w.anim_rows = b.next().unwrap().as_mut_ptr() as *const u8;
            w.anim_scripts = b.next().unwrap().as_mut_ptr() as *const u8;
            w.anim_free = b.next().unwrap().as_mut_ptr() as *const u8;
            w.anim_sounds = b.next().unwrap().as_mut_ptr() as *const u8;
            w.anim_steps_tile = b.next().unwrap().as_mut_ptr() as *const u8;
            w.anim_turn_while = b.next().unwrap().as_mut_ptr() as *const u8;
            w.supplies = b.next().unwrap().as_mut_ptr();
            w.equip_skill = b.next().unwrap().as_mut_ptr() as *const u8;
            w.attach = b.next().unwrap().as_mut_ptr() as *const u8;
            w.ground_items = b.next().unwrap().as_mut_ptr();
            w.notes = b.next().unwrap().as_mut_ptr();
            w.move_codes = b.next().unwrap().as_mut_ptr();
            w.held = b.next().unwrap().as_mut_ptr();
            w.blast_list = b.next().unwrap().as_mut_ptr();
            w.listed = b.next().unwrap().as_mut_ptr() as *const u8;
            w.t2_class = b.next().unwrap().as_mut_ptr() as *const u8;
            w.enemy_watch = b.next().unwrap().as_mut_ptr();
            w.text_buf = b.next().unwrap().as_mut_ptr();
            w.item_buf = b.next().unwrap().as_mut_ptr();
            w.crate_state = b.next().unwrap().as_mut_ptr();
            w.crate_shown = b.next().unwrap().as_mut_ptr();
            w.fixed_items = b.next().unwrap().as_mut_ptr() as *const u8;
            w.sectors = b.next().unwrap().as_mut_ptr();
            w.scratch = b.next().unwrap().as_mut_ptr();
            w.ap_warned = b.next().unwrap().as_mut_ptr();
            w.splash = b.next().unwrap().as_mut_ptr() as *const u8;
            w.fx_anims = b.next().unwrap().as_mut_ptr() as *const u8;
            w.view_rays = b.next().unwrap().as_mut_ptr() as *const u8;
            w.obj_block = b.next().unwrap().as_mut_ptr() as *const u8;
            w.itq = b.next().unwrap().as_mut_ptr();
            w.bled = b.next().unwrap().as_mut_ptr();
            w.timed_fx = b.next().unwrap().as_mut_ptr();
            w.sector_spoken = b.next().unwrap().as_mut_ptr();
            w.silencer = b.next().unwrap().as_mut_ptr() as *const u8;
            w.shot_sound = b.next().unwrap().as_mut_ptr() as *const u8;
            w.impact_noise = b.next().unwrap().as_mut_ptr() as *const u8;
            w.shot_range = b.next().unwrap().as_mut_ptr() as *const u8;
            w.dmg_text = b.next().unwrap().as_mut_ptr();
            w.posture = b.next().unwrap().as_mut_ptr() as *const u8;
            w.window = b.next().unwrap().as_mut_ptr() as *const u8;
            w.attack_sound = b.next().unwrap().as_mut_ptr() as *const u8;
            w.diff_mod = b.next().unwrap().as_mut_ptr() as *const u8;
            w.marks = b.next().unwrap().as_mut_ptr();
            w.inv_cursor = b.next().unwrap().as_mut_ptr();
            w.inv_swap = b.next().unwrap().as_mut_ptr();
            w.inv_picked = b.next().unwrap().as_mut_ptr();
            w.inv_give = b.next().unwrap().as_mut_ptr();
            w.item_rects = b.next().unwrap().as_mut_ptr() as *const u8;
            w.slot_rects = b.next().unwrap().as_mut_ptr() as *const u8;
            w.accounts = b.next().unwrap().as_mut_ptr();
            w.sector_order = b.next().unwrap().as_mut_ptr();
            w.profiles = b.next().unwrap().as_mut_ptr();
            w.stash_counts = b.next().unwrap().as_mut_ptr();
            w.stat_progress = b.next().unwrap().as_mut_ptr();
            w.greed = b.next().unwrap().as_mut_ptr() as *const u8;
            w.stat_changed = b.next().unwrap().as_mut_ptr();
            w.sector_replay = b.next().unwrap().as_mut_ptr();
            w.guide_queue = b.next().unwrap().as_mut_ptr();
            w.guard_eta = b.next().unwrap().as_mut_ptr();
            w.guard_rec = b.next().unwrap().as_mut_ptr();
            w.item_told = b.next().unwrap().as_mut_ptr();
            w.gas_clouds = b.next().unwrap().as_mut_ptr();
            w.no_break = b.next().unwrap().as_mut_ptr() as *const u8;
            w.walter_said = b.next().unwrap().as_mut_ptr();
            w.walter_next = b.next().unwrap().as_mut_ptr();
        }
        for (i, n) in FIELDS.iter().enumerate() {
            if null.contains(n) {
                bufs[i] = Vec::new();
                set_null(&mut w, n);
            }
        }
        let calls = Box::new(AtomicU32::new(0));
        let h = Ja1Host { ctx: &*calls as *const AtomicU32 as *mut c_void, random: Some(rnd), call: Some(call), host_ops: 0 };
        W { bufs, w, calls, h }
    }
    fn wp(&mut self) -> *mut Ja1World {
        &mut *self.w
    }
    /// No host op, no random number, no buffer byte changed.
    fn untouched(&self) -> bool {
        self.calls.load(Ordering::SeqCst) == 0 && self.bufs.iter().all(|b| b.iter().all(|&x| x == 0))
    }
}

fn set_null(w: &mut Ja1World, n: &str) {
    match n {
        "soldiers" => w.soldiers = std::ptr::null_mut(),
        "tiles" => w.tiles = std::ptr::null_mut(),
        "item_types" => w.item_types = std::ptr::null(),
        "anim_table" => w.anim_table = std::ptr::null(),
        "sector_state" => w.sector_state = std::ptr::null_mut(),
        "mission_done" => w.mission_done = std::ptr::null_mut(),
        "knowledge" => w.knowledge = std::ptr::null_mut(),
        "fresher" => w.fresher = std::ptr::null(),
        "last_seen_m" => w.last_seen_m = std::ptr::null_mut(),
        "last_seen_e" => w.last_seen_e = std::ptr::null_mut(),
        "calls" => w.calls = std::ptr::null_mut(),
        "paths" => w.paths = std::ptr::null_mut(),
        "dir_steps" => w.dir_steps = std::ptr::null(),
        "t0_class" => w.t0_class = std::ptr::null(),
        "t1_class" => w.t1_class = std::ptr::null(),
        "remember_m" => w.remember_m = std::ptr::null_mut(),
        "remember_e" => w.remember_e = std::ptr::null_mut(),
        "story" => w.story = std::ptr::null_mut(),
        "plants" => w.plants = std::ptr::null_mut(),
        "crates" => w.crates = std::ptr::null(),
        "weapons" => w.weapons = std::ptr::null(),
        "absorb" => w.absorb = std::ptr::null(),
        "anim_rows" => w.anim_rows = std::ptr::null(),
        "anim_scripts" => w.anim_scripts = std::ptr::null(),
        "anim_free" => w.anim_free = std::ptr::null(),
        "anim_sounds" => w.anim_sounds = std::ptr::null(),
        "anim_steps_tile" => w.anim_steps_tile = std::ptr::null(),
        "anim_turn_while" => w.anim_turn_while = std::ptr::null(),
        "supplies" => w.supplies = std::ptr::null_mut(),
        "equip_skill" => w.equip_skill = std::ptr::null(),
        "attach" => w.attach = std::ptr::null(),
        "ground_items" => w.ground_items = std::ptr::null_mut(),
        "notes" => w.notes = std::ptr::null_mut(),
        "move_codes" => w.move_codes = std::ptr::null_mut(),
        "held" => w.held = std::ptr::null_mut(),
        "blast_list" => w.blast_list = std::ptr::null_mut(),
        "listed" => w.listed = std::ptr::null(),
        "t2_class" => w.t2_class = std::ptr::null(),
        "enemy_watch" => w.enemy_watch = std::ptr::null_mut(),
        "text_buf" => w.text_buf = std::ptr::null_mut(),
        "item_buf" => w.item_buf = std::ptr::null_mut(),
        "crate_state" => w.crate_state = std::ptr::null_mut(),
        "crate_shown" => w.crate_shown = std::ptr::null_mut(),
        "fixed_items" => w.fixed_items = std::ptr::null(),
        "sectors" => w.sectors = std::ptr::null_mut(),
        "scratch" => w.scratch = std::ptr::null_mut(),
        "ap_warned" => w.ap_warned = std::ptr::null_mut(),
        "splash" => w.splash = std::ptr::null(),
        "fx_anims" => w.fx_anims = std::ptr::null(),
        "view_rays" => w.view_rays = std::ptr::null(),
        "obj_block" => w.obj_block = std::ptr::null(),
        "itq" => w.itq = std::ptr::null_mut(),
        "bled" => w.bled = std::ptr::null_mut(),
        "timed_fx" => w.timed_fx = std::ptr::null_mut(),
        "sector_spoken" => w.sector_spoken = std::ptr::null_mut(),
        "silencer" => w.silencer = std::ptr::null(),
        "shot_sound" => w.shot_sound = std::ptr::null(),
        "impact_noise" => w.impact_noise = std::ptr::null(),
        "shot_range" => w.shot_range = std::ptr::null(),
        "dmg_text" => w.dmg_text = std::ptr::null_mut(),
        "posture" => w.posture = std::ptr::null(),
        "window" => w.window = std::ptr::null(),
        "attack_sound" => w.attack_sound = std::ptr::null(),
        "diff_mod" => w.diff_mod = std::ptr::null(),
        "marks" => w.marks = std::ptr::null_mut(),
        "inv_cursor" => w.inv_cursor = std::ptr::null_mut(),
        "inv_swap" => w.inv_swap = std::ptr::null_mut(),
        "inv_picked" => w.inv_picked = std::ptr::null_mut(),
        "inv_give" => w.inv_give = std::ptr::null_mut(),
        "item_rects" => w.item_rects = std::ptr::null(),
        "slot_rects" => w.slot_rects = std::ptr::null(),
        "accounts" => w.accounts = std::ptr::null_mut(),
        "sector_order" => w.sector_order = std::ptr::null_mut(),
        "profiles" => w.profiles = std::ptr::null_mut(),
        "stash_counts" => w.stash_counts = std::ptr::null_mut(),
        "stat_progress" => w.stat_progress = std::ptr::null_mut(),
        "greed" => w.greed = std::ptr::null(),
        "stat_changed" => w.stat_changed = std::ptr::null_mut(),
        "sector_replay" => w.sector_replay = std::ptr::null_mut(),
        "guide_queue" => w.guide_queue = std::ptr::null_mut(),
        "guard_eta" => w.guard_eta = std::ptr::null_mut(),
        "guard_rec" => w.guard_rec = std::ptr::null_mut(),
        "item_told" => w.item_told = std::ptr::null_mut(),
        "gas_clouds" => w.gas_clouds = std::ptr::null_mut(),
        "no_break" => w.no_break = std::ptr::null(),
        "walter_said" => w.walter_said = std::ptr::null_mut(),
        "walter_next" => w.walter_next = std::ptr::null_mut(),
        _ => unreachable!(),
    }
}

use super::guard::Ja1GuardScreen;

/// Every entry point with a null world and a working host: no host call, the guard's answer.
#[test]
fn null_world_entries_do_nothing() {
    let calls = Box::new(AtomicU32::new(0));
    let h = Ja1Host { ctx: &*calls as *const AtomicU32 as *mut c_void, random: Some(rnd), call: Some(call), host_ops: 0 };
    let n = std::ptr::null_mut::<Ja1World>();
    let mut out = [0i16; 64];
    let mut g: Box<Ja1GuardScreen> = Box::new(unsafe { std::mem::zeroed() });
    let mut hinted = 0u8;
    assert_eq!(super::ja1_ai_sector_value(n, &h, 0, 0), -100);
    super::battle::ja1_resolve_battle(n, &h, 0, 0);
    super::battle::ja1_sector_gained(n, &h, 0);
    assert_eq!(super::daystart::ja1_day_start(n, &h), 0);
    super::doors::ja1_break_object(n, &h, 0, 0, 0);
    super::doors::ja1_close_door(n, &h, 0, 0, 0);
    super::duties::ja1_morning_duties(n, &h);
    super::ending::ja1_win_vars(n, &h, out.as_mut_ptr());
    assert_eq!(super::ending::ja1_performance_rating(n, &h), 0);
    assert_eq!(super::enemy::ja1_enemy_pick(n, &h), 0);
    assert_eq!(super::enemy::ja1_enemy_move(n, &h, 0), 0);
    super::guard::ja1_guard_close(n, &h, &mut *g);
    super::guide::ja1_guide_advice(n, &h);
    super::morning::ja1_duty_wake(n, &h);
    super::morning::ja1_profile_sync(n, &h);
    super::replay::ja1_day_replay(n, &h);
    super::stats::ja1_stat_changes(n, &h);
    super::storyday::ja1_story_day(n, &h);
    super::storyfind::ja1_story_find(n, &h);
    super::storyfind::ja1_story_hit(n, &h);
    super::stratopen::ja1_strategic_open(n, &h);
    super::sunrise::ja1_sunrise(n, &h);
    super::update::ja1_consume_trigger(n, &h, 0);
    assert_eq!(super::verdict::ja1_day_verdict(n, &h), 0);
    super::verdict::ja1_day_bonus(n, &h);
    assert_eq!(super::verdict::ja1_day_end(n, &h), 0);
    assert_eq!(super::clock::ja1_day_clock(n, &h, 0), 0);
    assert_eq!(super::mapclick::ja1_map_start(n, 0, &mut hinted), 0);
    assert_eq!(calls.load(Ordering::SeqCst), 0);
    assert_eq!(out, [0i16; 64]);
    assert_eq!(hinted, 0);
}

/// A good world and host with a null result buffer (win_vars) or flag (map_start).
#[test]
fn null_out_does_nothing() {
    let mut x = W::new(&[]);
    let wp = x.wp();
    super::ending::ja1_win_vars(wp, &x.h, std::ptr::null_mut());
    assert!(x.untouched());
    assert_eq!(super::mapclick::ja1_map_start(wp, 1, std::ptr::null_mut()), 0);
    assert!(x.untouched());
}

/// A good world and host with a null guard screen.
#[test]
fn null_guard_screen_does_nothing() {
    let mut x = W::new(&[]);
    let wp = x.wp();
    super::guard::ja1_guard_close(wp, &x.h, std::ptr::null_mut());
    assert!(x.untouched());
}

/// One null buffer that the entry needs: the entry returns at once.
#[test]
fn one_null_buffer_does_nothing() {
    type E = fn(*mut Ja1World, *const Ja1Host) -> i32;
    let cases: &[(&str, &[&str], E)] = &[
        ("aim_open", &["profiles"], |w, h| super::aimhire::ja1_aim_open(w, h)),
        ("complain_pick", &["soldiers"], |w, h| { super::complain::ja1_complain_pick(w, h); 0 }),
        ("hated_check", &["soldiers"], |w, h| { super::complain::ja1_hated_check(w, h); 0 }),
        ("corpse_export soldiers", &["soldiers"], |w, h| { super::corpse::ja1_corpse_export(w, h); 0 }),
        ("corpse_export profiles", &["profiles"], |w, h| { super::corpse::ja1_corpse_export(w, h); 0 }),
        ("day_start", &["accounts"], |w, h| super::daystart::ja1_day_start(w, h)),
        ("fire_merc_core", &["soldiers"], |w, h| { super::dismiss::ja1_fire_merc_core(w, h, 0, 0, 0); 0 }),
        ("fire_merc_core profiles", &["profiles"], |w, h| { super::dismiss::ja1_fire_merc_core(w, h, 0, 0, 0); 0 }),
        ("guard_close soldiers", &["soldiers"], |w, h| { let mut g: Ja1GuardScreen = unsafe { std::mem::zeroed() }; super::guard::ja1_guard_close(w, h, &mut g); 0 }),
        ("guard_close sector_state", &["sector_state"], |w, h| { let mut g: Ja1GuardScreen = unsafe { std::mem::zeroed() }; super::guard::ja1_guard_close(w, h, &mut g); 0 }),
        ("merc_join", &["soldiers"], |w, h| super::join::ja1_merc_join(w, h, 1)),
        ("map_click", &["accounts"], |w, h| { let mut warned = 0u8; super::mapclick::ja1_map_click(w, h, 1, 0, 1, &mut warned) }),
        ("map_start", &["accounts"], |w, _h| { let mut hinted = 0u8; super::mapclick::ja1_map_start(w, 1, &mut hinted) }),
        ("profile_sync", &["profiles"], |w, h| { super::morning::ja1_profile_sync(w, h); 0 }),
        ("raise_salary", &["soldiers"], |w, h| { super::raise::ja1_raise_salary(w, h); 0 }),
        ("day_replay", &["sector_replay"], |w, h| { super::replay::ja1_day_replay(w, h); 0 }),
    ];
    for (name, null, f) in cases {
        let mut x = W::new(null);
        let wp = x.wp();
        let r = f(wp, &x.h);
        let want = match *name { "aim_open" => -1, "merc_join" => -1, _ => 0 };
        assert_eq!(r, want, "{name}");
        assert!(x.untouched(), "{name}");
    }
}

/// Every entry point with a good world and a null host: nothing changes in the world.
#[test]
fn null_host_entries_do_nothing() {
    let mut x = W::new(&[]);
    let w = x.wp();
    let n = std::ptr::null::<Ja1Host>();
    let mut out = [7i16; 64];
    let mut g: Box<Ja1GuardScreen> = Box::new(unsafe { std::mem::zeroed() });
    assert_eq!(super::ja1_ai_sector_value(w, n, 0, 0), -100);
    super::battle::ja1_resolve_battle(w, n, 1, 0);
    super::battle::ja1_sector_gained(w, n, 1);
    assert_eq!(super::daystart::ja1_day_start(w, n), 0);
    super::doors::ja1_break_object(w, n, 300, 0x3E, 0);
    super::doors::ja1_close_door(w, n, 300, 0x3E, 0);
    super::ending::ja1_win_vars(w, n, out.as_mut_ptr());
    assert_eq!(super::enemy::ja1_enemy_pick(w, n), 0);
    assert_eq!(super::enemy::ja1_enemy_move(w, n, 0), 0);
    super::guard::ja1_guard_close(w, n, &mut *g);
    super::guide::ja1_guide_advice(w, n);
    super::replay::ja1_day_replay(w, n);
    super::storyday::ja1_story_day(w, n);
    super::storyfind::ja1_story_find(w, n);
    super::storyfind::ja1_story_hit(w, n);
    super::stratopen::ja1_strategic_open(w, n);
    super::sunrise::ja1_sunrise(w, n);
    super::update::ja1_consume_trigger(w, n, 0);
    assert_eq!(super::verdict::ja1_day_verdict(w, n), 0);
    super::verdict::ja1_day_bonus(w, n);
    assert_eq!(super::verdict::ja1_day_end(w, n), 0);
    assert_eq!(super::clock::ja1_day_clock(w, n, 60), 0);
    assert!(x.untouched());
    assert_eq!(out, [7i16; 64]);
}

/// win_vars with a null world writes nothing into `out`.
#[test]
fn win_vars_null_world_keeps_out() {
    let calls = Box::new(AtomicU32::new(0));
    let h = Ja1Host { ctx: &*calls as *const AtomicU32 as *mut c_void, random: Some(rnd), call: Some(call), host_ops: 0 };
    let mut out = [7i16; 4];
    super::ending::ja1_win_vars(std::ptr::null_mut(), &h, out.as_mut_ptr());
    assert_eq!(out, [7i16; 4]);
}

/// Move codes: one null class table gives `previous`; one null table in the rebuild writes nothing.
#[test]
fn move_code_null_tables() {
    let gc = [1u8; 256];
    let oc = [1u8; 256];
    assert_eq!(crate::ja1_move_code(1, 1, 0, 0x1234, gc.as_ptr(), std::ptr::null()), 0x1234);
    assert_eq!(crate::ja1_move_code(1, 1, 0, 0x1234, std::ptr::null(), oc.as_ptr()), 0x1234);
    let tiles = vec![1u8; 0x10000 * 8];
    let mut mc = vec![0x5555u16; 0x10000];
    let p = mc.as_mut_ptr();
    let n = std::ptr::null::<u8>();
    crate::ja1_rebuild_move_codes(n, gc.as_ptr(), oc.as_ptr(), p);
    crate::ja1_rebuild_move_codes(tiles.as_ptr(), n, oc.as_ptr(), p);
    crate::ja1_rebuild_move_codes(tiles.as_ptr(), gc.as_ptr(), n, p);
    assert!(mc.iter().all(|&v| v == 0x5555));
}

/// Entries that check several buffers: each buffer null alone stops the entry.
#[test]
fn each_null_buffer_does_nothing() {
    type E = fn(*mut Ja1World, *const Ja1Host) -> i32;
    let cases: &[(&[&str], i32, E)] = &[
        (&["soldiers", "profiles", "accounts"], -1, |w, h| super::aimhire::ja1_aim_open(w, h)),
        (&["soldiers", "profiles", "accounts"], 0, |w, h| { super::complain::ja1_complain(w, h, 0); 0 }),
        (&["soldiers", "profiles"], 0, |w, h| { super::complain::ja1_hated_check(w, h); 0 }),
        (&["soldiers", "profiles", "accounts"], 0, |w, h| { super::corpse::ja1_corpse_export(w, h); 0 }),
        (&["soldiers", "profiles", "accounts"], 0, |w, h| { super::dismiss::ja1_fire_merc_core(w, h, 0, 0, 0); 0 }),
        (&["soldiers", "profiles", "stat_progress", "item_types"], -1, |w, h| super::join::ja1_merc_join(w, h, 1)),
        (&["soldiers", "profiles"], 0, |w, h| { super::morning::ja1_profile_sync(w, h); 0 }),
        (&["soldiers", "profiles", "accounts"], 0, |w, h| { super::raise::ja1_raise_salary(w, h); 0 }),
        (&["soldiers", "guard_eta", "guard_rec", "sector_state"], 0, |w, h| {
            let mut g: Ja1GuardScreen = unsafe { std::mem::zeroed() };
            super::guard::ja1_guard_close(w, h, &mut g);
            0
        }),
        (&["sector_state", "accounts"], 0, |w, h| { let mut warned = 0u8; super::mapclick::ja1_map_click(w, h, 1, 0, 1, &mut warned) }),
    ];
    for (k, (bufs, want, f)) in cases.iter().enumerate() {
        for b in bufs.iter() {
            let mut x = W::new(&[b]);
            let wp = x.wp();
            assert_eq!(f(wp, &x.h), *want, "case {k} null {b}");
            assert!(x.untouched(), "case {k} null {b}");
        }
    }
}

/// Port rules (SPEC GAP) of single entries. Ledger rows c2aea50:ja1logic:8104 (trigger index), 15169 (replay
/// copy), 3254 (edge tile), 16604 (guard click buttons).
#[test]
fn entry_range_rules() {
    // Consume trigger: an index outside 0..9 does nothing (update.rs trigger_tile_ptr).
    let mut x = W::new(&[]);
    let wp = x.wp();
    super::update::ja1_consume_trigger(wp, &x.h, 10);
    super::update::ja1_consume_trigger(wp, &x.h, -1);
    assert!(x.untouched());
    // A null sectors buffer with a good index does nothing either.
    let mut x = W::new(&["sectors"]);
    let wp = x.wp();
    super::update::ja1_consume_trigger(wp, &x.h, 0);
    assert!(x.untouched());
    // Day start: a null sector table skips the replay copy, the replay copy keeps its bytes.
    let mut x = W::new(&["sector_state"]);
    let i = FIELDS.iter().position(|f| *f == "sector_replay").unwrap();
    x.bufs[i].iter_mut().for_each(|b| *b = 0x11);
    let wp = x.wp();
    // The host answers yes to the day question (op DAY_ASK), so the day starts.
    unsafe extern "C" fn yes(_c: *mut c_void, _o: i32, _a: *const i32, _n: i32, _out: *mut u8, _sz: i32) -> u32 {
        1
    }
    x.h.call = Some(yes);
    assert_eq!(super::daystart::ja1_day_start(wp, &x.h), 1);
    assert!(x.bufs[i].iter().all(|&b| b == 0x11));
    // Edge tile: a null world gives −1.
    let calls = AtomicU32::new(0);
    let h = Ja1Host { ctx: &calls as *const AtomicU32 as *mut c_void, random: Some(rnd), call: Some(call), host_ops: 0 };
    assert_eq!(crate::sector::ja1_edge_tile(std::ptr::null_mut(), &h, 0), -1);
    // Guard click: no left or right button (bits 0 and 1) does nothing.
    let mut x = W::new(&[]);
    let wp = x.wp();
    let mut g: Box<Ja1GuardScreen> = Box::new(unsafe { std::mem::zeroed() });
    let before: Vec<u8> = unsafe { std::slice::from_raw_parts(&*g as *const Ja1GuardScreen as *const u8, std::mem::size_of::<Ja1GuardScreen>()) }.to_vec();
    assert_eq!(super::guard::ja1_guard_click(wp, &x.h, &mut *g, 1, 4, 0), 0);
    let after = unsafe { std::slice::from_raw_parts(&*g as *const Ja1GuardScreen as *const u8, std::mem::size_of::<Ja1GuardScreen>()) };
    assert_eq!(before, after);
    assert!(x.untouched());
}

/// Entries that check several buffers, run on a world with one merc in the sector (slot 0, HP 1, profile 0):
/// one null buffer alone still stops the entry, so a host op or a buffer write cannot come from the merc.
/// Ledger rows ja1logic:14568 (hated_check), 15032 (raise_salary), 16558, 16562, 16613 (guard_close).
#[test]
fn null_buffer_with_merc_does_nothing() {
    type E = fn(*mut Ja1World, *const Ja1Host) -> i32;
    let cases: &[(&[&str], E)] = &[
        (&["soldiers", "profiles"], |w, h| { super::complain::ja1_hated_check(w, h); 0 }),
        (&["soldiers", "profiles", "accounts"], |w, h| { super::raise::ja1_raise_salary(w, h); 0 }),
        (&["soldiers", "guard_eta", "guard_rec"], |w, h| {
            let mut g: Ja1GuardScreen = unsafe { std::mem::zeroed() };
            g.moves_n[0] = 1; // one guard move into sector 0
            super::guard::ja1_guard_close(w, h, &mut g);
            0
        }),
    ];
    let s = FIELDS.iter().position(|f| *f == "soldiers").unwrap();
    for (k, (bufs, f)) in cases.iter().enumerate() {
        for b in bufs.iter() {
            let mut x = W::new(&[b]);
            if !x.bufs[s].is_empty() {
                x.bufs[s][0x182] = 1; // in the current sector
                x.bufs[s][0x183] = 1; // HP
            }
            let snap = x.bufs.clone();
            let wp = x.wp();
            f(wp, &x.h);
            assert_eq!(x.calls.load(Ordering::SeqCst), 0, "case {k} null {b}");
            assert!(x.bufs == snap, "case {k} null {b}");
        }
    }
}

/// New game with a difficulty above 2 (a free u8 in the save): the old sector count stays, but the cap of 8
/// (newgame.rs, `n > 8`) still holds. Ledger row ja1logic:17546.
#[test]
fn new_game_caps_sector_count() {
    let mut x = W::new(&[]);
    let st = FIELDS.iter().position(|f| *f == "sector_state").unwrap();
    let sec = FIELDS.iter().position(|f| *f == "sectors").unwrap();
    x.bufs[st][1] = 20; // sector 0, byte 1
    x.w.difficulty = 3;
    let wp = x.wp();
    super::newgame::ja1_new_game(wp, &x.h);
    assert_eq!(x.bufs[st][1], 8);
    assert_eq!(x.bufs[sec][0x96 + 7], 1);
}

/// sector.rs null rules (SPEC GAP) on a world with data: a null class table skips the move code rebuild of a
/// ground item take, and a null soldiers buffer stops slot reset and guard entry. Ledger rows ja1logic:2257,
/// 2258 (rebuild_move_codes), 1664 (soldiers_ok), 3235 (guard_enter).
#[test]
fn sector_null_rules_with_data() {
    let t = FIELDS.iter().position(|f| *f == "tiles").unwrap();
    let mc = FIELDS.iter().position(|f| *f == "move_codes").unwrap();
    for null in ["t0_class", "t1_class"] {
        let mut x = W::new(&[null]);
        x.bufs[t][500 * 8 + 6] = 3; // ground item list 3
        x.bufs[t][500 * 8 + 1] = 0x86; // the item object
        let wp = x.wp();
        crate::sector::ja1_take_ground_item(wp, &x.h, 500, 0);
        assert!(x.bufs[mc].iter().all(|&b| b == 0), "null {null}");
    }
    let mut x = W::new(&["soldiers"]);
    let wp = x.wp();
    crate::sector::ja1_slot_reset(wp, &x.h, 0, 0);
    assert_eq!(crate::sector::ja1_guard_enter(wp, &x.h, 0), 0);
    assert!(x.untouched());
}

/// Move code rebuilds after an object change, with one null class table (SPEC GAP: skip the rebuild): mine flag
/// (op 118), disarm of a trap under a flag (op 248, step 7 kind 0x13) and an effect hit on a flag tile.
/// Ledger rows ja1logic:18833 (mine_flag), 18978 (remove_flag), c2aea50:ja1logic:18399 + 18401 (blast).
#[test]
fn flag_rebuild_null_tables() {
    let t = FIELDS.iter().position(|f| *f == "tiles").unwrap();
    let mc = FIELDS.iter().position(|f| *f == "move_codes").unwrap();
    let ge = FIELDS.iter().position(|f| *f == "ground_items").unwrap();
    let s = FIELDS.iter().position(|f| *f == "soldiers").unwrap();
    for null in ["t0_class", "t1_class", "move_codes"] {
        // Mine flag.
        let mut x = W::new(&[null]);
        let wp = x.wp();
        super::usetile::ja1_mine_flag(wp, &x.h, 0, 100);
        assert_eq!(x.bufs[t][100 * 8 + 1], 0x86, "mine null {null}");
        if null != "move_codes" {
            assert!(x.bufs[mc].iter().all(|&b| b == 0), "mine null {null}");
        }
        // Disarm: trap 0x13 at power -50 under a flag, merc skill 100 (chance 50 > random 0).
        let mut x = W::new(&[null]);
        x.bufs[t][100 * 8 + 1] = 0x86;
        x.bufs[ge][0] = 0x13;
        x.bufs[ge][2] = (-50i8) as u8;
        x.bufs[s][0x18F] = 100;
        let wp = x.wp();
        super::usetile::ja1_disarm(wp, &x.h, 0, 100, 0, 0, 0);
        assert_eq!(x.bufs[t][100 * 8 + 1], 0, "disarm null {null}");
        if null != "move_codes" {
            assert!(x.bufs[mc].iter().all(|&b| b == 0), "disarm null {null}");
        }
        // Effect hit on the flag tile.
        let mut x = W::new(&[null]);
        x.bufs[t][100 * 8 + 1] = 0x86;
        let wp = x.wp();
        super::blast::ja1_effect_hit(wp, &x.h, 100, 0, 0);
        assert_eq!(x.bufs[t][100 * 8 + 1], 0, "effect null {null}");
        if null != "move_codes" {
            assert!(x.bufs[mc].iter().all(|&b| b == 0), "effect null {null}");
        }
    }
}

/// A.I.M. refusal: the day of the accounts is a 32-bit value (two words). On day 0x10000 a merc free from day 5
/// is free; the next check (max fatal rate 0) then gives reason 2, not reason 1 (busy). Ledger row
/// c2aea50:ja1logic:19952 (acc32).
#[test]
fn aim_refusal_day_high_word() {
    let mut x = W::new(&[]);
    let pr = FIELDS.iter().position(|f| *f == "profiles").unwrap();
    let ac = FIELDS.iter().position(|f| *f == "accounts").unwrap();
    x.bufs[pr][0x3B] = 60; // hates nobody
    x.bufs[pr][0x4D] = 60;
    x.bufs[pr][0x35] = 5; // free from day 5
    x.bufs[ac][4..8].copy_from_slice(&0x10000u32.to_le_bytes());
    let wp = x.wp();
    assert_eq!(super::aimhire::ja1_aim_refusal(wp, &x.h, 0), 2);
}

/// Tile patch kind 4 (DAT\XRA.DAT, fixed name): a host answer other than −1 opens the patch, so the patch runs.
/// Ledger row c2aea50:ja1logic:2459 (open_patch kind 4).
#[test]
fn tile_patch_kind_4_opens() {
    let mut x = W::new(&[]);
    let wp = x.wp();
    assert_ne!(crate::sector::ja1_tile_patch(wp, &x.h, 4, 0, 0, 0, 0), -1);
}
