//! Fan patch 1.15m fixes (#70; unofficial patch, third_party/ja115m/): the player toggle `fan_fixes` (ja1.cfg,
//! JA1_FAN_FIXES). Off by default: the port stays 1:1 with 1.13b. Each fix has an id `Fnn` = bit nn-1 of the mask, so
//! a test can turn on one fix alone. No fix changes the save format.
//!
//! The table of #70 lists each fix row with its id and its proof (diff / live / code).
//!
//! #71 (Insert #13402): one setting `fixes = off | 1.15m | all` (ja1.cfg `fixes`, JA1_FIXES) replaces `fan_fixes`.
//! `off` = 1.13b, `1.15m` = every F id, `all` = every F id + every port fix `Pnn` (our own fixes for the #2 bugs that
//! 1.15m leaves; bit nn-1 of the port mask). JA1_FAN_FIXES (F ids only) stays for the tests.
//!
//! Credit (#70, Insert #13185): our own code, written from the 1.15m changelog and checked against its exe. DarzaR
//! (author of the unofficial 1.15 patch) found and documented the bugs; the 1.15m release fixes bugs that
//! MitchShudlem found. Each fix site names its changelog version and item and who found it.
use std::sync::atomic::{AtomicU64, Ordering};

/// The fixes that exist (each fix row adds its bit). `on` / "1" turns on all of them.
pub const ALL: u128 = u128::MAX; // F01..F128 (ids F01..F70 exist; the mask has room up to F128)
/// The highest fix id the mask holds.
pub const MAX_ID: u32 = 128;
// F01 XRA.DAT spawn data (sector.rs XRA_F01; changelog 1.15x, found by DarzaR).
// F02 explosives training by the explosives skill (duties.rs; changelog 1.15z, found by DarzaR).
// F03 health step of a merc below 15 health at day end (stats.rs; changelog 1.15z, found by DarzaR).
// F05 no guard send to a sector while guards walk there (guard.rs click; changelog 1.15 jan 08, found by DarzaR).
// F04 merc message text with the name past place 127 (ja1front balloon.rs str_replace; changelog 1.15l, found by DarzaR).
// F06 bandages lower the wound penalty of max AP and of the gun, knife and throw chances (strength.rs max_ap, aim.rs,
//     attack.rs skill + throw_chance; 1.15m switch HEALTHY bit set; changelog 1.15 jan 08, found by DarzaR).
// F07 sight through windows the same from every side (sight.rs: window step limit + window_byte; changelog 1.15z,
//     found by DarzaR).
// F08 a small blast off the merc's tile lets a merc at 0 health fall (damage.rs take_damage; changelog 1.15m, found by
//     MitchShudlem, fixed by DarzaR).
// F09 the fall picture below 15 health, so a merc who collapses from bleeding falls (damage.rs take_damage;
//     changelog 1.15z, found by DarzaR).
// F10 the knock-down flag of a leg hit is cleared at the end of each damage (damage.rs take_damage; changelog 1.15l,
//     found by DarzaR).
// F11 a crouching merc knocked down by a leg hit falls at once (damage.rs take_damage: ReplaceSoldier first; changelog
//     1.15m, found by MitchShudlem, fixed by DarzaR).
// F12 first aid cancelled during the kneel-down picture stops that picture (firstaid.rs end_task: SetAnim ECX 1;
//     changelog 1.15m, found by MitchShudlem, fixed by DarzaR).
// F13 a merc moved by swap-by-X stands on its new tile's centre point with its step state reset (turns.rs place_at:
//     SnapToTile; changelog 1.15m, found by MitchShudlem, fixed by DarzaR).
// F14 the busy display of a walk ends at the goal tile even when an interrupt selected another merc (update.rs
//     walk_end passes -1 to StopWalk; ja1front strips.rs stop_walk; changelog 1.15z, found by DarzaR).
// F15 an enemy that bled at the start of the enemies' turn and is down (+0x186 > 0) gets the view slid to it, so its
//     death fall is seen; its cry also plays with see-all (turns.rs enemies_turn step 6, host op 357 slide_view;
//     changelog 1.15z "death animation for enemies dying from bleeding", found by DarzaR).
// F16 the enemy attack on the south processing plant (story byte 0x0D == 1) is not held back by its water sides: the
//     plant counts as an open side and the attackers' weight takes the target sector (enemy.rs enemy_move; changelog
//     1.15z, found by DarzaR).
// F17 an opponent at 0 health that is still down (+0x186 > 0) counts in Sight check, Mark seen and Guard turned
//     (sightings.rs sg_opp_in_play; 1.15m SightCheck 0x72FCD, MarkSeen 0x73360, 0x7351C; #70 row 9, changelog 1.15z
//     "last acting enemy noticed as unseen attacker ... lockup", found by DarzaR; row match by function, see #70).
// F18 timed effects count down by the tick step (10, or 1000 with time compression on), and each effect that goes
//     off turns time compression off (update.rs ticks step 3 + fuses, turns.rs end_turn step 1; 1.15m
//     TacticalTimer? 0x5F216 helper 0x3D4D5, TacticalLoop 0x1125D, EndTurn? 0x1220D; #70 row 69, changelog 1.15l
//     "explosives set before time compression", found by DarzaR).
// F19 New turn raises the sector-0 stage only when Santino's spoke byte is 2 (set by the loop pass after his quote),
//     not already at 1: at 1 Explode dat still skips the cache tiles, so the stage blasts there did nothing and
//     Santino stayed alive (turns.rs new_turn step 20; 1.15m NewTurn? 0x12B50; #70 row 10, changelog 1.15z
//     "Santino's attempt to detonate his trap during Enemy turn", found by DarzaR).
// F20 New turn handles redraw_next (guards or enemies arrived) before its step 8, not at its end: in combat the kept
//     turn start runs in the same New turn (no second New turn, so no turn is lost); out of combat redraw_pending = 1
//     and the turn state is handed over as after a queued interrupt (turns.rs new_turn; 1.15m NewTurn? 0x127E2 ->
//     0x12BEB; #70 row 33, changelog 1.15y "skip a whole turn", found by DarzaR).
// F21 the retreat risk at day end: a sector with a way out home now lowers the risk (30 x (2 x unowned - way out))
//     instead of raising it (leave.rs retreat step 2; 1.15m MsgRetreatCasualty? 0x30B28, 0x30C4C; #70 row 16,
//     changelog 1.15z, found by DarzaR).
// F22 a sector taken with no battle at the day-end leave (no enemies left, not yet owned) gets the territory recount
//     at once, as a sector won by a battle already did (leave.rs leave_sector step 5; 1.15m LeaveSectorCleanup?
//     0x30F7D -> cave 0x4889B = SectorGained? 0x3FA54 + RecalcTerritory 0x50EA8; #70 row 15, changelog 1.15z
//     "wrong conditions for determining a need to retreat at day's end", found by DarzaR).
// F23 Anim step: a soldier under 15 health (signed byte) whose animation is not 5 no longer gets the stand animation
//     4 (anim.rs anim_step step 3; 1.15m AnimStep 0x1F3A9 / 0x1F404; #70 row 11 part, changelog 1.15z "falling and
//     rising animations", found by DarzaR). Anim frame (1.15m AnimFrame 0x18E35..0x192CC): word 0x1E5 tests only the tile
//     behind and steps 1 px forward when it is free; word 0x1E6 steps 2 px after the turn ends and 1 px back when the tile
//     ahead is free; a diagonal pixel step is s + s/2 on x (north-east: s + 2*(s/2) when >= 0) and s on y (anim.rs
//     anim_frame).
// F24 Step start: the low-AP stop compares the step AP with the AP minus the reserve as signed bytes, so a merc with
//     fewer AP than the reserve stops too (update.rs u_step_start step 4; 1.15m StepStart 0x1A587 jbe -> jle; #70
//     row 58 part, changelog 1.15l "reserve points enabling players to continue movement", found by DarzaR).
// F25 Step start: a merc who spots a mine on the next tile, when that tile is not in the blast list, gets two
//     practice rolls (stats 4 and 6, |ground byte 2| / 2 rolls, flag 0) before the trigger message (update.rs
//     u_step_start step 6; 1.15m StepStart 0x1A757 block; #70 row 74 part, changelog 1.15l "trap detection now
//     gives one", found by DarzaR).
// F26 Recalc territory: in an owned sector connected home, the free trees (b6 - b8) are first capped by the workable
//     count (signed byte compare), then by tappers / 3, so trees drawn as not workable bring no income (ledger.rs
//     recalc_territory; 1.15m RecalcTerritory 0x50F75 + helper 0x62356; #70 row 84, changelog 1.15 jan13 "money income
//     from them still happened", found by DarzaR).
// F27 Sector battle show: the last key byte is cleared after the first refresh, so an Escape pressed before the show
//     no longer makes the first mouse click skip the whole battle table (battle.rs resolve_battle step 1, battle_ui
//     kind 9; ja1front clears LAST_KEY; 1.15m ResolveSectorBattle 0x3E54D; #70 row 22, changelog 1.15z, found by DarzaR).
// F28 Leave path at day end: the cleanup (with the battle show of guards and enemies still in the sector) runs before
//     the fade out and the End of day screen, so that show is no longer played on a black screen (leave.rs
//     leave_path; 1.15m TacticalLoop 0x11358..0x113EA; #70 row 17, changelog 1.15z "events of a battle between them
//     weren't shown on a screen", found by DarzaR).
// F29 Interrupt inside an interrupt: NextTurn and the merc interrupt turn first put the cursor item back; a merc in
//     state 8 gets +0x194 = +0x1B0; an interrupter byte above 7 (signed) ends the interrupt turn with slot 0x40; the
//     shown turn flag 0x92DCA is set (turns.rs next_turn + interrupt_turn; 1.15m NextTurn? 0x12D55, InterruptTurn?
//     0x130C7..0x1314F, cave 0x8AF04; #70 row 18, changelog 1.15z "more complete fix for interrupts", found by DarzaR).
// F30 Knife miss: the target's practice roll (stat 2) after a missed stab comes only when an enemy (slot >= 0x28)
//     attacked, not a merc or guard of his own side (aim.rs melee_hit step 5; 1.15m MeleeHit? 0x35517; #70 row 20,
//     changelog 1.15z, found by DarzaR).
// F31 New game after a game: the found flags of the 15 one-off notes are cleared and the listening device line
//     sector is 99 again, so the next game places the notes again (ja1front world.rs new_game_after; 1.15m
//     NewGameInit 0x37B5A, cave 0x18FE9; #70 row 21, changelog 1.15z, found by DarzaR).
// F32 Shot in flight: a merc does not fire while something still animates (busy 0x92E38 > 0); the new shot waits
//     for the first bullet (aim.rs attack_go step 1; 1.15m AttackGo? 0x5AD6A; #70 row 23, changelog 1.15z, found by
//     DarzaR).
// F33 Stab then touch: the end of a stab clears the queued action +0x1A6 (the knife), so a merc who walked to stab
//     someone no longer says "No one there" at the next touch order (aim.rs melee_hit step 6; 1.15m MeleeHit?
//     0x355C2; #70 row 25, changelog 1.15z, found by DarzaR).
// F34 Wall breach: after a crater the move codes of the 8 tiles around the centre are built again, so a breach
//     blown into a wall next to the centre can be walked through at once (explode.rs step 8; 1.15m LoadExplodeDat
//     0x351A7, cave 0x18EC4; #70 row 26, changelog 1.15z, found by DarzaR).
// F35 Group shape at entry: the mercs around the entry tile are placed from the facing round (not from north), so
//     a squad that changes sector keeps its shape in every direction (sector.rs merc_entry step 3b; 1.15m
//     PlaceMercsAtEntry 0x2B5B7, wrap helper 0x1FD10; #70 row 27, changelog 1.15z, found by DarzaR).
// F36 Enemy turn lock-up: when an enemy action times out with a forced shot, turn-based mode (0x92DAC) ends, so
//     the enemy turn no longer hangs there (ai.rs think step 10; 1.15m AiThink 0x6D476 / 0x6D4DE, store at 0x6D47A;
//     #70 row 34, changelog 1.15y, found by DarzaR).
// F37 Worn guns jam: the jam check also runs for items at condition 10..15 (test > 9, not > 15), and a jam sets
//     the condition to 1..9 (click.rs jam_check; 1.15m JamCheck 0x59A77 / 0x59A94; #70 row 79, changelog 1.15l, found
//     by DarzaR).
// F38 Turn hand-back after an interrupt: a new mercs' turn copies the "moved" byte +0x1B0 into +0x194 (not 1) for
//     mercs waiting at duty 8, shows the kept merc's record (0x284F0F, not the selected 0x284F11), and the first
//     merc pick prefers health 15+ (then 1+) and skips a duty-8 merc whose +0x194 is set (turns.rs new_mercs_turn
//     steps 2-3 + first_merc; 1.15m MercsTurn? 0x12F23 / 0x12F6B, FirstMerc? 0x2AC69 + tail 0x8AE73 / 0x8AFAD;
//     #70 row 91, changelog 1.15 jan 08, found by DarzaR).
// F39 No story scene during an interrupt: "a merc is busy" (MercBusy?) also holds while a merc in play has a duty
//     byte other than 0 (duty 8 = waiting in an interrupt), so the Brenda rescue view and sector speech wait
//     (update.rs u_merc_busy; 1.15m MercBusy? 0x1F707, tail 0x8AF96; #70 row 92 part, changelog 1.15 jan 08,
//     found by DarzaR).
// F40 Dying Trees and Brenda's kidnapping on one day: the day-start story events treat "kidnapping due today" like
//     "kidnapping started", so the blight messages do not take the guide queue slots the kidnapping needs and both
//     quests start (storyday.rs kidnap_due; 1.15m StoryDayEvents? 0x61E38 / 0x61EA7 / 0x61F2B, tail 0x8AC93; #70
//     row 85, changelog 1.15 jan 08, found by DarzaR).
// F41 Special income kept: cash collected from the mercs in the morning and the guide's 5000 bonus also go into
//     the balance before the day (g_prevBalance), so the day-end accounts do not lose them when the mercs cannot be
//     paid (pay.rs collect_cash, verdict.rs bonus; 1.15m CollectCash? 0x44616 + TickGuideBonus? 0x61641 / 0x6165C,
//     helper 0x6233E / 0x62344; #70 row 63, changelog 1.15l, found by DarzaR).
// F42 Throw while bandaged: a merc who is being bandaged gets -20 on the throw chance (lower limit 1), as on every
//     other attack (attack.rs throw_chance; 1.15m ThrowChance 0x58BFE, tail 0x8AE8E; #70 row 89, changelog 1.15
//     jan 08, found by DarzaR).
// F43 Worn gear needs working condition (part): sun goggles (condition 10+) and the scope (condition 10+) help
//     the aim and throw chances only when they work (aim.rs chance_to_hit, attack.rs throw_chance; 1.15m tails
//     0x8AD3A / 0x8AD4D at ChanceToHit 0x566F7 / 0x5671D and ThrowChance 0x58C50; #70 row 93, changelog 1.15 jan
//     08, found by DarzaR). Other row 93 sites (blast, damage, AI scores) are not ported yet.
// F44 No fake guards: at sector start the guards placed beside the tapper groups stop at the sector's guard count;
//     1.13b puts one guard at each group when there are fewer guards than groups (sector.rs setup step 14;
//     1.15m SetupSectorFromXra 0x2F50D / 0x2F580; #70 row 96, changelog 1.15 jan 08, found by DarzaR).
// F45 Insert toggles the revealed map: in the overview with fog of war (filter on), Insert shows the whole map
//     (filter off), and a second Insert goes back to the normal view; 1.13b ignores Insert in the overview (ja1front viewmode.rs insert;
//     1.15m TacticalKeyHandler 0x137D0 / 0x137E2; #70 row 80, changelog 1.15l, found by DarzaR).
// F46 No target mark on unseen soldiers (part): the target mark under the tile cursor shows only on a soldier
//     whose visible byte +0x9C is above 0, so the cursor no longer finds hidden enemies (ja1front cursor.rs
//     target_mark; 1.15m DrawTargetMark? 0x16E15 -> helper 0x2C57E; #70 row 60, changelog 1.15l, found by
//     DarzaR). The overview press (ja1front viewmode.rs press_walks; 1.15m OverviewMouse 0x2471A / 0x24733 -> helper
//     0x2C57C): a left press on a merc under 15 hp says "Not available!", and a soldier tile walks unless that
//     soldier is seen. The TacticalClick runs listed for row 60 belong to other rows (item drop, snap to tile).
// F47 Unseen soldiers do not block a merc's path: for a merc (slot 0..7) the path search treats the tile of a
//     soldier with visible byte +0x9C <= 0 as free (update.rs find_path_op; 1.15m FindPath 0x40B52 -> helper
//     0x2C57E, AH = 1; #70 row 61, changelog 1.15l, found by DarzaR).
// F48 Backward steps use the right tile: while a soldier walks backward (+0x196 set), the walk step picks the next
//     tile with the opposite facing, so legal backward moves are no longer refused (update.rs u_walk_move; 1.15m
//     WalkMove 0x1A057 -> cave 0x48250; #70 row 62, changelog 1.15l, found by DarzaR). The WalkBlocked runs of row 62
//     are code golf with the same behaviour (caves for other rows).
// F49 Bury a non-mine explosive: the item goes onto the ground first (PutItem: exact tile only when it is not the
//     merc's own tile; type < 0x11: +6 = 0, note check on, no near search), then the explosives practice, then the
//     blast starts at the tile where PutItem put the item (usetile.rs bury_mine; 1.15m BuryMine 0x3CBB3..0x3CC36
//     and PutItem 0x3FE43 returns the tile; #70 row 66, changelog 1.15l, found by DarzaR). The PutItem swap cave
//     0x488A6 (blast list follows the item) is row 70, not this fix.
// F50 Items put on a square with an item stay sane: when a new item lands on a square whose old item must move
//     aside, the old item stays put if a timed explosive counts down on that square, and a mine mark (blast list)
//     moves with the old item (sector.rs put_item step 7; 1.15m PutItem 0x3FE2E -> cave 0x488A6; #70 row 70 part,
//     changelog 1.15l, found by DarzaR). The LandItem? and TriggerItem? runs of row 70 are not in this fix.
// F51 A cleared blue flag recomputes only its own square's move code: where 1.13b rebuilt the whole move-code table
//     (door pass included) after a blue flag (object 0x86) went away, 1.15m runs MoveCode for that tile alone
//     (lib.rs flag_cleared_codes; 1.15m cave 0x5A855 from EffectHit? 0x173E5, ClearTileMarker86? 0x19D6C,
//     TriggerItem? 0x1AE80 / 0x1AF42; #70 rows 70 + 68 part, changelog 1.15l, found by DarzaR).
// F52 A thrown explosive that does not go off starts its effect where it really lies: when the landed item had to
//     move to another square, the effect (and the view jump in combat on an empty target square) uses that square,
//     not the aimed one (shoot.rs explosive; 1.15m LandItem? 0x57066 / 0x57130 returns PutItem's tile,
//     ProjectileImpact 0x572FA / 0x574E9 / 0x57562; #70 row 70 rest, changelog 1.15l, found by DarzaR).
// F53 A sector you do not own keeps its tapper and guard counts in the territory count: 1.13b set them to 0, so the
//     island map showed the harvested trees wrong after a battle until the day ended (ledger.rs recalc_territory;
//     1.15m RecalcTerritory 0x50EF6 -> 0x50FA9 clears only the workable byte; #70 row 77, changelog 1.15l "display of
//     harvestable trees when the map was updated at the end of a battle", found by DarzaR).
// F54 A merc who uses a nearby square with no item on it no longer starts an item trigger with the empty entry (0xFF):
//     1.13b read past the item table, which could start odd animations (drowning, a snake fight) (usetile.rs
//     use_on_tile; 1.15m UseOnTile? 0x3D412 / 0x3D4B5..0x3D4D5 skips TriggerItem? for 0xFF; #70 row 76, changelog
//     1.15l "weird animation sequences ... when a moving merc tried to interact with a close object", found by DarzaR).
// F55 Using an adjacent object no longer gives a free explosives practice each time: an empty square gives none, a
//     known trap (one in the blast list) gives none, and a disarmed locked trap leaves the blast list (usetile.rs
//     use_on_tile; 1.15m UseOnTile? 0x3D2B6..0x3D2E1 BlastListRemove? 0x3B718, cave 0x2118B at 0x3D436,
//     0x3D4F4..0x3D50A BlastListHas? 0x3B6F0; #70 row 74 rest (F25 = the step-start part), changelog 1.15l,
//     found by DarzaR).
// F56 A trapped explosive (types 0x16/0x18/0x1A/0x1C/0x1D) with an effect on the merc's own square now goes through
//     the trap check in the mine test itself: it answers -1 (the old pick-up path checked the effect separately and
//     the other routes not at all), and the callers count a mine down only for a real mine (usetile.rs mine_spotted,
//     use_on_tile, pickup_here; inventory.rs pick_up; 1.15m MineSpotted? 0x3C328 cave 0x42770, 0x3C346 cave 0x1B6D6,
//     UseOnTile? 0x3D5B6 / 0x3D609, PickUpHere? 0x3CEC0 / 0x3CF21 / 0x3CF5A, UseInventoryItem? 0x54A4D; #70 row 67,
//     changelog 1.15l, found by DarzaR).
// F57 The AP cost of using an object at a distance (walk there, then act) now matches what the action really
//     spends: the ground item's byte 6 no longer turns the walk-and-use cost into path + 4, item 0x76 no longer adds
//     10, and items 0x9F/0x97 add 10, 0xA3 adds 8, 0x98 adds 3 (click.rs use_cost; 1.15m UseCost 0x59B61 and
//     0x59B9E..0x59C01; ActionApCost 0x5635A only moves an inline test into the same helper 0x5AD1C, no change;
//     #70 row 78, changelog 1.15l, found by DarzaR).
// F58 The enemy no longer plans throws at targets well out of its throwing range: a target more than 16 past the
//     thrower's reach counts as chance 0 in the throw option (attack.rs throw_option + throw_chance_far; 1.15m
//     ThrowChance 0x58C6C..0x58CD7 leaves ebp = 15 - (distance - reach), `inc ebp` at 0x58DDC sets the sign flag that
//     ThrowOption 0x71C68 tests; the WeaponReach? calls in ThrowChance / ThrowAim are the same formula; #70 row 37,
//     changelog 1.15y, found by DarzaR).
// F59 A stab at a target square off the map edge no longer finds a soldier in the bytes after the map (aim.rs
//     melee_hit; 1.15m MeleeHit? 0x3523F: target tile >= 0xC40 goes straight to the AP spend; #70 row 49, changelog
//     1.15x "point blank shot/stab damage applied to the wrong merc", found by DarzaR).
// F60 The merc keys (F1..F8, Alt+F1..F8) do nothing while an item is on the mouse cursor, so the item cannot move to
//     another merc (ja1front play.rs Input::Merc; 1.15m TacticalKeyHandler 0x136B3..0x1377B -> tail 0x8AEED tests
//     g_cursorItem 0x222B50; #70 row 87 part, changelog 1.15 jan 08, found by DarzaR).
// F61 A merc switch by the game (new turn, low-AP stop, a spotted item, a portrait click, a click on a merc on the map
//     or in the overview) first puts an item held on the mouse back into the old merc's box (turns.rs new_turn +
//     new_mercs_turn, update.rs u_low_ap_stop + merc_view; ja1front play.rs Portrait, cursor.rs UseItemAt + overview
//     select; 1.15m: 10 SelectMerc? call sites go through cave 0x8AF04 = CursorItemBack? 0x53660 first; #70 row 87
//     rest, changelog 1.15 jan 08, found by DarzaR).
// F62 A game that ends in the sector (won, home lost, ...) still counts its last day: accounts, replay and verdict
//     (score) run, and a lost game shows the final rating, then the program ends instead of a new game (ja1front
//     dayloop.rs day_steps + game_end, main.rs; 1.15m PlayLoop 0x4FC47 + 0x4FCCE, DayEndScreen? cave 0x1EEC1; #70
//     row 72, changelog 1.15l, found by DarzaR).
// F63 Not paying a live merc leaves a debt (rehire costs triple, terminations + 1); not paying a dead merc's family
//     counts as a refused corpse export (grudges), with no debt (payday.rs dismiss; 1.15m PayDayDialog? 0x453C1..
//     0x45456, CorpseGrudge? 0x46548, Merc leaves status 0; #70 row 65 part, changelog 1.15l, found by DarzaR).
//     The A.I.M. part (ja1front aim.rs): a payable old debt that leaves too little for the helicopter shows the
//     warning box 0x9771F first (AimScreen 0x2070E -> cave 0x6F735).
// F64 Opening A.I.M. no longer gives a merc who is already away on another job a new job length (aimhire.rs open;
//     1.15m AimScreen 0x20176..0x20197; #70 row 47, changelog 1.15x, found by DarzaR).
// F65 The inventory screen stops the timer (ja1front play.rs inv_ticks: round_lock2 for its time, not Lock?/Unlock?;
//     1.15m UseInventoryItem? 0x53B06 / 0x54D0E call cave 0x42780; #70 row 73, changelog 1.15l, found by DarzaR).
// F66 A pick-up's inventory screen shows the pointer also when an enemy interrupted the merc (ja1front play.rs
//     inventory: screen_drawn 1 for the screen; 1.15m CursorItemToSoldier? 0x54EE9 -> cave 0x2B6A1; #70 row 90,
//     changelog 1.15 jan 08, found by DarzaR).
// F67 Quick Restore, Quick Save and Exit to DOS first put a held item back in its merc's pocket; the strips reload
//     after those boxes and after the Control Panel, traverse and abort-day boxes (ja1front tactkeys.rs back_fan +
//     strip_reload; 1.15m TacticalKeyHandler caves 0x8AF11..0x8AF87; #70 row 86, changelog 1.15 jan 08, found by DarzaR).
// F68 A broken bridge piece in the water (move code 25300 on a water tile) costs 13 AP to enter, not 2, and after a
//     crater every such piece with no bridge deck (25400) next to it becomes impassable (update.rs step_cost +
//     bridge_sweep, blast.rs effect_hit; 1.15m StepApCost 0x39B83 -> cave 0x38427 / tail 0x8AFCD, EffectHit? 0x174E0
//     -> cave 0x54CB1; #70 row 71 part, changelog 1.15l, found by DarzaR).
// F69 One explosion hits each target only once: an effect record is marked as hit before its hit runs, so a merc
//     killed by the blast (its death redraw runs the effect records again) no longer makes the same blast hit again
//     (blast.rs effect_pass; 1.15m DrawProjectilePass? 0x1763A / OverviewRedraw 0x24019 set +0x10 first; #70 row 30,
//     changelog 1.15y, found by DarzaR).
// F70 A sneaking merc practises agility against unaware enemies once per step, not on every frame at a step point
//     (update.rs u_active_at_point step 2; 1.15m ActiveFrame 0x1BA9F/0x1BB0C tests +0x193 first; #70 row 99; a 1.15m
//     change with no changelog line, found in the jam115.exe bytes, patch by DarzaR).

// The mask in two halves: LO = F01..F64, HI = F65..F128 (no 128-bit atomic in std).
static LO: AtomicU64 = AtomicU64::new(0);
static HI: AtomicU64 = AtomicU64::new(0);

// The port fixes P01..P64 (#71): our own fixes for #2 bugs that 1.15m leaves. Each has a check line `ai_diff --pfix`
// (or another test) that runs the original with the same fix as a small asm patch.
// P01 #2 row 9: a key used from a key ring kept in a pocket (not the first key on it) clears the ring's 4th key id,
//     not the 4th byte of the hand item (doors.rs use_key; ConsumeKey? 0x3AE78, write at 0x3AF66).
// P02 #2 row 10: a ctrl + left click on the attachment box or the ground box of the inventory unstacks nothing (the
//     original treated soldier bytes +0x90 / +0x9C as an item entry; inventory.rs item clicks, UseInventoryItem?
//     0x54179 and AssignScreen? 0x49619 before UnstackItem 0x53580).
// P03 #2 row 13: the morning message about quitting tappers shows the number of tappers who quit, not the number
//     of guards (ledger.rs natives_quit step 4; MercsQuitMsg? 0x50E64 `mov al, cl` -> `mov al, bh`).
// P04 #2 row 16: on the island map a click at the far right edge of the east column picks that column's sector,
//     not the first sector of the next row (ja1front island.rs sector_at; MapClickToSector? 0x41410, column capped at 9).
// P05 #2 row 19: a left or right press on a store place of the assignment screen with an empty store (store view 0,
//     no current record) takes nothing and shows no info box (the original read the "record" at memory address 0: a
//     phantom item, a hang or a count of -1; inventory.rs assign_press / assign_right; AssignScreen? 0x49E51 and
//     0x4A083).
// P06 #2 row 20: key E (all items into the store; FIRE uses the same step) leaves an item on the merc when the store
//     does not take it (99 or more of that type there); 1.13b cleared the entry anyway and the item was lost
//     (inventory.rs store_all; StoreAllItems 0x52150 clears bytes 0..5 after AddStashItem? 0x51D50).
// P07 #2 rows 26 and 27: on the save name line Home, Left, Right, End and Delete move the cursor or delete a letter
//     (the name input's own cursor keys); 1.13b put the signs [ < > ] or 0x7F into the name, and a drawn 0x7F crashed
//     the game (ja1front saveslots.rs name_keys; SaveSlotNameInput? 0x4E075, the key reader 0x4DC1C maps the
//     extended keys O G K M S to "][<>" + 0x7F and its cursor switch at 0x4E084 reads a second key instead).
// P08 #2 row 37: a merc who uses an item on a square with no ground item no longer reads the empty entry's type from
//     profile 5's kit (a strobe there called the enemies, a plastic bag toggled doors); F54 drops only the trigger,
//     P08 drops the type test too, so the square goes the normal way (usetile.rs use_on_tile step 7 no_pickup;
//     UseOnTile? 0x3D3E1, no g < 0xFF test before the type read 0x3D3F0).
// P09 #2 row 41: Quick Restore sends the restored music and sound volume numbers to the sound output, so the game
//     plays at the restored loudness at once (ja1front sound.rs restore_volumes; QuickRestore 0x2D688 / 0x2D6BC read
//     0x963A0 / 0x963A8 without the calls 0x5CEB4 / 0x5CE94). Test: ja1front unit test
//     sound::tests::volume_keys_work_without_a_sound_device.
// P10 #2 row 46: the thin line above the merc's name on the A.I.M. page shows no stray pixels from the bottom of the
//     screen (ja1front aim.rs page; ShowAimScreen 0x1FF2D saves area 4, never restored, before area 5, so they no
//     longer share back buffer row 66).
// P14 #2 row 32: no DrawMarkersTo 0x41788 call in the replay battle flash (ja1front replaypic.rs battle_frame; the fixed
//     original: 0x64B5E nopped). Test: replaypic_diff --port --pfix 14.
// P15 #2 row 33: the overview holds mode-1 view coordinates: 2/3 -> 0 takes the wide shifts back, 0 -> 2/3 shifts as
//     from 1 (ja1front viewmode.rs set; 0x249E4 je gone + cave at 0x24B64). Test: viewmode_diff --port --pfix 15.
// P16 #2 row 36: an XRA charge 0x71 with E[1] 0x63 takes the kind of the sector's first other charge (sector.rs XRA
//     load; cave at 0x2EF26). Test: ai_diff --port --only sector_start --pfix 16.
// P17 #2 row 40: the volume numbers start at 127, the loudness the game starts with; a louder key caps at 127
//     (ja1front sound.rs start_volumes / volume_key). Test: ja1front unit test volume_keys_work_without_a_sound_device.
// P18 #2 row 51: a missing name sample (speaker 29 has no 40.VOC) plays nothing (ja1front sound.rs name_voice; the
//     fixed original: cave at LoadVocSample 0x5C98B). Test: voice_diff --pfix 18.
// P19 #2 row 57: op 157 of the turn code no longer moves the pointer to the shared saved place 0x229C40 (ja1front
//     pointer.rs turn_mouse). Test: ja1front unit test the_turn_change_keeps_the_pointer_with_p19.
// P20 #2 row 62: a capacity above the start with no quote in the table gives no guide remark (verdict.rs step 13;
//     DayEndVerdict 0x60E7F..0x60E9B re-aimed). Test: verdict_diff --port --pfix 20.
// P21 #2 row 63: key D does nothing while an enemy interrupt waits (turn_moves 0x92DD0 != 0; ja1front tactkeys.rs;
//     cave at 0x13DE8). Test: keyhandler_diff --port --pfix 21.
// P22 #2 row 64: a held item at day end goes to the first empty strip entry (vest pocket only when worn), else to
//     the ground (ja1front play.rs cursor_back_host; cave at 0x536BD). Test: cursorback_diff --pfix 22.
// P23 #2 row 65: a held click on a gear item of the A.I.M. page draws no eye frame over a dark face (DECEASED, or the
//     stamp of a merc on the team or on assignment) (ja1front aim.rs item_click; FaceEyeFrame 0x27D64 at 0x20D6A).
//     Test: ja1front unit test a_gear_click_draws_no_eyes_on_a_dark_face_with_p23.
static PORT: AtomicU64 = AtomicU64::new(0);
/// Every port fix (level `all`).
pub const PORT_ALL: u64 = u64::MAX;
/// The highest port fix id.
pub const PORT_MAX_ID: u32 = 64;

/// Is port fix `Pnn` (`id` = nn, 1..=PORT_MAX_ID) on?
#[inline]
pub fn port_on(id: u32) -> bool {
    (1..=PORT_MAX_ID).contains(&id) && PORT.load(Ordering::Relaxed) & (1u64 << (id - 1)) != 0
}

pub fn port_set(mask: u64) {
    PORT.store(mask, Ordering::Relaxed);
}

pub fn port_mask() -> u64 {
    PORT.load(Ordering::Relaxed)
}

/// The (F mask, P mask) of a JA1_FIXES / ja1.cfg `fixes` value: "" / "0" / "off" = none, "1.15m" (also "1", "on":
/// the old fan_fixes values) = every F id, "all" = every F and P id, else ids `F07,P02` (case ignored, a bad id is
/// an error).
pub fn parse_fixes(s: &str) -> Result<(u128, u64), String> {
    let s = s.trim();
    match s.to_ascii_lowercase().as_str() {
        "" | "0" | "off" => return Ok((0, 0)),
        "1.15m" | "1" | "on" => return Ok((ALL, 0)),
        "all" => return Ok((ALL, PORT_ALL)),
        _ => {}
    }
    let (mut f, mut p) = (0u128, 0u64);
    for x in s.split(',').map(str::trim).filter(|x| !x.is_empty()) {
        if let Some(n) = x.strip_prefix(['P', 'p']).and_then(|d| d.parse::<u32>().ok()).filter(|n| (1..=PORT_MAX_ID).contains(n)) {
            p |= 1u64 << (n - 1);
        } else {
            f |= parse(x).map_err(|_| format!("fixes: {:?} is not a fix id (F01..F{}, P01..P{})", x, MAX_ID, PORT_MAX_ID))?;
        }
    }
    Ok((f, p))
}

/// Fan fix F07 (#70 row 7, data): the 1.15m g_windowTbl 0x95788 bytes +0x40B and +0x40F (object 0x81, directions
/// 3 and 7) are 0x11, not 0x07. `i` = the table index, `v` = the 1.13b byte read there.
pub fn window_byte(i: i64, v: u8) -> u8 {
    if (i == 0x40B || i == 0x40F) && on(7) {
        0x11
    } else {
        v
    }
}

/// Is fix `Fnn` (`id` = nn, 1..=MAX_ID) on?
#[inline]
pub fn on(id: u32) -> bool {
    match id {
        1..=64 => LO.load(Ordering::Relaxed) & (1u64 << (id - 1)) != 0,
        65..=128 => HI.load(Ordering::Relaxed) & (1u64 << (id - 65)) != 0,
        _ => false,
    }
}

pub fn set(mask: u128) {
    LO.store(mask as u64, Ordering::Relaxed);
    HI.store((mask >> 64) as u64, Ordering::Relaxed);
}

pub fn mask() -> u128 {
    LO.load(Ordering::Relaxed) as u128 | (HI.load(Ordering::Relaxed) as u128) << 64
}

/// The mask of a JA1_FAN_FIXES value: "" / "0" / "off" = none, "1" / "on" = ALL, else ids `F07,F12` (case
/// ignored, a bad id is an error).
pub fn parse(s: &str) -> Result<u128, String> {
    let s = s.trim();
    match s.to_ascii_lowercase().as_str() {
        "" | "0" | "off" => return Ok(0),
        "1" | "on" => return Ok(ALL),
        _ => {}
    }
    let mut m = 0u128;
    for p in s.split(',').map(str::trim).filter(|p| !p.is_empty()) {
        let n = p.strip_prefix(['F', 'f']).and_then(|d| d.parse::<u32>().ok()).filter(|n| (1..=MAX_ID).contains(n));
        match n {
            Some(n) => m |= 1u128 << (n - 1),
            None => return Err(format!("fan_fixes: {:?} is not a fix id (F01..F{})", p, MAX_ID)),
        }
    }
    Ok(m)
}

/// C interface for the diff tests (validate/diff/port_lib.py): set / read the mask. `ja1_fan_fixes_set` sets
/// F01..F64 and turns F65..F128 off (a test that names only low ids gets exactly those); `ja1_fan_fixes_set_hi`
/// then sets F65..F128 (bit 0 = F65).
#[no_mangle]
pub extern "C" fn ja1_fan_fixes_set(lo: u64) {
    set(lo as u128);
}

#[no_mangle]
pub extern "C" fn ja1_fan_fixes_set_hi(hi: u64) {
    HI.store(hi, Ordering::Relaxed);
}

#[no_mangle]
pub extern "C" fn ja1_fan_fixes() -> u64 {
    LO.load(Ordering::Relaxed)
}

#[no_mangle]
pub extern "C" fn ja1_fan_fixes_hi() -> u64 {
    HI.load(Ordering::Relaxed)
}

/// C interface: set / read the port fix mask (P01 = bit 0).
#[no_mangle]
pub extern "C" fn ja1_port_fixes_set(m: u64) {
    port_set(m);
}

#[no_mangle]
pub extern "C" fn ja1_port_fixes() -> u64 {
    port_mask()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn levels_parse() {
        assert_eq!(parse_fixes("off"), Ok((0, 0)));
        assert_eq!(parse_fixes(""), Ok((0, 0)));
        assert_eq!(parse_fixes("1.15m"), Ok((ALL, 0)));
        assert_eq!(parse_fixes("1"), Ok((ALL, 0)));
        assert_eq!(parse_fixes("on"), Ok((ALL, 0)));
        assert_eq!(parse_fixes("ALL"), Ok((ALL, PORT_ALL)));
        assert_eq!(parse_fixes("F07,p02"), Ok((1 << 6, 1 << 1)));
        assert_eq!(parse_fixes("P64"), Ok((0, 1 << 63)));
        assert!(parse_fixes("P65").is_err());
        assert!(parse_fixes("P00").is_err());
        assert!(parse_fixes("F129").is_err());
    }

    #[test]
    fn values_and_ids_parse() {
        assert_eq!(parse(""), Ok(0));
        assert_eq!(parse("off"), Ok(0));
        assert_eq!(parse("0"), Ok(0));
        assert_eq!(parse("on"), Ok(ALL));
        assert_eq!(parse("1"), Ok(ALL));
        assert_eq!(parse("F01"), Ok(1));
        assert_eq!(parse("f07, F12"), Ok((1 << 6) | (1 << 11)));
        assert!(parse("F00").is_err());
        assert_eq!(parse("F65"), Ok(1u128 << 64));
        assert_eq!(parse("F128"), Ok(1u128 << 127));
        assert!(parse("F129").is_err());
        assert!(parse("seven").is_err());
    }

    #[test]
    fn default_is_off() {
        // No other test sets the mask: the port starts as 1.13b.
        assert_eq!(mask(), 0);
        assert!(!on(1));
    }
}
