# Third-party software

The release builds link these libraries into `ja1front`. All have licences that allow use in a GPL-3.0 program.

| Part | Licence | Source |
| --- | --- | --- |
| SDL 2 (linked statically in the release builds) | zlib | https://www.libsdl.org |
| sdl2, sdl2-sys (Rust bindings) | MIT | https://github.com/Rust-SDL2/rust-sdl2 |
| png | MIT or Apache-2.0 | https://github.com/image-rs/image-png |
| flate2, miniz_oxide, fdeflate, crc32fast, adler2, simd-adler32, zlib-rs | MIT / Apache-2.0 / Zlib (each crate states its own) | https://crates.io |
| bitflags, cfg-if, lazy_static, libc, version-compare | MIT or Apache-2.0 | https://crates.io |

`port/ja1front/Cargo.lock` holds the exact versions.

## Not included

- The game Jagged Alliance (Sir-tech / Madlab, 1994): you need your own copy (GOG).
- The official 1.13b patch files: optional, supply your own (README "Play").
- For the RE tools: Capstone, Unicorn, Ghidra, DOSBox-X are installed separately; the Watcom libraries that
  `./t libid` compares against are not included.

## Unofficial patch 1.15m (fan fixes)

The `fixes` setting of the port at level `1.15m` or `all` (ja1.cfg `fixes`, `JA1_FIXES`; off by default; the old
`fan_fixes` switch reads as `1.15m`) turns on bug fixes that
copy the behaviour of the unofficial **Jagged Alliance 1.15m** patch (20 April 2021, built on Sir-tech's 1.13b).
Patch author: **DarzaR** (GOG forum thread "JA1 unofficial 1.15 patch",
https://www.gog.com/forum/jagged_alliance_series/ja1_unofficial_115_patch). The port ships no file of that patch:
each fix is written in port code from a study of its changed bytes, and each one is checked against its exe
(work issue #70 of the project tracker).

### Fan fixes (credit)

- **Our own code.** Each fix in the port is our own code, written from the 1.15m changelog
  (`README_JAm115.TXT`) and checked against the 1.15m exe (`jam115.exe`) in diff tests or by a code read.
- **DarzaR** found and documented the bugs, and wrote the unofficial 1.15 patch.
- **MitchShudlem** found the bugs that the 1.15m release fixes (changelog "ver. aug 20 1.15m": "with fixes for
  bugs found by him only"); DarzaR fixed them in 1.15m.
- Forum: https://www.gog.com/forum/jagged_alliance_series/ja1_unofficial_115_patch
- **No 1.15m file ships** with the port: not the exe, not its XRA.DAT. The XRA.DAT spawn fix (F01) is a table of
  27 byte values in port code.
- In port code, each fix id (`F01`, `F02`, ...) has a comment with the changelog version and item and who found
  it. The table on issue #70 has a "Found by" column.
- **F70 has no changelog line.** It is a 1.15m change that we found in the exe bytes, not in the changelog.
- **Not included:** 1.15m drops the ordinary items in the last two inventory places of a merc who leaves the team.
  The port keeps the 1.13b rule: those items go to the team store.

| Fix id | Changelog | Found by | In plain words |
|---|---|---|---|
| F01 | 1.15x "Fixed errors in game data file pertaining to enemy spawn points" | DarzaR | Enemy spawn points in the game data are fixed: no two enemies on one square, none in walls. |
| F02 | 1.15z "Fixed Explosives training's erroneous use of ExpLevel value" | DarzaR | Explosives training rolls by the explosives skill, not by the experience level. |
| F03 | 1.15z "In case a merc finish a day in incapacitated state (<15 HP), bug caused a wrong calculation of MaxHP" | DarzaR | A merc who ends a day below 15 health no longer gains maximum health or loses his health experience. |
| F04 | 1.15l "Fixed a memory corruption problem in the display of some merc's messages (which resulted in badly formed and cut text)" | DarzaR | A merc message with his name late in the text no longer loses letters or shows old text. |
| F05 | 1.15 (jan 08) "In case Guards were sent to a sector with Guards already travelling" | DarzaR | Sending guards to a sector where guards are already walking is refused, so the earlier guards no longer get lost. |
| F06 | 1.15 (jan 08) "Bandaging a wounded merc had negative instead of positive effect on his/her speed and aim" | DarzaR | Bandages on a wounded merc now make his action points and his aim better, not worse. |
| F07 | 1.15z sight through windows (window table and window step limit) | DarzaR | Looking through a window now works the same from every side; south and east windows no longer block the view too much. |
| F08 | 1.15m small blast leaves a merc at 0 health standing (TakeDamage) | MitchShudlem (fixed by DarzaR) | A merc brought to 0 health by a small blast that is not on his square now falls down instead of staying half dead. |
| F09 | 1.15z collapse from bleeding (TakeDamage fall picture below 15 health) | DarzaR | A merc who collapses from bleeding now falls to the ground in turn-based play. |
| F10 | 1.15l leg wound of one merc drops another (TakeDamage knock-down flag) | DarzaR | A merc no longer falls down because of a leg wound that another merc received. |
| F11 | 1.15m critical leg hit on a crouching merc (TakeDamage) | MitchShudlem (fixed by DarzaR) | A critical leg hit on a crouching merc now makes him fall at once, without a long wait in the enemy turn. |
| F12 | 1.15m first aid cancelled during the kneel-down (EndTask) | MitchShudlem (fixed by DarzaR) | First aid cancelled early now also stops the kneeling picture. |
| F13 | 1.15m swap-by-X tile change (PlaceAt) | MitchShudlem (fixed by DarzaR) | A merc moved by a swap with X now stands on the centre of the new square with his step state reset. |
| F14 | 1.15z interrupted at the goal tile, control lost until Escape (WalkEnd StopWalk) | DarzaR | A merc interrupted just as he reaches his goal no longer needs Escape to give control back. |
| F15 | 1.15z death animation for enemies dying from bleeding (EnemiesTurn?) | DarzaR | An enemy who dies from bleeding at the start of the enemies' turn now falls where you can see it. |
| F16 | 1.15z attack on the South Processing Plant held back by water snakes (EnemyMoves?) | DarzaR | Enemies who attack the South Processing Plant in the story are no longer held back by water snakes, as in the attack on Brenda's lab. |
| F17 | 1.15z lock-up when the last acting enemy is noticed as an unseen attacker (SightCheck, MarkSeen) | DarzaR | An enemy who is down but not yet dead still counts as seen (the 1.15 fix for a lock-up in the enemies' turn). |
| F18 | 1.15l time compression usually resulted in set explosives failing to detonate (TacticalTimer?, TacticalLoop, EndTurn?) | DarzaR | Explosives with a timer now count down at the speed of compressed time, and compressed time stops when they go off. |
| F19 | 1.15z Santino's attempt to detonate his trap during the enemy turn kept some bombs from going off (NewTurn?) | DarzaR | The bombs in Santino's base now go off after he speaks, so he no longer stays alive by mistake. |
| F20 | 1.15y enemies and guards arriving in the mercs' sector caused a skip of a whole turn (NewTurn?) | DarzaR | Enemies or guards who arrive in your sector no longer make you lose a turn. |
| F21 | 1.15z retreat penalty for linked and unlinked enemy sectors swapped (MsgRetreatCasualty?) | DarzaR | When you retreat at the end of the day, a sector with a safe way home is now less dangerous, not more. |
| F22 | 1.15z wrong conditions for determining a need to retreat at day's end (LeaveSectorCleanup?) | DarzaR | A sector you take at the very end of the day now counts as yours at once, so its trees and workers are counted right away. |
| F23 | 1.15z falling and rising animations, part: no stand animation under 15 health (AnimStep); fall steps + diagonal step size (AnimFrame) | DarzaR | A badly wounded soldier on the ground no longer switches to the standing animation by mistake, and a falling soldier lands on the right tile on diagonals. |
| F24 | 1.15l reserve points enabling players to continue movement without remaining APs, part (StepStart) | DarzaR | In combat, a merc with fewer action points than the reserve now stops walking, as the reserve says. |
| F25 | 1.15l free experience from adjacent objects, part: trap detection now gives experience (StepStart) | DarzaR | A merc who spots a mine on the way now gets experience rolls for it. |
| F26 | 1.15 (jan 13) losing a sector changed tree availability in adjacent sectors, but income from trees drawn non-harvestable still happened (RecalcTerritory) | DarzaR | Trees that the map shows as not harvestable no longer bring money. |
| F27 | 1.15z fixed occasional disappearing of table of events of a battle between guards and enemies after a mouse click (ResolveSectorBattle) | DarzaR | The table of a battle between your guards and the enemies no longer vanishes at a mouse click after an earlier Escape. |
| F28 | 1.15z if guards and enemies are still present in sector after very last turn of day, events of a battle between them weren't shown on a screen (TacticalLoop leave path) | DarzaR | A battle between your guards and the enemies at the end of the day now shows on screen instead of on a black screen. |
| F29 | 1.15z + 1.15y interrupts inside interrupts (NextTurn?, InterruptTurn?) | DarzaR | An interrupt that happens inside another interrupt now hands the turn back correctly, and guards who interrupt no longer stop a merc who also interrupts. |
| F30 | 1.15z no experience roll for a stab by the own side (MeleeHit?) | DarzaR | A merc stabbed by a merc of his own team no longer gets an agility experience roll. |
| F31 | 1.15z new game after a game left some items out (NewGameInit) | DarzaR | A new game started after another game, without a restart, places all its items again. |
| F32 | 1.15z shot while a bullet is still in flight (AttackGo?) | DarzaR | A new shot now waits until the first bullet lands, so it no longer misses for that reason. |
| F33 | 1.15z 'No one there' after a stab (MeleeHit?) | DarzaR | A merc who walks to stab someone no longer says 'No one there' when his next order is to touch an object. |
| F34 | 1.15z breaches blown into walls (LoadExplodeDat cave) | DarzaR | A breach that an explosion blows into a wall can be walked through at once, not only after you enter the sector again. |
| F35 | 1.15z group position on a sector change (PlaceMercsAtEntry) | DarzaR | Mercs who change sector keep their group position in every direction, not only north. |
| F36 | 1.15y lock-up in the enemy turn (AiThink) | DarzaR | A rare lock-up in the enemies' turn after a forced shot is fixed. |
| F37 | 1.15l items in nearly useless condition passed the working check (JamCheck) | DarzaR | A badly worn gun can now jam, as the condition says. |
| F38 | 1.15 (jan 08) merc tabs greyed out after an interrupt inside an interrupt (MercsTurn?, FirstMerc?) | DarzaR | After an interrupt inside an interrupt, your mercs' tabs work again and the turn goes back to the right merc. |
| F39 | 1.15 (jan 08) Rescue Brenda event during an interrupt (MercBusy?) | DarzaR | The Rescue Brenda event no longer starts during an interrupt. |
| F40 | 1.15 (jan 08) Dying Trees and Brenda's Kidnapping on one day (StoryDayEvents?) | DarzaR | The Dying Trees and Brenda's Kidnapping quests now both start when they fall on the same day. |
| F41 | 1.15l special income lost when the mercs cannot be paid (CollectCash?, TickGuideBonus?) | DarzaR | Cash found by your mercs and Jack's bonus are no longer lost when you cannot pay all your mercs. |
| F42 | 1.15 (jan 08) "Merc being bandaged received penalty for all attack chances but throwing" (ThrowChance) | DarzaR | A merc who is being bandaged now gets the same penalty on a throw as on every other attack. |
| F43 | 1.15 (jan 08) goggles, gas masks, scopes, silencers and armour work only in working condition (part: ChanceToHit, ThrowChance, ThrowDamage, ExpectedDamage, KnifeDamage, HitLocationDamage, CanStandAt, EngagedAction, CalmAction, TurnRest?, BlastTile?, ShotEnd, LaunchProjectile) | DarzaR | Sun goggles, gas masks, scopes, silencers and armour that are almost broken no longer help. |
| F44 | 1.15 (jan 08) "fake" additional guards proportional to the tappers (SetupSectorFromXra) | DarzaR | A sector with guards and tappers no longer gets extra guards when there are fewer guards than tapper groups. |
| F45 | 1.15l "The Insert button now correctly toggles between fog-of-war and fully revealed map views" (TacticalKeyHandler) | DarzaR | In the map overview, Insert now switches from fog of war to the fully revealed map, and then back to the normal view. |
| F46 | 1.15l finding enemies by the interface alone, without seeing them (DrawTargetMark?, OverviewMouse) | DarzaR | The target mark under the cursor no longer shows on an enemy that your mercs cannot see. In the map overview, a click on an unseen enemy walks there, and a click on a badly hurt merc says "Not available!". |
| F47 | 1.15l unseen enemies no longer block a move order (FindPath) | DarzaR | An enemy that your mercs cannot see no longer blocks or bends a move order, so the order does not give the enemy away. |
| F48 | 1.15l "Fixed a bug in collision detection preventing some perfectly legal moves while going backward" (WalkMove) | DarzaR | A merc who steps backward now checks the square behind them, so legal backward moves are no longer refused. |
| F49 | 1.15l "Several Mines and other Explosives related bugs fixed" (BuryMine, PutItem) | DarzaR | When a merc sets off an explosive that is not a mine, it goes onto the ground first and the blast starts on the square where it landed. |
| F50 | 1.15l "Several Mines and other Explosives related bugs fixed": placing an item on a square with other items (PutItem) | DarzaR | When an item lands on a square where the old item must move aside, a ticking explosive stays put and a mine mark moves with its item. |
| F51 | 1.15l "Several Mines and other Explosives related bugs fixed": the mine sprite and item moves (EffectHit, ClearTileMarker86, TriggerItem) | DarzaR | When a blue mine flag goes away, only that square's walking cost is worked out again, as the fan patch does. |
| F52 | 1.15l "Several Mines and other Explosives related bugs fixed": a thrown explosive that lands (LandItem, ProjectileImpact) | DarzaR | A thrown explosive that does not go off at once starts its countdown on the square where it really lies, not on the aimed square. |
| F53 | 1.15l "Fixed the display of harvestable trees when the map was updated at the end of a battle in a sector" (RecalcTerritory) | DarzaR | A sector you do not own keeps its tapper and guard counts, so the island map shows the harvested trees right after a battle. |
| F54 | 1.15l "Fixed a minor graphics bug resulting in weird animation sequences (such as drowning or fighting snake, etc) when a moving merc tried to interact with a close object" (UseOnTile) | DarzaR | A merc who uses a nearby square with nothing on it no longer starts a strange animation such as drowning or a snake fight. |
| F55 | 1.15l free experience from adjacent objects, rest: only the first touch practises (UseOnTile) | DarzaR | Touching a nearby object gives explosives practice only once, not each time, and never for an empty square or a known trap. |
| F56 | 1.15l a trapped explosive taken through some item routes skipped the defuse check (MineSpotted, UseOnTile, PickUpHere, inventory pick up) | DarzaR | Picking up or using a trapped explosive always runs the trap check, and a trap no longer counts as a mine found. |
| F57 | 1.15l actions that need extra walking counted the wrong AP (UseCost) | DarzaR | The AP shown for walking to an object and using it matches what the action spends, so an accepted order no longer ends in "not enough points". |
| F58 | 1.15y the enemy planned throws at targets far out of its throwing range (ThrowOption) | DarzaR | The enemy takes your distance into account and wastes fewer out-of-range throws. |
| F59 | 1.15x a stab at a square past the map edge could hit a wrong merc (MeleeHit) | DarzaR | A stab at the map edge no longer hurts a merc somewhere else. |
| F60 | 1.15 an item held on the mouse moved to another merc after F1-F8 (TacticalKeyHandler) | DarzaR | The merc keys wait until the item on the mouse is put down. |
| F61 | 1.15 an item held on the mouse moved to another merc by auto-switch (SelectMerc callers) | DarzaR | When the game or a click switches to another merc, the item on the mouse goes back to the merc who held it. |
| F62 | 1.15l the last day's score and replay were cut when the game ended; the final score of a lost game was not shown (PlayLoop) | DarzaR | A game that ends still counts its last day and shows its replay, and a lost game shows its final rating. |
| F63 | 1.15l non-payment cases reversed: a dead merc's family got the debt, a live merc none (PayDayDialog) | DarzaR | An unpaid live merc leaves a debt you must pay to rehire him; an unpaid dead merc's family holds a grudge instead. |
| F64 | 1.15x side assignments re-rolled for mercs already away; closing and reopening the laptop could end them (AimScreen) | DarzaR | A merc away on another job keeps his days left when you open A.I.M. again. |
| F65 | 1.15l time restarted in the inventory screen, which could set off bombs after you leave (UseInventoryItem) | DarzaR | Time stays stopped while the inventory screen is open, also after a message box in it. |
| F66 | 1.15 (jan 08) no mouse cursor in the inventory screen for a merc interrupted while picking up an item (CursorItemToSoldier) | DarzaR | A merc that an enemy interrupts in a pick-up sees the mouse pointer in his inventory screen. |
| F67 | 1.15 (jan 08) menus and pop-ups with an item held could clone or lose that item (TacticalKeyHandler) | DarzaR | Quick Save, Quick Restore and Exit to DOS put a held item back in its merc's pocket first. |
| F68 | 1.15l exploded bridges stay usable at a high AP cost until the sector reloads, and no longer trap mercs (StepApCost, EffectHit) | DarzaR | Part: a broken bridge piece in the water costs 13 AP to enter (not 2); after a crater, a piece with no bridge deck next to it becomes impassable. Not yet: the new blast kind 0x11, CanStandAt and WalkBlocked changes of the same row |
| F69 | 1.15y one explosion could damage the same target more than once in special cases (DrawProjectilePass?) | DarzaR | An explosion's effect record is marked as hit before its hit runs, so a merc killed by the blast (its death redraw runs the effect records again) no longer makes the same blast hit everyone in range a second time |
| F70 | none: a 1.15m change with no changelog line, found in the exe bytes (ActiveFrame) | DarzaR (patch author) | A sneaking merc practises agility against enemies that do not see him once per step, not again on every animation frame while he stands at a step point |

## Port fixes (our own, level `all`)

At level `all` the `fixes` setting also turns on our own fixes (`P01`, `P02`, ...) for bugs of issue #2 that 1.15m
leaves (work issue #71). This project found each bug in its reverse engineering of `JA.EXE` 1.13b; the "Found"
column is the date of its issue #2 row. Each fix is checked against the original with the same fix as a small patch
(a diff test with `--pfix`), or by a unit test. Level `off` and level `1.15m` do not change.

| Fix id | Issue #2 row | Found | In plain words |
|---|---|---|---|
| P01 | 9 | 2026-10-22 | A key from a key ring in a pocket no longer damages the item in the merc's hand. |
| P02 | 10 | 2026-10-23 | A Ctrl-click on the attachment box or the ground box of the inventory screen no longer scrambles the merc's data. |
| P03 | 13 | 2026-10-23 | The message about tappers who quit shows the number of tappers. |
| P04 | 16 | 2026-10-23 | A click on the far right edge of the island map picks the sector under the mouse. |
| P05 | 19 | 2026-10-24 | A click on an empty item store takes nothing and cannot crash the game. |
| P06 | 20 | 2026-10-24 | Key E keeps an item that the store cannot take (99 or more of its kind) on the merc. |
| P07 | 26, 27 | 2026-10-25 | On the save name line, the arrow keys, Home, End and Delete move the cursor and delete; no strange signs, no crash. |
| P08 | 37 | 2026-10-28 | An item used on a spot with nothing on the ground no longer calls the enemies or opens doors. |
| P09 | 41 | 2026-10-30 | Quick Restore plays at the restored music and sound volume at once. |
| P10 | 46 | 2026-11-01 | No wrong pixels above the merc's name on the A.I.M. screen. |
| P11 | 47 | 2026-11-01 | After the inventory screen, a merc you switched to there gets its reserved action points for its new hand item. |
| P12 | 55 | 2026-11-04 | With Subtitles on, an inventory remark ("No Room", "Empty") plays to its end while its text box shows. |
| P13 | 56 | 2026-11-04 | A battle sound on the map box plays to its end (a new round's sound still stops it). |
| P14 | 32 | 2026-10-26 | The battle flash in the replay of the day no longer leaves a shifted copy of the guards, trees and mercs on the map. |
| P15 | 33 | 2026-10-26 | Paging through the four map views no longer moves the map: back in the normal view it shows the same place as before. |
| P16 | 36 | 2026-10-28 | The broken demolition charge in the plant sector 26 now blows up like the other charges of that sector. |
| P17 | 40 | 2026-10-30 | In a fight, the first press of the louder-music or louder-sound key no longer makes it quieter. |
| P18 | 51 | 2026-11-02 | Fox no longer says Needle's name when she means Lance (his name sample is missing): she leaves the name out. |
| P19 | 57 | 2026-11-04 | At a turn change the mouse pointer stays where it is; it no longer jumps to where the last box opened. |
| P20 | 62 | 2026-11-06 | A plant capacity with no remark of the guide no longer plays a broken scrap of speech. |
| P21 | 63 | 2026-11-06 | The End turn key does nothing while an enemy interrupt waits, so the enemies' turn can no longer freeze. |
| P22 | 64 | 2026-11-08 | An item that you hold with the mouse when the day ends goes to a free place of the merc (or to the ground), not over the item in the merc's hand. |
| P23 | 65 | 2026-11-25 | A click on an item of the A.I.M. gear page no longer paints open eyes over the dark face of a merc on the team, on assignment or dead. |
