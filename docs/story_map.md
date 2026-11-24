# Story map: the 43 story bytes and the quest chains

Purpose: the coverage list for quest testing (quest scenarios first, then a campaign bot that is not in this repo;
2026-11-06). It lists each story byte, what reads and writes it, and which quest
chain it belongs to, and for each chain what a test proves today.

- **Story block**: 43 bytes at 0x22A040 in the original (`story` in the port, save offset 20844). Accessor forms
  and every port access: `python3 tools/story_refs.py` (`--check` fails when the port touches a byte that has no
  row below, or when an access does not resolve). Original users: `./t varmap 0x22A040 43 --funcs` (a dword load
  plus `sar 24` reads byte X + 3, so check the asm before you trust a varmap name).
- **Quest status of a save**: `./t quests SAVE [--bytes] [--json]` prints each chain as not started / open / done /
  failed with its counters and the quest items the squad holds; `--campaign DIR` gives the first day each chain
  reached each status over every save in DIR. Its names and meanings come from this file; `./t quests --check` (in
  `./t check`) fails when a chain or byte here has no status rule.
- **Aliases**: bytes 0..4 lie on day accounts +0xA0..+0xA4; byte 0x19 is `merc_tire` (its own port field, save
  offset 20869 wins). See "Aliases" below.
- **Sector numbers**: P = the port's sector index, G = the game's number (G = P + 1; the guide and the maintainer use G).
- **Names**: a quest name below comes from a quote of the game (QST = guide quotes, BRE = Brenda, DAY = merc
  quotes; `./t speech text QST`). A name with `?` has no such proof yet (naming rule, CONTRIBUTING.md).
- **Stage bytes** 5..0x0E: 0 = not started, 1 = open, 2 = done (the performance rating, ending.md, scores them).

## Quest chains

| Chain | Bytes | Items | Start | Done | Fail / cost |
|---|---|---|---|---|---|
| A. Micro-purifier stolen (QST 0) | 5, 0x1A | 0x79 micro-purifier (DAY 18 "micro-gizmo") | guide tip 0, day 1 (owned >= 1); EASY/NORMAL: an enemy of the next unowned sector gets it; HARD: placed in P 48/49/58 | squad holds 0x79 at day end: S[5] = 2, plant 0 runs, bonus $1000 by day L2 | plant stays dead; rating gets no +10 |
| B. Sick workers (QST 2, BRE 10) | 6, 2, 0x25, 0x19 | none | guide tip 1, day 2, only when A is done (S[5] >= 2) and B not started, not all 5 sectors owned: S[2] = 0, merc_tire 30/50/70 | sectors P 28 29 38 48 49 all owned: S[6] = 2, S[2] = 2 (countdown, then merc_tire 0) | rating -5 while open |
| C. Lucas and the water supply (QST 6) | 7, 2, 0x19 | none | guide tip 2, day 3, only when B is done (S[6] >= 2), NOT all of P 28 29 38 48 49 owned (guide.rs TIPS row 2, e[4..9]), and nothing else said that morning (e.g. the tapping-limit quote 0x2D - gap when g_processCap - g_treesTapped < 6), random gate by difficulty: tip with 20/50/80 % (guide.rs step 11) | same sectors owned: S[7] = 2 | rating -5 while open |
| D. Burial ground in G 6 (QST 16) | 8, 0x12, 0x13, 0x24 | none | guide tip 3, day 12, only with >= 24 sectors owned, S[8] = 0 and S[0x0C] = 0 (E not started), P 5 not connected, nothing else said that morning | P 5 connected to home: S[8] = 2, S[0x13] = 1, count +100/+50 | each day not connected: S[0x24] + 1, rating - S[0x24] |
| E. Headstone stolen (QST 18) | 0x0C, 0x18 | 0x77 headstone (placed with QST 18) | guide_advice 9c: guards or tappers in P 5, QUESTS.DAT record 7 places 0x77 in an unowned sector (no free sector: S[0x0C] = 2 at once) | squad holds 0x77: S[0x0C] = 2, count +150 | each day: S[0x18] + 1, rating - S[0x18] |
| F. Brenda's lab in G 28 (BRE 0) | 9, 0x0F, 0x26 | none | guide tip 4 (Brenda), day 1, only with >= 7 sectors owned, P 27 not connected, Brenda not missing (S[0x0B] != 1, S[0x15] = 0), nothing else said that morning | P 27 connected to home: S[9] = 2; by day L5: S[0x0F] = 2 (5000 bonus days) | rating -10 while open |
| G. Brenda missing (QST 20, DAY 20/21/24) | 0x0B, 0x14..0x17, 0x1B | 0x86 sign she was taken (DAY 21, put in P 27), clues 0x7A, 0x5C | guide_advice 9a: S[9] = 2, S[6] = 2, P 27 not owned; QUESTS.DAT record 6: S[0x0B] = 1, S[0x17] = sector of the story object 0xAE | a merc sees the story object (story_find): S[0x16] = 1, at day end $15000, S[0x0B] = 2 | S[0x14] days >= L1: S[0x15] = 1 (dead: DAY 11/20), rating -25 |
| H. Fallow blight (QST 24, BRE 4) | 0x0A, 0x20..0x23 | 0x51 Pomillia flowers (BRE 13-16), 0x5E old antidote? | story_day: >= 30 sectors owned and a QUESTS.DAT pair fits | no owned sector unhealthy at day end: S[0x0A] = 2 (QST 77) | spread (limit 4/3/2); no owned tree left: game over (verdict 3) |
| I. Brenda's journal (BRE 19/20) | 0x0E, 0x1C, 0x1E | 0x6E journal (from the safe in P 11, DAY 31), 0x7C burned journal | guide_advice step 10, first morning: S[0x0E] = 1 (the safe in P 11 counts) | squad holds 0x6E or 0x7C: S[0x0E] = 2 | blast turns 0x6E into 0x7C: S[0x1E] = 1, loses the +25 |
| J. Fallow sapling (QST 26, BRE 6) | 0x1D, 0x1F | 0x5F sapling (DAY 27 "I've got the plant"), 0x7B (rating -25) | guide: S[0x0E] >= 2, S[0x1E] = 0: S[0x1F] = 1 | squad holds 0x5F at the end: rating +25 | holds 0x7B: rating -25 |
| K. Tree price / Dynamic Labs (QST 8-15, 22, 71-73) | 0x27..0x2A | none | trees tapped > 16 or owned > 32 | price back to 500: S[0x27] = 4 (QST 73) | price -100 then -20 steps |
| L. Plant 2 threat in G 54 (QST 74) | 0x0D | none | plant 2 owned, connected, enemies > 6 near: S[0x0D] = 1 | next morning S[0x0D] = 2 | enemies weigh P 53 higher (attack_weight) |
| M. Santino and the win | none (santino_spoke, end code) | none | sector 0 | win film, rating | not a story byte; gate_campaign steps 15-16 |

## Coverage today

Scenario runner: `validate/quest_scenarios.py` (in `./t check`): one stage per scenario, from a port-written save edited
for the stage's preconditions, real script input, checks on story bytes and game effects. Mutation-tested 2026-11-06
(chains A, J, K, L: HARD site, NORMAL flag, the L2 test, the sapling hint test, the 0x5F rating, the K start, drop, raise and 500 tests, the L enemy test and stage 2, in two batches; day_verdict steps 6, 11, 16, 17, 18, 21, story_day blight start and end, and the guide's merc_tire reset broken one at a time or in a batch: each
matching scenario fails, the others stay green). Connected-to-home paths follow the save's side flags (home.rs).

Cheat campaign bot: `validate/gate_quests.py` (1 day in `./t check`; long runs by hand, `--days N --attack [--start SAVE]`)
writes the chain status of each day to build/gate/quests_days.jsonl. Runs of 2026-11-07: A done by play; B and I open;
C..H and J..L not started (no route script yet: the fights alone do not reach them). The tip gates (port guide.rs TIPS, step 11 of
guide_advice 0x5F94C, guide_diff --port) explain it: C waits for B done (all of P 28 29 38 48 49), F for 7 owned
sectors, D for 24. With `--arm` (M16s by `./t cheat --arm`) the 12-day long run from week_normalgate11_day7 owns 7
from day 12 and F starts by play on day 13 (2026-11-07); without it, 5. Best long-run flags (2026-11-07):
`--days 16 --attack --arm --aim 90 --margin -2 --armor --lean --route-margin -4 --start
build/gate/week_normalgate11_day7.SAV` (armour by `./t cheat --armor`, guards only on sectors with workable trees or
on the chain B route, route targets with up to 4 more enemies than fit mercs): **B done by play on day 10** (29
taken), F done on day 16 (27 taken: the lab connected to home), owned 13 on day 22. Without --route-margin: owned 14,
F done on day 12, but B open (29, 8 enemies, never attacked). Without --armor --lean: owned 10.
**C by play (2026-11-07, `--letgo`):** tip 2 needs a B sector NOT owned and a quiet morning, so after B the bot gives
back the guards of 29 (fell to enemies 7 days later), taps at most g_processCap - 6 trees while 29 is lost (else the
guide says the tapping-limit quote each morning), and plays each day with its own seed (one seed for every day drew
the same 50 % roll on each quiet morning). From the day-9 save of that run (29 lost): C open on day 31, 29 retaken and
**C done on day 32**. Whole 24-day run from day 7 (2026-11-07, two bot fixes): the morning landing tries every
sector a target starts from (ConnectedHome refuses 29: its byte 0 edge bit 0x08 closes the way to 39), and LET_GO is
the B sector the game's enemy move attacks most often (week_normal.attacks), with its enemy neighbours left alive:
**B done day 13, 48 let go, C done day 22**, K stage 1 day 24, owned 14 on day 27.
**F and G by play (2026-11-07, `--letgo` from that run's day-25 save):** with C done, P 27 joins the route bias while F
is open: 27 taken on day 29, **F done day 29**. Guide 9a starts G with no roll (F and B done, 27 not owned), so the bot
lets 27 go: **G open day 31** (story object in P 23). Its sector joins the bias, but every attack on 26 on the way is
lost; from the day-39 save **G failed (Brenda dead) at the day-40 end** (10 days missing on EASY, L1).
**G alert rule (guide s037 "they will try to kill Brenda", checked in the asm-based port 2026-11-07, live in JA.EXE 2026-11-08):** the claim is
real, and it starts with the enemies' alarm, not with the squad's entry. Sector setup (sector.rs step 6) sets the
mission step to RadioResult? 0x63254: 1 with mission target = the tile of story object 0xAE when S[0x0B] = 1, the
current sector = S[0x17], S[0x16] = 0 and S[0x15] = 0; mission_slot stays 0x40 (no enemy has the order). The first
alarm (CombatAction 0x6E3F4 action 7: an enemy in combat with >= 5 AP shouts, 50/80/95 % by difficulty when no alarm
is up yet) calls MissionHandOver? 0x72670: the enemy with hp >= 15 and energy >= 35 that can reach the target with the
fewest path AP gets the order. That enemy walks to the mission spot and attacks the target tile (CombatAction mission
step 1); a melee hit (aim.rs, attack step 2) or a shot (shoot.rs step c) on a tile with object 0xAE calls StoryHit?
0x61D28 (0x22A055 = 1, 0xAE -> 0xAC): the find then gives the "found dead" quotes. Bot rule: the squad must see
her tile (StoryFind 0x6179C, S[0x16] = 1) before the order enemy reaches it, or kill each enemy before its first
shout; a merc that sees her first ends the danger (S[0x16] = 1 blocks RadioResult and StoryHit). **Live check
(#61, #62, 2026-11-08):** a port quick save with the order enemy on mission spot 1423 (next to her tile) in JA.EXE:
the enemy kills her on its first turn (S[0x15] = 1, mission step 0, the enemy walks on to 1598). The port did not:
no front-end handled op 177 story_hit until commit 4d6f1c6 (now a library op, storyhit_diff). Port G results in P 23
before that commit (finds after long fights) are too kind. In a fight where the enemies spend all their AP on
shots no enemy shouts (alarm 0, no order; same in both games, build/probe/claude_g/alarm3.py, alarm_orig.py).
**G room in P 23 (probes 2026-11-07, bot save build/probe/g2/d36.SAV moved to 23; corrected 2026-11-08, #61
c12146):** the story object is on tile 1479 (x 23, y 26), an unwalkable tile in a closed building (wall row y 24).
Door table of P 23: 1475 = 58, 1477 = 161 (big, lock 61), 1482 = 175 (big, lock 75), 1598 = 38, 762 = 74 (combat.md
"The door effect": value = key id; > 100 = big door, v - 100; kit: condition x lock skill / 100 >= v). No keys (items
0x57/0x58) lie in P 23 or on its 4 enemies, and its trigger slots are empty (build/probe/claude_g/keys.py). Two ways
reach the tiles next to her (1478, 1422..1424):
- **West, through water (the easy way).** Shallow water (move code 3600) on the west shore -> 1474 -> door 1475 (58)
  -> 1476 -> big door 1477 (61) -> 1478. The old note "1475/1477 lead only to a closed pocket" was wrong: its search
  did not allow water. Speck (merc 3, lock skill 70, kit condition 100, no cheat) walks from the landing through the
  water, opens 1475 and 1477 on the first try and sees Brenda (S[0x16] = 1; build/probe/claude_g/west.py, no
  enemies). Lock 61 needs kit condition >= 88 at skill 70 (70 x 88 / 100 = 61), so no failed tries on other doors
  first (each failed try: Random(100) < v drops the condition by 1). The fandom wiki ("Find and Rescue Brenda") gives
  the same plan: one fast merc swims to the west side with a lock kit and opens the back doors, without a fight.
- **East, through 1598 (38) and big door 1482 (75).** Speck opens 1598 but not 1482. Lock skill >= 75 on A.I.M. (d36
  profiles): Mike 128 ($12000/day), Magic 90 ($5600), Spike 80 ($6000), Scully 78 ($7500); Speck reaches 75 at level
  2 with a kit at condition 100. With `./t cheat --lock 85@3` Speck opens 1482 after a fight (4 enemies dead in 35
  turns; that fight ran in the port before the story_hit fix, see the alert rule). gate_quests.py `--pick --lock 85`
  gives the picker that skill each day.
- **Grenades.** A grenade (item 0x1C) thrown at 1598 (from 1602) and at 1482 (from 1485, same row, 3 tiles away)
  breaks both doors (combat.md Explosion step 5: a size-2 blast on an unstandable tile; tileset 1, no_break 0); the
  merc sees Brenda alive and the day-37 end gives S[0x0B] = 2 (build/probe/claude_g/gthrow.py, save d36g.SAV). The
  blast radius is 2 and she is 3 tiles from 1482. Walls can break too: explode_art (combat.md "Craters and blast
  leftovers", DAT\EXPLODE.DAT) breaks the objects round a kind 2/3 blast.
- **North-west landing, the way that works (#61, 2026-11-08; `gate_campaign.rescue_cross`, `--g-rescue`, OFF).** Start = a
  clear P 13 (bot save d36 moved there, 13's enemies removed, no lock cheat). The squad walks the wet path to 13's west
  part and crosses S to the P 23 north-west island. The squad goes first over dry row 6 (one tile wide at columns
  10..17; a merc on it stops the kit merc) to (20..22, 5). A squad merc who sees an enemy within 9 tiles shoots it. Speck
  then walks to 1474, rests while his anim word < 4 or AP < 10, and opens both doors with right click + click. Found alive
  in 14 of 16 seeds, 4 mercs alive in all 14, and S[0x0B] = 2 at the day end in all 14. The 2 failures: an enemy shouted
  (Brenda dead), and enemy 43 killed the squad. `rescue_cross` alone: 7 of 8 new seeds, the eighth gave no result in 280 s.
  From P 33 with the same rules: 0 of 8 found, Brenda dead in 5.
  Walk clicks on 13's wet path go 1 tile at a time: a longer click lets the game's own path enter deep water (5000), where
  clicks do nothing. With the day-3 squad, 3 of 8 seeds still stopped on such a tile. Test: quest_scenarios `brenda_route`
  (stored script validate/brenda_route.txt, squad of 4, P 23 with one enemy). **Live check (2026-11-08, validate/brenda_live.py,
  also from bot save d36 with all enemies):** a port quick save at 1474 in JA.EXE: both doors open at 10 AP each, Brenda's
  rescue speech plays, S[0x16] = 1, S[0x15] = 0, day end S[0x0B] = 2. The walk from P 13 to 1474 ran in the port only.
- **Which sector.** Not always P 23: 9a takes the first pair of QUESTS.DAT record 6 with neither sector owned. Holding
  sectors in file order: 23, 32, 52, 6, 54 (25 pairs, 5 clue sectors each); no dice. So the guide's "almost 95%" means
  P 23 is first. The route above is for P 23 only.
Enemies walk through these doors (with .38s one enemy ended inside the building at 1422). Guide P 26 keys (68 = the
P 26 doors of value 68, padlock 77) do not fit 1482.

| Chain | Played (real input from a port save) | Edited save | Diff test (function vs original) | Uncovered |
|---|---|---|---|---|
| A | week_normal.py steps 3-4 (drop, pickup, day-2 story, NORMAL); gate_quests.py --days 8 --attack from DAY_day3: done on day 4 (sector 49 taken on day 3) | quest_scenarios purifier_hard (HARD: 0x79 in record P 48/49/58), purifier_normal (carrier flag), purifier_late, purifier_on_time (bonus only by day L2), purifier_pickup (quote 0x12 once, S[0x1A]) | verdict_diff, guide_diff | none (gap 8 done: purifier_hard_by_play) |
| B, C | B open in DAY_day3.SAV (gate_campaign play from New game); gate_quests.py best flags (2026-11-07): B done by play on day 10; C (with --letgo) done on day 22 in the whole run from day 7 | quest_scenarios.py water_done, water_open, water_cured (stages, countdown S[2], merc_tire reset) | verdict_diff, guide_diff, quest_live (#61 c12084, c12087) | start in `./t check` (gap 3); BRE 10 (gap 4) |
| D | | quest_scenarios burial_done, burial_cut, burial_start (tip 3: day 12, 26 owned, P 5 not connected, a quiet morning), burial_reminder / burial_kept (S[8] = 2: P 5 cut off -> the morning clears S[0x12] with quote 0x38; connected -> S[0x12] stays 1) | verdict_diff, guide_diff | no played step at all: done by play (gap 1); QST 76 (gap 4) |
| E | | quest_scenarios headstone_back, headstone_missing, headstone_placed / headstone_next / headstone_none (9c: 0x77 into the first free record-7 sector, or S[0x0C] = 2 at once) ; headstone_by_play (finding it by play: P 22 land route from the fort past 4 undetectable and 2 trapped mines, pick up on 1104, day end S[0x0C] = 2) | verdict_diff, guide_diff | start with guards set by play (gap 2) |
| F | gate_quests.py --days 12 --attack --arm: started by play on day 13 (7 owned); best flags (--armor --lean): done by play on day 12 (27 taken, connected to home); --letgo from day 25: done day 29 | quest_scenarios lab_done, lab_late (day 11 > L5: no bonus days) | verdict_diff, guide_diff | start in `./t check` (gap 3); QST 75 (gap 4) |
| G | gate_quests.py --letgo (2026-11-07): open by play on day 31 (guide 9a, 27 let go), failed (Brenda dead) by play at the day-40 end; again at the day-44 end with --pick --swim (26, 36, 25 taken by play, never held); done by play from an edited position (2026-11-07, build/probe/lk/gdone.py: the day-36 bot save with the squad moved into P 23 and `./t cheat --lock 85@3`): the fight finds Brenda (S[0x16] = 1), the day-36 end gives S[0x0B] = 2, `./t quests` G done | smoke_play.py step_story_find (S[0x17] set by hand); quest_scenarios brenda_dies, brenda_home, item86_pickup (item 0x86: quote 0x15 once, S[0x1B]); brenda_route (2026-11-08, stored script validate/brenda_route.txt: squad of 4 from a clear P 13, one enemy in P 23, merc 0 opens doors 1475 and 1477 with the kit in combat, Brenda alive, day end S[0x0B] = 2; no kit: no find) | storyfind_diff, storyhit_diff, verdict_diff; brenda_live.py (2026-11-08, JA.EXE from a port quick save at 1474: both doors, S[0x16] = 1, S[0x15] = 0, S[0x0B] = 2; not the walk from P 13 and not the fight at the wall) | start in `./t check` (gap 3); clue quotes (gap 5). Not gaps: the bot never reaches P 13 (`--g-rescue` stays OFF), and the day-3 squad stops in deep water in P 13 on 3 of 8 seeds (held, #61 c12184) |
| H | | gate_endings.py blight loss (S[0x0A] = 1 by hand); quest_scenarios blight_start (30 sectors owned), blight_cure (item 0x5E), blight_grow / blight_jump / blight_kill (one day of spread: more sick trees, the jump to an owned neighbour, a dead tree at the NORMAL limit), flower_cure / flower_journal (Pomillia flower 0x51: S[0x21] = 1, S[0x22] = 3 days without the journal (Brenda 16), 1 day with it (Brenda 14); then S[0x20] = 1 (Brenda 18) and the blight ends (Brenda 17)) ; flower_by_play (2026-11-08: the flower picked up by play in P 7 = G 8, four flowers on 710/772/825/937, seven mines in the west half, route north from the landing tile; day end S[0x21] = 1, S[0x22] = 3, Brenda 16); antidote_by_play (2026-11-08: blight_start writes 0x5E into the record of the record-5 pair's story sector b: pairs (sick a, b, tile) = (54, 6, 1425), (24, 41, 775), (18, 55, 380), (41, 4, 1713), (39, 51, 1591), the guide's G 7, 42, 56, 5, 52; in P 6 it lies in a room of the building north east of the landing, doors 1597 + 1483; picked up by play, the day end S[0x20] = 1, the morning ends the blight, Brenda 17) | storyday_diff, verdict_diff | none for the sites (gap 9 done: antidote_p41, _p55, _p4, _p51); QST 78 (gap 4) |
| I | open in DAY_day3.SAV (gate_campaign play); the gate_quests.py runs (to day 18) do not move it | smoke_play.py safe steps (S[0x0E] = 1 by hand; 0x6E sends the squad out); quest_scenarios journal_back, journal_burned, journal_blast (blast on the ground journal: 0x7C, S[0x1E] = 1) | verdict_diff | start in `./t check` (gap 3); journal back by play (gap 6). Notes: the journal blast by play is done (journal_trigger). Port probe 2026-11-07 (build/probe/l/p11c.py): the journal 0x6E lies on P 11 tile 457 (no safe trigger near); with S[0x0E] = 1 the P 11 mission step is 3 (sector.rs radio_result step 3, quest_check 0x1C9), and only an enemy holding a Detonator 0x40 (JA.EXE name) fires the mission blast (ai.rs action 10 -> blast.rs mission_blast: sector 11 blows item type 0x6E = the journal). On P 11 entry from DAY_day3 (difficulty 0..2, seeds 1..3) none of the 6 enemies holds one: they carry Combat Knives 0x10 in the second hand (the guide's "item 16"?). XRA.DAT (CD and 1.13b, build/probe/l/xra.py): no P 11 template holds a Detonator, but the P 11 ground list holds item 0x72 (the mission trigger): springing it (blast.rs trigger_item case 0x72) runs the same mission blast, so the by-play route is the 0x72 trigger tile. Done 2026-11-07 (quest_scenarios journal_trigger, build/probe/i/scen.py): the trigger lies on tile 1311 (condition -2, inside the house behind the locked doors 1200 and 1192, lock value 4; a lock kit opens them). UseOnTile? 0x3CFD0 step 3 drops the ground entry when the merc's tile is below the object tile (merc north of it), so the merc Ctrl clicks it from 1367 (south). A merc with explosives / 30 + level < 2 does not spot it and sets it off (DisarmExplosivePrompt step 5 -> TriggerItem 0x72): the mission blast burns the journal on 457 (0x7C, S[0x1E] = 1), mission step 0, and the merc takes the spent Detonator 0x40. A merc that spots it and passes the roll turns it into a Detonator with no blast. Journal back by play (probe 2026-11-08, build/probe/h/walk.py S=11): the journal on 457 lies in the wing cols 9..11 rows 9..14 of the big house; its only ways in are door 797 (value 82) and door 573 (98) (P 11 door table: 1077 68, 806 48, 573 98, 797 82, 1415 64, 1043 2, 695 4, 687 4, 1420 4, 1192 4, 1200 4) = the guide's safe (lock kit near 100 % with lock pick 80/90/100). No DAY_day3 merc reaches lock skill 82 at dexterity 100 (`./t cheat --lock`: best merc 4 = 61); grenades do not open these doors: a blast on 1200 (thrown from 1203) and on 797 (thrown from 1023 at 741 or 685, effect_hit 797; a throw aimed at 797 lands on 853) leave them shut. Open: a picker hire (Mike / Magic / Spike) or note #14. Script recipe: hand swap = `strip 280 R` (second hand = soldier +12). |
| J | | quest_scenarios sapling_hint, sapling_burned (hint S[0x1F]), sapling_rating (win rating +25 with 0x5F, -25 with 0x7B), sapling_pickup (quote 0x1B once, S[0x1D] = 1), sapling_blast (0x5F -> 0x7B) | rating_diff (items held); gate_campaign step 16 (sapling 0x5F on P 0 tile 345 at entry; the stage 3 record on 345, kind 0xA, turns.rs STAGE_FX, burns it: 0x7B) | none. Notes: sapling pickup by play: 345 is a blocked tile (move code 25500), the nearest standable tile is 401 and the stage 0 bombs lie on 402/404; build/probe/l/sap2.py (gate_campaign approach to 401, 2 mercs from campaign_day3): merc 0 dies in the stage blasts on turn 7. Done 2026-11-07 (quest_scenarios sapling_by_play, build/probe/i/sap7.py): Santino speaks when the merc reaches 685 (after door 686); the stage 0 bombs go off at once, the stages then advance one per turn end (stage 1: 628 + 404 at difficulty >= 1 with 30 s delay; stage 2: 349 at 8 s, 682 at 20 s into the next turn; stage 3: fire on 345). A merc that waits 400 ticks on 685, walks to 401 and Ctrl clicks 345 in the same turn takes the sapling (quote 0x1B, S[0x1D] = 1); it is hurt at the turn end (hp 82 -> 42) but lives. Waiting near 686 or 687 at a turn end loses AP or the merc. |
| K | gate_quests.py --letgo run (2026-11-07): stage 1 by play by day 34 (19..20 trees tapped > 16); run goE (2026-11-07, `--out build/probe/dash/goE` from build/probe/g2/d25.SAV, best flags + `--stuck --swim-edge --g-squad 6`): plant 1 (P 26, capacity 35) taken and connected, g_processCap 55, 31 trees tapped: stage 2 by play on day 34, the -100 drop (stage 3) on day 35, the first -20 step (S[0x29] = 1) on day 39 | quest_scenarios price_start (owned > 32), price_drop (-100), price_raise (+20, first raise quote byte), price_back (500: S[0x27] = 4), price_step (first -20 step, S[0x29] = 1) | guide_diff | none (see "Not gaps" below). Notes: price raise and price back by play (t = 300 + tapped / 10 x 20 above the price needs 50+ tapped at 380) Stage 3 moves the price only toward t and stops at t = price (no count change), so 30..39 tapped trees hold it at 360: a raise by play needs more tapped trees after the fall (40+ at 360). goE day 40 (build/probe/kdip): 31 -> 20 tapped after a lost sector, income 7600 < natives cost 9030, no money even with no attack and 3 mercs fired.) |
| L | | quest_scenarios plant_warn, plant_calm (enemies > 6 near), plant_done (next morning 2), plant_attack / plant_quiet (S[0x0D] = 1: P 52 attacks P 53, 9 guards -> 4; S[0x0D] = 2: no attack; the weight holds only for the one day between the warning morning and the next); plant_hold / plant_hold_quiet (2026-11-08, the guide's advice: the squad in P 53 on the warned day: sector_value 1000 draws the enemy move onto the squad, 2 enemies walk in on the north edge from P 43 (not a random neighbour as the guide says: the move picks by value), the fight starts, guards stay 9; S[0x0D] = 2: nobody comes) | guide_diff, enemy_diff | none (gap 7 done: plant_hold_win) |
| M | | gate_campaign.py steps 15-16 (other enemies cleared) | rating_diff | |

### Gap list (claude 2026-11-08, #61 c12184 step 1)

Source: the rows above, the SCENARIOS list and the asserts of validate/quest_scenarios.py, guide.rs, verdict.rs. A "scenario" here
is an edited save plus real input, with a planted fault that it catches. None of these rows needs bot strategy. Nobody ran
the new scenarios yet: when a row cannot pass, record why with evidence and go on.

| Gap | Chain | Why it is a gap | New test and pass condition |
|---|---|---|---|
| 1 | D | The only chain with no played step: every D state comes from an `own()` edit | `burial_by_play`: S[8] = 1, BURIAL_PATH owned but P 5, squad in P 5, its enemies beaten by script input. Day end: S[8] = 2, S[0x13] = 1, S[0x12] = 1, S[0x24] does not change. Same script with P 15 not owned: S[8] = 1, S[0x24] + 1 | **Done 2026-11-08 (`burial_by_play`, stored script validate/burial_route.txt; 2 of the 6 enemies and no enemies next to P 5 are preconditions: the day-3 squad loses to 6).**
| 2 | E | headstone_placed sets 9 guards by save edit | `headstone_start_by_play`: P 5 owned with 0 guards, guards set by real clicks on the island map (map mode 2). Next morning: S[0x0C] = 1, 0x77 in the first free record-7 sector. No clicks: S[0x0C] = 0 | **Done 2026-11-08 (`headstone_start_by_play`; path owned and pool + 100 are preconditions: map messages 2 and 12).**
| 3 | B C F G I | Their start runs only in guide_diff and in long bot runs, which are not in `./t check` | One scenario per start. B: tip 1 S[6] = 1. C: tip 2 S[7] = 1 (the roll on both sides, by seed or difficulty). F: tip 4 S[9] = 1 with 7 owned. G: 9a S[0x0B] = 1, S[0x17] set, records hold 0x5C, 0x7A and 0x86. I: step 10 S[0x0E] = 1. Each has a case that does not start, from the row's gate | **Done 2026-11-08 (ROWS `start_b`, `start_b_no`, `start_c`, `start_c_roll` seed 4, `start_c_owned`, `start_f`, `start_f_no`, `start_g` incl. P 27 owned, `start_i`, `start_i_no`). The second clue of the first pair is P 24, not P 32.**
| 4 | B D F H | No scenario asserts these quotes or bytes: S[0x25], S[0x26], S[0x23], and quote 0x4C | Morning quotes, each with its byte and each said at the right count only. B: BRE 10 once, S[0x25] = 1. F: quote 0x4B when S[0x26] reaches 3. D: quote 0x4C at S[0x24] = 3. H: quote 0x4E, S[0x23] = 1 (the code also repeats it when nothing else was said: test both) | **Done 2026-11-08 (ROWS `quote_b`, `quote_b_said`, `quote_f*`, `quote_d*`, `quote_h`, `quote_h_over`, `quote_h_again`, `quote_h_blocked`).**
| 5 | G | No scenario asserts the day-end clue quotes (verdict.rs, `item_told`) | Each of 0x86, 0x7A, 0x5C in hand with S[0x0B] = 1: quote 0xAB, 0xA0, 0x9F on the first day end only. The second day end gives the day-count quote | **Done 2026-11-08 (ROWS `clue_86`, `clue_7a`, `clue_5c`, `clue_none`).**
| 6 | I | Journal back has only a hand edit (journal_back) and a safe by hand (smoke_play) | `journal_by_play`: merc with lock skill edited to >= 82 (a precondition, as in brenda_route), kit in hand, opens door 797, takes 0x6E from tile 457. S[0x1C] = 1, squad out, day end S[0x0E] = 2, Brenda 19. No kit: door shut, no journal. Then check note #14 (guide: three pieces) in the asm: add a test, or record that the port has no such way | **Done 2026-11-08 (`journal_by_play`, `note14_safe`). The safe needs skill >= 90, door 797 needs 82. Live in the original (#61 evidence): a journal on the cursor at the day end goes over the selected merc's main hand (CursorItemBack? 0x53660 from 0x11370, #2), and S[0x0E] = 2. Port fault fixed: the leave of the pick-up's pass now waits for the inventory screen (`Play::leave_held`). Note #14: three items 0x73 merge by table rows (0x73, 0x73) -> 0x74 and (0x73, 0x74) -> 0x87; 0x87 in hand opens the safe with stats too low for the kit.**
| 7 | L | plant_hold stops when the fight starts | `plant_hold_win`: from plant_hold, the squad beats both enemies by script input. P 53 stays owned, guards stay 9, the plant still runs at the next day end | **Done 2026-11-08 (`plant_hold_win`, stored script validate/plant_hold_route.txt, seed 3). "The plant runs" here = the day end's step 20 (state 0 -> 1, quote 0x93) and the morning's S[0x0D] = 2: this day-3 plant is not yet at state 2. The same edit with other seeds brings a second wave, or an enemy that the bot cannot reach (no fix: no bot work).**
| 8 | A | purifier_hard only places the item; purifier_pickup does not run on HARD | `purifier_hard_by_play`: HARD save, seed fixed, walk to the tile that the record gives, pick up. Quote 0x12, day end S[5] = 2. Bonus only by day L2 | **Done 2026-11-08 (`purifier_hard_by_play`, stored script validate/purifier_hard_route.txt, seed 2). The record gives no tile: R(c) +0x9E is the item that the sector's first enemy holds (sector.rs step 4e), on HARD too; the only HARD difference is the site. The squad beats the four enemies of P 49. Late case: 0x79 by edit on day 2 (the day changes the fight).**
| 9 | H | antidote_by_play covers only site P 6 | One scenario per site P 41, 55, 4, 51: pick up 0x5E, day end S[0x20] = 1, the morning ends the blight. For a site that the squad cannot reach: record the door value and the lock skill it needs | **Done 2026-11-08 (`antidote_p41`, `_p55`, `_p4`, `_p51`). Lock skill 100 by edit (best day-3 skill 46). Doors and values: P 41 big door 1223 (big key id 53, no way round, without it merc 0 stops on 1279) + 944 (60); P 55 939 (58) + 435 (82); P 4 2165 (34) + 1940 (50); P 51 1759 (70). Each box is locked: the first Ctrl click opens it, the second picks up.**
| 10 | all | **Done for day-end saves 2026-11-08 (evidence #61).** quest_live on the 42 run() scenarios with no result: 41 equal in the first run; headstone_none differed once (S[9], S[0x26], tip 4 at exactly 7 owned, the original uses live RNG for enemy moves) and was equal in a rerun, with the same save shape as start_c_owned (P 58 no guards, 7 owned), where part (a) saw the lost sector move tip 4. start_f through the morning: equal, S[9] = 1. | **Part (a) done 2026-11-08 (evidence #61):** the 19 days = 1 rows that read the morning are in TWO_DAYS: 15 equal in the first run. start_c, start_c_roll and start_g_no differ only by the 50 % roll of tip 2 (original S[7] = 1 in 6 of 12 runs, port 20 of 40 seeds). start_c_owned differs only when the enemy takes unguarded P 58 (7 owned: S[9] = 1 and equal; 6 owned: S[9] = 0; original keeps 58 in 2 of 12 runs, port on 5 of 40 seeds). The live test compares story bytes, not voices. flower_cure: first day end of 4. **Not live-compared:** (b) Need a walk (quest_live replays no input): antidote_by_play, antidote_p4/p41/p41_nokey/p51/p55, brenda_route, burial_by_play(_cut), flower_by_play, headstone_by_play, headstone_start_by_play, journal_by_play_*, journal_trigger, plant_hold, plant_hold_quiet, plant_hold_win, purifier_hard_by_play_*, sapling_by_play, *_pickup_again, sapling_again: their result comes from input before the day end. Old probe saves, not scenarios: fl*, m_*, pa*, x, y |

**Not gaps (no new test; the maintainer's plan c12184: no more bot strategy):**
- D start and L reach by the bot: both need more owned sectors than the bot holds (19 at most). burial_start and plant_warn test the same code from an edited save.
- K raise and back by play: the gate is money for 40+ tapped trees. No player input reaches the code except tappers. price_raise, price_back and the live compare cover it.
- H spread and F late branch: no player input (blight_grow / blight_jump / blight_kill, lab_late).
- J: the pickup by play is done (sapling_by_play), and gate_campaign step 16 burns the sapling.
- G: the bot route to P 13 and the deep-water stops of the day-3 squad stay held.

## Bytes

Writers and readers are port functions (file: function). V = values seen in the code.

| Byte | Chain | Meaning | V | Writers | Readers |
|---|---|---|---|---|---|
| 0x00 | alias | accounts +0xA0 A.I.M. resignations | count | accounts only | accounts only |
| 0x01 | alias | accounts +0xA1 non-payments | count | accounts only | accounts only |
| 0x02 | B, C | sick-workers countdown (story only, see Aliases) | 0, 2 | verdict: day_verdict (2); guide: guide_advice (0, -1) | guide_advice |
| 0x03 | alias | accounts +0xA3 A.I.M. link | | accounts only | accounts only |
| 0x04 | alias | accounts +0xA4 A.I.M. page (g_aimProfile) | | accounts only | accounts only |
| 0x05 | A | purifier stage; 2 = plant 0 runs | 0 1 2 | guide_advice (tip 0); day_verdict | rating, day_verdict, day_end, guide_advice, plant_sector, wal_story |
| 0x06 | B | sick-workers stage | 0 1 2 | guide_advice (tip 1); day_verdict | rating, story_sector, sector_value, guide_advice, wal_story, day_verdict |
| 0x07 | C | water-supply stage | 0 1 2 | guide_advice (tip 2); day_verdict | rating, guide_advice, wal_story, day_verdict |
| 0x08 | D | burial-ground stage | 0 1 2 | guide_advice (tip 3); day_verdict | rating, guide_advice, day_verdict |
| 0x09 | F | Brenda's lab stage | 0 1 2 | guide_advice (tip 4); day_verdict | rating, story_sector, sector_value, guide_advice, day_verdict |
| 0x0A | H | blight stage | 0 1 2 | story_day; day_verdict; lib.rs playloop_run (gate_endings hook, JA1 day-end test only) | rating, story_day, day_verdict, wal_story, walter |
| 0x0B | G | Brenda-missing stage (code: "ransom") | 0 1 2 | guide_advice (9a); day_verdict | rating, win_vars, story_sector, sector_value, setup_sector, radio_result, story_day, ransom_open, walter, day_verdict, guide_advice |
| 0x0C | E | headstone stage | 0 1 2 | guide_advice; day_verdict | rating, guide_advice, wal_story, day_verdict |
| 0x0D | L | plant-2 warning said | 0 1 2 | guide_advice | attack_weight, sector_value, guide_advice |
| 0x0E | I | journal stage | 0 1 2 | guide_advice (step 10); day_verdict | rating, win_vars, end_of_day, quest_check, story_day, cure_words, wal_story, walter, day_verdict, guide_advice |
| 0x0F | F | lab bonus days left | 0 2 | day_verdict; day_bonus | day_bonus |
| 0x10 | none | no user in the original or the port | | | |
| 0x11 | none | no user in the original or the port | | | |
| 0x12 | D | burial-ground "keep a route" reminder due (QST 56) | 0 1 | day_verdict (1); guide_advice (0) | guide_advice |
| 0x13 | D | burial ground secured once | 0 1 | day_verdict | day_verdict |
| 0x14 | G | days Brenda is missing | count | day_verdict | story_find, guide_advice, day_verdict |
| 0x15 | G | Brenda dead | 0 1 | day_verdict | rating, win_vars, melee_hit, projectile_impact, sector_value, radio_result, story_day, story_find, day_verdict, ransom_open, guide_advice |
| 0x16 | G | Brenda found | 0 1 | story_find | melee_hit, projectile_impact, sector_value, radio_result, start_turn, merc_view, day_verdict |
| 0x17 | G | sector of the story object; 0xFF none | sector | guide_advice (9a); setup_sector; story_find | melee_hit, effect_hit, projectile_impact, sector_value, radio_result, start_turn, merc_view, day_verdict |
| 0x18 | E | days the headstone is missing | count | day_verdict | rating, day_verdict |
| 0x19 | alias | merc_tire (port field `update.merc_tire`) | 0 30 50 70 | guide_advice (field) | fatigue (field) |
| 0x1A | A | micro-purifier pickup quote said | 0 1 | pick_up | pick_up |
| 0x1B | G | item 0x86 pickup quote said | 0 1 | pick_up | pick_up |
| 0x1C | I | journal picked up (sends the squad out) | 0 1 | pick_up | end_of_day, quest_check, pick_up |
| 0x1D | J | sapling pickup quote said | 0 1 | pick_up | pick_up |
| 0x1E | I | journal burned | 0 1 | explode_dat | rating, win_vars, guide_advice, quest_check, story_day, cure_words |
| 0x1F | J | sapling hint said | 0 1 | guide_advice | guide_advice |
| 0x20 | H | antidote used / blight ends | 0 1 | story_day; day_verdict | story_day, day_verdict |
| 0x21 | H | Brenda makes the antidote | 0 1 | story_day; cure_words | story_day, day_verdict |
| 0x22 | H | days until the antidote | 1 3 | story_day; cure_words; day_verdict | story_day, day_verdict |
| 0x23 | H | blight-spread warning said (QST 78) | 0 1 | guide_advice | guide_advice |
| 0x24 | D | days the burial ground is cut off | count | day_verdict | rating, guide_advice, day_verdict |
| 0x25 | B | Brenda's water message said (BRE 10) | 0 1 | guide_advice | guide_advice |
| 0x26 | F | lab reminder count (QST 75 at 3) | count | guide_advice (s_add) | guide_advice |
| 0x27 | K | tree-price stage | 0..4 | guide_advice | guide_advice |
| 0x28 | K | tree-price day counter | signed | guide_advice | guide_advice |
| 0x29 | K | first price drop said | 0 1 | guide_advice | guide_advice |
| 0x2A | K | first price raise said | 0 1 | guide_advice | guide_advice |

## Aliases

- Bytes 0..4 = day accounts +0xA0..+0xA4 (accounts is 0xA8 bytes at save offset 20684). In the original the two
  views are one memory. The port keeps two buffers and makes the save from `accounts` for bytes 0, 1, 3, 4. Byte 2
  is a story byte only: its original users are DayEndVerdict, NativeGuidePrompt? and NewGameInit, no accounts
  code. Fixed 2026-11-06 (port/ja1front/src/world.rs, test `story_byte_2_survives_the_save`): the save used to
  write the stale `accounts[0xA2]` there, so the sick-workers countdown went back after a save and a load.
- Bytes 5..7 = accounts +0xA5..+0xA7: no accounts code uses them.
- Byte 0x19 = `merc_tire`: the save writes the field after the story block, so the field wins.
- A cheat must not write accounts +0xA0..+0xA7 or `merc_tire` (it changes story bytes), and must check all 43
  story bytes after each write. `./t cheat` (tools/cheat.py) does this: it edits only g_soldiers, then compares the 43 bytes
  and every chain status before it writes OUT.

## Notes

- **P 22 (G 23) map, chain E (2026-11-07, guide s088 figure + port probe build/probe/e/walk.py):** the fort (west, rows 13-23) holds the squad landing; the headstone island (rows 13-26, cols 36-46) has the record-7 tile 1104 with undetectable mines (condition -90) on 937, 1158, 1162, 1271. Land route: col 13 bridge (rows 24-37, the black-15 charges 0x3F/0x71 on it do not go off when walked on) between the mines 1356/1358, row 40 bridge east, 2325 -> 2437, row 43 bridge east, col 41 bridge north (black-6 charges 0x71 rows 30-39), then 1330, 1273, 1216, 1160. Keep off the panic trigger 0x72 on 1023 (guide: level 2, blows the col 41 bridge; not checked in the port) and the trigger 0x2C on 2436 (level 7: the 15 bridges). The north island (216/269/326/379) holds trapped live plastic 0x20. Deep water (5000) fills the rest.

- Each port file has its own STORY_LEN (32, 35 or 43); a read at or past it gives 0. Every literal index is
  inside its file's length today (`story_refs.py` prints the lengths).
- LoadTacticalSector? reads bytes 6 and 7 (S[6] = 1 or S[7] = 1 sets 0x22A104 from a difficulty table at
  0x95C8E). Nothing reads 0x22A104 (labels.csv), so the port leaves it out.
- QUESTS.DAT records 5, 6, 7 feed chains H and E (strategic.md "Story events at the day start").
